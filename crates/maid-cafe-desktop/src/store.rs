//! 設定の保存。`%APPDATA%\maid-cafe-se\`(環境変数`MAID_CAFE_SE_DATA`で変更可、テスト用)。
//!
//! Kotlin版(旧Windows版)と**同じ形式**: アラームは`alarms.txt`(1行1件の`key=value&...`、core::codec)、
//! 設定は`settings.properties`(Javaのpropertiesと同じ`key=value`)。旧版のデータをそのまま読める。

use maid_cafe_core::codec;
use maid_cafe_core::{AlarmEntry, VoiceStyle};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub struct Store {
    dir: PathBuf,
    /// 書き込みの競合を避ける(アラーム保存・設定保存・ログを1本化)
    lock: Mutex<()>,
}

impl Store {
    /// 既定の保存先: `MAID_CAFE_SE_DATA`、無ければ`%APPDATA%\maid-cafe-se`。
    pub fn default_dir() -> PathBuf {
        if let Some(d) = std::env::var_os("MAID_CAFE_SE_DATA") {
            return PathBuf::from(d);
        }
        let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
        base.join("maid-cafe-se")
    }

    pub fn new(dir: PathBuf) -> std::io::Result<Self> {
        std::fs::create_dir_all(&dir)?;
        Ok(Store { dir, lock: Mutex::new(()) })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn alarms_file(&self) -> PathBuf {
        self.dir.join("alarms.txt")
    }

    fn settings_file(&self) -> PathBuf {
        self.dir.join("settings.properties")
    }

    pub fn load_entries(&self) -> Vec<AlarmEntry> {
        std::fs::read_to_string(self.alarms_file()).map(|s| codec::decode_all(&s)).unwrap_or_default()
    }

    pub fn save_entries(&self, list: &[AlarmEntry]) -> std::io::Result<()> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        write_atomically(&self.alarms_file(), &codec::encode_all(list))
    }

    fn load_props(&self) -> HashMap<String, String> {
        std::fs::read_to_string(self.settings_file()).map(|s| parse_props(&s)).unwrap_or_default()
    }

    /// 声の元にするWindows音声の指定(未指定は`None`=自動)。
    pub fn voice_pref(&self, style: VoiceStyle) -> Option<String> {
        self.load_props().get(&format!("voice_{}", style.name())).filter(|v| !v.is_empty()).cloned()
    }

    pub fn save_voice_pref(&self, style: VoiceStyle, name: Option<&str>) -> std::io::Result<()> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut props = self.load_props();
        let key = format!("voice_{}", style.name());
        match name {
            Some(n) => {
                props.insert(key, n.to_string());
            }
            None => {
                props.remove(&key);
            }
        }
        write_atomically(&self.settings_file(), &render_props(&props))
    }

    /// `app.log`への簡易ログ(1MBを超えたら作り直す)。失敗は無視する。
    pub fn log(&self, msg: &str) {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let path = self.dir.join("app.log");
        if std::fs::metadata(&path).map(|m| m.len() > 1_000_000).unwrap_or(false) {
            let _ = std::fs::remove_file(&path);
        }
        let line = format!("{} {msg}\n", chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%.3f"));
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = f.write_all(line.as_bytes());
        }
    }

    /// 起動中のサーバーのポート(2つ目のプロセスが、起動済みの画面を開くために読む)。
    pub fn write_port(&self, port: u16) -> std::io::Result<()> {
        std::fs::write(self.dir.join("port.txt"), port.to_string())
    }

    pub fn read_port(&self) -> Option<u16> {
        std::fs::read_to_string(self.dir.join("port.txt")).ok()?.trim().parse().ok()
    }
}

/// 途中で電源が落ちても壊れないよう、一時ファイルへ書いてから置き換える。
fn write_atomically(target: &Path, text: &str) -> std::io::Result<()> {
    let tmp = target.with_extension("tmp");
    std::fs::write(&tmp, text.as_bytes())?;
    std::fs::rename(&tmp, target)
}

