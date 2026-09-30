//! 同梱音源を純粋な数式で生成する(外部音源を一切使わないため著作権の問題が無い)。
//! 生成物は本プロジェクトが CC0 1.0 (パブリックドメイン相当) として公開する。
//!
//! 使い方: `cargo run --release -p gen-sounds [出力フォルダ]`
//! 既定の出力先は `app/src/main/res/raw`(リポジトリのルートから実行した場合)。

use std::f64::consts::PI;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

const SR: u32 = 22_050;

/// 減衰する倍音つきの音。`harm`は倍音の強さ(基音から順に)。
fn tone(freq: f64, dur: f64, vol: f64, decay: f64, harm: &[f64]) -> Vec<f64> {
    let n = (SR as f64 * dur) as usize;
    let total: f64 = harm.iter().sum();
    (0..n)
        .map(|i| {
            let env = (-decay * i as f64 / n as f64).exp();
            let s: f64 = harm
                .iter()
                .enumerate()
                .map(|(k, h)| h * (2.0 * PI * freq * (k as f64 + 1.0) * i as f64 / SR as f64).sin())
                .sum();
            vol * env * s / total
        })
        .collect()
}

fn silence(dur: f64) -> Vec<f64> {
    vec![0.0; (SR as f64 * dur) as usize]
}

/// 16bit・モノラルのWAVを書く。
fn write_wav(dir: &PathBuf, name: &str, samples: &[f64]) -> std::io::Result<()> {
    let data_len = (samples.len() * 2) as u32;
    let mut b: Vec<u8> = Vec::with_capacity(44 + samples.len() * 2);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes()); // PCM
    b.extend_from_slice(&1u16.to_le_bytes()); // mono
    b.extend_from_slice(&SR.to_le_bytes());
    b.extend_from_slice(&(SR * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        // 旧Python版と同じく、0へ向かって切り捨てる
        b.extend_from_slice(&((s.clamp(-1.0, 1.0) * 30000.0) as i16).to_le_bytes());
    }
    fs::create_dir_all(dir)?;
    fs::File::create(dir.join(format!("{name}.wav")))?.write_all(&b)
}

/// チャイム: 3音の鐘(倍音つき、ゆっくり減衰)
fn chime() -> Vec<f64> {
    [784.0, 988.0, 1319.0]
        .iter()
        .flat_map(|&f| tone(f, 0.9, 0.6, 4.0, &[1.0, 0.4, 0.2]))
        .collect()
}

/// アラーム: ピピピピ を2セット
fn alarm() -> Vec<f64> {
    let mut v = Vec::new();
    for _ in 0..2 {
        for _ in 0..4 {
            v.extend(tone(1760.0, 0.12, 0.6, 1.0, &[1.0]));
            v.extend(silence(0.08));
        }
        v.extend(silence(0.5));
    }
    v
}

/// メロディ: やさしい上昇アルペジオ(C長調)
fn melody() -> Vec<f64> {
    let mut v: Vec<f64> = [523.25, 659.25, 783.99, 1046.5, 783.99, 659.25, 523.25]
        .iter()
        .flat_map(|&f| tone(f, 0.45, 0.5, 3.0, &[1.0, 0.3]))
        .collect();
    v.extend(silence(0.6));
    v
}

fn main() -> std::io::Result<()> {
    let dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("app/src/main/res/raw"));
    write_wav(&dir, "chime", &chime())?;
    write_wav(&dir, "alarm", &alarm())?;
    write_wav(&dir, "melody", &melody())?;
    println!("ok: {}", dir.display());
    Ok(())
}
