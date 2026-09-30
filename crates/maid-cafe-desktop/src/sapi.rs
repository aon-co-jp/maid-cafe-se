//! Windows標準の音声合成(System.Speech = SAPI5)を、PowerShell経由で使う。追加インストール不要。
//! 日本語の声(Microsoft Haruka等)は、Windowsの「言語」設定で日本語音声を入れると使える。
//! (Kotlin版`Sapi.kt`と、open-englishの`server/src/tts.rs`の実績のある方式。)

use maid_cafe_core::audio::SourceGender;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Windowsにインストールされている音声。
#[derive(Clone, Debug, PartialEq)]
pub struct SapiVoice {
    pub name: String,
    pub culture: String,
    pub gender: SourceGender,
}

impl SapiVoice {
    pub fn is_japanese(&self) -> bool {
        self.culture.eq_ignore_ascii_case("ja-JP")
    }
}

/// [`synthesize`]の結果: 区切りごとのWAV(バイト列)と、実際に使われた声。
#[derive(Debug)]
pub struct Synthesized {
    pub wavs: Vec<Vec<u8>>,
    pub voice: SapiVoice,
}

const HEADER: &str = "[Console]::OutputEncoding = [Text.Encoding]::UTF8\nAdd-Type -AssemblyName System.Speech\n";

fn list_script() -> String {
    format!(
        "{HEADER}$s = New-Object System.Speech.Synthesis.SpeechSynthesizer\n\
         $s.GetInstalledVoices() | Where-Object {{ $_.Enabled }} | ForEach-Object {{\n\
         \x20   $_.VoiceInfo.Name + \"`t\" + $_.VoiceInfo.Culture.Name + \"`t\" + $_.VoiceInfo.Gender\n}}\n"
    )
}

fn synth_script() -> String {
    format!(
        "param([string]$InFile, [string]$OutDir, [string]$Voice)\n{HEADER}\
         $s = New-Object System.Speech.Synthesis.SpeechSynthesizer\n\
         if ($Voice) {{\n    $s.SelectVoice($Voice)\n}} else {{\n\
         \x20   $ja = $s.GetInstalledVoices() | Where-Object {{ $_.Enabled -and $_.VoiceInfo.Culture.Name -eq 'ja-JP' }} | Select-Object -First 1\n\
         \x20   if (-not $ja) {{ [Console]::Error.WriteLine('no ja-JP voice'); exit 2 }}\n\
         \x20   $s.SelectVoice($ja.VoiceInfo.Name)\n}}\n\
         Write-Output (\"VOICE`t\" + $s.Voice.Name + \"`t\" + $s.Voice.Culture.Name + \"`t\" + $s.Voice.Gender)\n\
         $lines = [IO.File]::ReadAllLines($InFile, [Text.Encoding]::UTF8)\n$i = 0\n\
         foreach ($l in $lines) {{\n    $p = $l.Split(\"`t\", 2)\n    $s.Rate = [int]$p[0]\n\
         \x20   $f = Join-Path $OutDir (\"seg_\" + $i + \".wav\")\n    $s.SetOutputToWaveFile($f)\n\
         \x20   $s.Speak($p[1])\n    $s.SetOutputToNull()\n    $i++\n}}\n"
    )
}

fn gender(s: &str) -> SourceGender {
    match s.trim().to_ascii_lowercase().as_str() {
        "female" => SourceGender::Female,
        "male" => SourceGender::Male,
        _ => SourceGender::Unknown,
    }
}

/// 話速の倍率(1.0=標準)をSAPIのRate(-10..10)へ。おおむね-10が約1/3倍、+10が約3倍。
pub fn rate_step(multiplier: f64) -> i32 {
    ((10.0 * multiplier.ln() / 3f64.ln()).round() as i32).clamp(-10, 10)
}

/// Windows PowerShell 5.1は、BOMなしUTF-8のスクリプトを日本語コードページで読んでしまうので、BOMを付けて書く。
fn write_script(dir: &Path, name: &str, body: &str) -> std::io::Result<PathBuf> {
    let path = dir.join(name);
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(body.as_bytes());
    std::fs::write(&path, bytes)?;
    Ok(path)
}

fn work_dir() -> std::io::Result<PathBuf> {
    static N: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!("maid-cafe-se-sapi-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn powershell(args: &[String], timeout: Duration) -> Result<String, String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use std::process::{Command, Stdio};
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let mut child = Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass"])
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("PowerShellを起動できません: {e}"))?;
        // 標準出力/標準エラーは別スレッドで読む(パイプが詰まって固まるのを避ける)
        let mut out = child.stdout.take().ok_or("stdoutを取れません")?;
        let mut err = child.stderr.take().ok_or("stderrを取れません")?;
        let t_out = std::thread::spawn(move || {
            let mut b = Vec::new();
            let _ = std::io::Read::read_to_end(&mut out, &mut b);
            b
        });
        let t_err = std::thread::spawn(move || {
            let mut b = Vec::new();
            let _ = std::io::Read::read_to_end(&mut err, &mut b);
            b
        });
        let start = std::time::Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(s)) => break s,
                Ok(None) if start.elapsed() > timeout => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("音声合成がタイムアウトしました".into());
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(20)),
                Err(e) => return Err(format!("PowerShellの待機に失敗: {e}")),
            }
        };
        let stdout = String::from_utf8_lossy(&t_out.join().unwrap_or_default()).into_owned();
        let stderr = String::from_utf8_lossy(&t_err.join().unwrap_or_default()).into_owned();
        if !status.success() {
            return Err(format!("音声合成に失敗しました: {}", stderr.trim()));
        }
        Ok(stdout)
    }
    #[cfg(not(windows))]
    {
        let _ = (args, timeout);
        Err("この環境(Windows以外)では音声合成を使えません".into())
    }
}