/// Javaのpropertiesの最小サブセット(`key=value`、`#`/`!`コメント、`\\` `\:` `\=` `\uXXXX`のエスケープ)。
pub fn parse_props(s: &str) -> HashMap<String, String> {
    let mut m = HashMap::new();
    for line in s.lines() {
        let line = line.trim_start();
        if line.is_empty() || line.starts_with('#') || line.starts_with('!') {
            continue;
        }
        if let Some(i) = line.find(['=', ':']) {
            m.insert(unescape(line[..i].trim()), unescape(line[i + 1..].trim_start()));
        }
    }
    m
}

fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('u') => {
                let hex: String = it.by_ref().take(4).collect();
                if let Some(ch) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    out.push(ch);
                }
            }
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

fn render_props(m: &HashMap<String, String>) -> String {
    let mut keys: Vec<&String> = m.keys().collect();
    keys.sort();
    let esc = |s: &str| s.replace('\\', "\\\\").replace('\n', "\\n").replace('=', "\\=").replace(':', "\\:");
    keys.iter().map(|k| format!("{}={}\n", esc(k), esc(&m[*k]))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use maid_cafe_core::{AlarmKind, Recurrence, Schedule};

    fn temp_store(tag: &str) -> Store {
        let dir = std::env::temp_dir().join(format!("mcs-store-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Store::new(dir).unwrap()
    }

    #[test]
    fn alarms_round_trip_and_survive_a_missing_or_broken_file() {
        let s = temp_store("alarms");
        assert!(s.load_entries().is_empty()); // ファイルが無い
        let t = chrono::NaiveTime::from_hms_opt(7, 30, 0).unwrap();
        let mut e = AlarmEntry::new("a1", "起床", Schedule::new(Recurrence::weekdays(), t), AlarmKind::Speech);
        e.phrases = vec!["okite".into(), "okaeri".into()];
        e.harmony = true;
        s.save_entries(&[e.clone()]).unwrap();
        assert_eq!(vec![e], s.load_entries());
        std::fs::write(s.dir().join("alarms.txt"), "こわれた行\n").unwrap();
        assert!(s.load_entries().is_empty()); // 壊れた行は読み飛ばす
        let _ = std::fs::remove_dir_all(s.dir());
    }

    #[test]
    fn voice_prefs_round_trip_with_special_characters() {
        let s = temp_store("prefs");
        assert_eq!(None, s.voice_pref(VoiceStyle::Maid));
        s.save_voice_pref(VoiceStyle::Maid, Some("Microsoft Haruka Desktop")).unwrap();
        s.save_voice_pref(VoiceStyle::DeepMale, Some("a=b:c\\d")).unwrap();
        assert_eq!(Some("Microsoft Haruka Desktop".to_string()), s.voice_pref(VoiceStyle::Maid));
        assert_eq!(Some("a=b:c\\d".to_string()), s.voice_pref(VoiceStyle::DeepMale));
        s.save_voice_pref(VoiceStyle::Maid, None).unwrap();
        assert_eq!(None, s.voice_pref(VoiceStyle::Maid));
        assert_eq!(Some("a=b:c\\d".to_string()), s.voice_pref(VoiceStyle::DeepMale)); // 他の設定は残る
        let _ = std::fs::remove_dir_all(s.dir());
    }

    #[test]
    fn reads_properties_written_by_the_kotlin_version() {
        // Java の Properties.store が書く形式(コメント行+エスケープ)
        let m = parse_props("#Wed Sep 30 13:00:00 JST 2026\nvoice_MAID=Microsoft Haruka Desktop\nvoice_DEEP_MALE=\\u65e5\\u672c\n");
        assert_eq!(Some("Microsoft Haruka Desktop"), m.get("voice_MAID").map(|s| s.as_str()));
        assert_eq!(Some("日本"), m.get("voice_DEEP_MALE").map(|s| s.as_str()));
    }

    #[test]
    fn port_file_round_trips() {
        let s = temp_store("port");
        assert_eq!(None, s.read_port());
        s.write_port(47411).unwrap();
        assert_eq!(Some(47411), s.read_port());
        let _ = std::fs::remove_dir_all(s.dir());
    }
}
