//! `maid-cafe-se --selftest [出力先] [--play]`: 画面なしで、読み上げ(SAPI→声の加工→連結)を1回通して確かめる。
//! 声質ごとのWAVを出力先へ書き、長さ・音量・使った声を表示する。`--play`で実際に鳴らす。

use crate::player::Player;
use crate::store::Store;
use chrono::Local;
use maid_cafe_core::audio::wav_bytes;
use maid_cafe_core::{Occurrence, SpeechText, VoiceStyle};
use std::path::PathBuf;
use std::sync::Arc;

fn occurrence(voice: VoiceStyle, harmony: bool) -> Occurrence {
    let ids: Vec<String> = ["okite", "fight"].iter().map(|s| s.to_string()).collect();
    let base = SpeechText::alarm(if voice == VoiceStyle::Maid { "薬を飲む" } else { "会議" }, voice);
    Occurrence {
        time: Local::now().naive_local(),
        key: "selftest".into(),
        title: "selftest".into(),
        sound_id: None,
        speech: SpeechText::with_phrases(Some(&base), &ids),
        voice,
        harmony,
        segments: SpeechText::segments(Some(&base), &ids),
    }
}

pub fn run(args: &[String]) -> i32 {
    let play = args.iter().any(|a| a == "--play");
    let out: PathBuf = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("maid-cafe-selftest"));
    if let Err(e) = std::fs::create_dir_all(&out) {
        eprintln!("出力先を作れません: {e}");
        return 2;
    }
    let store = match Store::new(out.join("data")) {
        Ok(s) => Arc::new(s),
        Err(e) => {
            eprintln!("保存先を作れません: {e}");
            return 2;
        }
    };
    let player = Player::new(store);
    let mut failed = false;
    let mut report = String::new();
    let mut say = |line: String| {
        println!("{line}");
        report.push_str(&line);
        report.push('\n');
    };
    for (name, voice, harmony) in [("maid", VoiceStyle::Maid, false), ("maid-harmony", VoiceStyle::Maid, true), ("deep-male", VoiceStyle::DeepMale, false)] {
        let started = std::time::Instant::now();
        match player.render(&occurrence(voice, harmony), None) {
            Ok((pcm, v)) => {
                let rms = (pcm.samples.iter().map(|x| (*x as f64).powi(2)).sum::<f64>() / pcm.samples.len().max(1) as f64).sqrt();
                let peak = pcm.samples.iter().fold(0f32, |m, x| m.max(x.abs()));
                let path = out.join(format!("{name}.wav"));
                match std::fs::write(&path, wav_bytes(&pcm)) {
                    Ok(()) => say(format!(
                        "OK  {name}: {:.2}秒 rms={rms:.3} peak={peak:.3} 声='{}' ({}) 合成{}ms -> {}",
                        pcm.seconds(),
                        v.name,
                        v.culture,
                        started.elapsed().as_millis(),
                        path.display()
                    )),
                    Err(e) => {
                        say(format!("NG  {name}: 書き込めません: {e}"));
                        failed = true;
                    }
                }
            }
            Err(e) => {
                say(format!("NG  {name}: {e}"));
                failed = true;
            }
        }
    }
    // 画面なし(GUIサブシステム)のexeはコンソールへ出せないことがあるので、結果をファイルにも残す(ビルドスクリプトが読む)
    let _ = std::fs::write(out.join("report.txt"), report);
    if play && !failed {
        player.play(&[occurrence(VoiceStyle::Maid, true)], &|_| None);
    }
    i32::from(failed)
}