/// インストール済みの音声一覧。取得できなければ空。
pub fn list_voices() -> Vec<SapiVoice> {
    let Ok(dir) = work_dir() else { return Vec::new() };
    let result = write_script(&dir, "list.ps1", &list_script())
        .map_err(|e| e.to_string())
        .and_then(|script| powershell(&["-File".into(), script.display().to_string()], Duration::from_secs(30)));
    let _ = std::fs::remove_dir_all(&dir);
    result
        .map(|out| {
            out.lines()
                .filter_map(|l| {
                    let p: Vec<&str> = l.trim_end_matches('\r').split('\t').collect();
                    (p.len() >= 3).then(|| SapiVoice { name: p[0].into(), culture: p[1].into(), gender: gender(p[2]) })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// `items`((SAPIのRate, 文章)の並び)を、1回のPowerShell起動でWAVへ合成する。
/// 声は`voice_name`、未指定なら最初の日本語音声。文章はファイル経由で渡す(コマンドラインに埋め込まない)。
pub fn synthesize(items: &[(i32, String)], voice_name: Option<&str>) -> Result<Synthesized, String> {
    if items.is_empty() {
        return Err("読み上げる文章がありません".into());
    }
    let dir = work_dir().map_err(|e| e.to_string())?;
    let result = (|| {
        let script = write_script(&dir, "synth.ps1", &synth_script()).map_err(|e| e.to_string())?;
        let input = dir.join("in.txt");
        let lines: Vec<String> = items
            .iter()
            .map(|(r, t)| format!("{r}\t{}", t.replace(['\t', '\r', '\n'], " ")))
            .collect();
        std::fs::write(&input, lines.join("\r\n").as_bytes()).map_err(|e| e.to_string())?;
        let out = powershell(
            &[
                "-File".into(),
                script.display().to_string(),
                "-InFile".into(),
                input.display().to_string(),
                "-OutDir".into(),
                dir.display().to_string(),
                "-Voice".into(),
                voice_name.unwrap_or("").to_string(),
            ],
            Duration::from_secs(120),
        )?;
        let v: Vec<&str> = out
            .lines()
            .map(|l| l.trim_end_matches('\r'))
            .find(|l| l.starts_with("VOICE\t"))
            .ok_or("使った声を取得できません")?
            .split('\t')
            .collect();
        let voice = SapiVoice {
            name: v.get(1).unwrap_or(&"").to_string(),
            culture: v.get(2).unwrap_or(&"").to_string(),
            gender: gender(v.get(3).unwrap_or(&"")),
        };
        let mut wavs = Vec::new();
        for i in 0..items.len() {
            let bytes = std::fs::read(dir.join(format!("seg_{i}.wav"))).map_err(|e| format!("WAVができていません(seg_{i}): {e}"))?;
            if bytes.len() < 100 {
                return Err(format!("WAVが空です(seg_{i})"));
            }
            wavs.push(bytes);
        }
        Ok(Synthesized { wavs, voice })
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_step_maps_multiplier_to_sapi_range() {
        assert_eq!(0, rate_step(1.0));
        assert_eq!(10, rate_step(3.0));
        assert_eq!(-10, rate_step(1.0 / 3.0));
        assert_eq!(10, rate_step(100.0));
        assert_eq!(-10, rate_step(0.001));
        assert!(rate_step(0.85) < 0 && rate_step(1.1) > 0);
    }

    #[test]
    fn scripts_are_well_formed() {
        let s = synth_script();
        assert!(s.contains("param([string]$InFile") && s.contains("SetOutputToWaveFile") && s.contains("SelectVoice"));
        assert!(list_script().contains("GetInstalledVoices"));
    }

    #[cfg(windows)]
    #[test]
    fn real_sapi_lists_and_synthesizes_japanese_when_available() {
        let voices = list_voices();
        if !voices.iter().any(|v| v.is_japanese()) {
            eprintln!("SKIP: 日本語のWindows音声が無い");
            return;
        }
        let out = synthesize(&[(0, "こんにちは".into()), (-2, "ご主人様、おかえりなさいませ".into())], None).expect("合成できる");
        assert_eq!(2, out.wavs.len());
        assert!(out.voice.is_japanese());
        for w in &out.wavs {
            assert_eq!(b"RIFF", &w[0..4]);
        }
        assert!(out.wavs[1].len() > out.wavs[0].len(), "長い文のほうがWAVも長い");
    }
}
