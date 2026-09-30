//! モデルを使うテスト。モデルが`MAID_CAFE_MODEL_DIR`(既定: リポジトリの`target/models`)に無ければ、スキップする
//! (CIやオフライン環境でも、ネットワーク無しで`cargo test`が通るように)。

use maid_cafe_core::audio::resampler::resample_poly;
use maid_cafe_core::audio::wav::Pcm;
use maid_cafe_enhance::{enhance_speech, models_present, Lavasr, OUT_SR};
use std::path::PathBuf;

fn model_dir() -> PathBuf {
    std::env::var("MAID_CAFE_MODEL_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/models")))
}

/// 声のような疑似音声(倍音つき)を22.05kHzで作る。
fn voice(seconds: f64) -> Pcm {
    let sr = 22_050u32;
    let x: Vec<f32> = (0..(sr as f64 * seconds) as usize)
        .map(|i| {
            let t = i as f64 / sr as f64;
            let env = 0.6 + 0.4 * (2.0 * std::f64::consts::PI * 3.0 * t).sin();
            let v: f64 = (1..=12).map(|h| (2.0 * std::f64::consts::PI * 180.0 * h as f64 * t).sin() / h as f64).sum();
            (env * v * 0.2) as f32
        })
        .collect();
    Pcm::new(x, sr)
}

#[test]
fn enhance_keeps_the_input_band_and_the_length() {
    let dir = model_dir();
    if !models_present(&dir) {
        eprintln!("SKIP: モデルが {} に無い", dir.display());
        return;
    }
    let model = Lavasr::load(&dir).expect("モデルを読み込める");
    let pcm = voice(2.0);
    // カットオフを明示(この疑似音声は4kHzより上が欠けているとみなす)
    let out = enhance_speech(&model, &pcm, Some(4000.0)).expect("拡張できる");
    assert_eq!(OUT_SR as u32, out.sample_rate);
    let expected = (pcm.samples.len() as f64 * OUT_SR as f64 / pcm.sample_rate as f64).round() as usize;
    assert!((out.samples.len() as i64 - expected as i64).abs() <= 2, "長さ {} vs {expected}", out.samples.len());
    assert!(out.samples.iter().all(|v| v.is_finite() && v.abs() < 2.0), "発散/クリップ");
    // 入力の帯域は変えない: 48kHzへ上げただけの入力と、低域(〜3kHz)がほぼ一致する
    let up = resample_poly(&pcm.samples, OUT_SR / 150, pcm.sample_rate as usize / 150);
    let n = up.len().min(out.samples.len());
    // 48kHz→6kHzへ下げる(Kaiser窓のアンチエイリアス)ことで、3kHz以下だけが残る
    let low = |x: &[f32]| resample_poly(&x[..n], 1, 8);
    let (a, b) = (low(&up), low(&out.samples));
    let err = a.iter().zip(&b).map(|(x, y)| ((x - y) as f64).powi(2)).sum::<f64>() / a.iter().map(|x| (*x as f64).powi(2)).sum::<f64>();
    assert!(err.sqrt() < 0.02, "低域が変わっている: 相対誤差 {}", err.sqrt());
}

#[test]
fn a_band_complete_voice_is_returned_unchanged() {
    let dir = model_dir();
    if !models_present(&dir) {
        eprintln!("SKIP: モデルが {} に無い", dir.display());
        return;
    }
    let model = Lavasr::load(&dir).unwrap();
    // 48kHzの全帯域に音がある(白色ノイズ)入力には拡張は不要 → 何も変えずに返す
    // (22.05kHzで作られたTTSの声は、11kHzより上が最初から無いので拡張の対象になる。それは正しい動作)
    let mut s = 1u64;
    let x: Vec<f32> = (0..OUT_SR * 2)
        .map(|_| {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (((s >> 33) as f64 / (1u64 << 31) as f64 * 2.0 - 1.0) * 0.2) as f32
        })
        .collect();
    let out = enhance_speech(&model, &Pcm::new(x.clone(), OUT_SR as u32), None).unwrap();
    assert_eq!(x, out.samples, "帯域が欠けていない声は、そのまま返るはず");
}

#[test]
fn a_22khz_voice_gets_its_missing_air_but_stays_bounded() {
    let dir = model_dir();
    if !models_present(&dir) {
        eprintln!("SKIP: モデルが {} に無い", dir.display());
        return;
    }
    let model = Lavasr::load(&dir).unwrap();
    let pcm = voice(2.0);
    let out = enhance_speech(&model, &pcm, None).unwrap(); // カットオフは声のロールオフから自動で決まる(または不要と判断される)
    assert_eq!(OUT_SR as u32, out.sample_rate);
    let peak = out.samples.iter().fold(0f32, |m, v| m.max(v.abs()));
    let in_peak = pcm.samples.iter().fold(0f32, |m, v| m.max(v.abs()));
    assert!(peak < in_peak * 1.5 + 0.05, "足した高域でピークが大きく増えた: {peak} vs 入力 {in_peak}");
}
