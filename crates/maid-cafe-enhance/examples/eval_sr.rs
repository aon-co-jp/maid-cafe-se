//! 帯域拡張の自己教師あり評価: 本物のTTS音声(高域まで存在する部分が「正解」)を人工的に帯域制限し、拡張して、
//! 元の音声にどれだけ近づくかを対数スペクトル距離(LSD、dB)で測る。
//! 使い方: `cargo run --release -p maid-cafe-enhance --example eval_sr -- <モデル置き場> <wav> [制限周波数Hz=6000]`
//!
//! **注意**: LSDはスペクトル包絡の近さの指標で、聴感品質そのものではない。

use maid_cafe_core::audio::fft::Fft;
use maid_cafe_core::audio::resampler::resample_poly;
use maid_cafe_core::audio::wav::parse_wav;
use maid_cafe_enhance::{extend_channel, Lavasr, OUT_SR};
use std::f64::consts::PI;
use std::path::PathBuf;

fn gcd(a: usize, b: usize) -> usize {
    if b == 0 { a } else { gcd(b, a % b) }
}

fn resample(x: &[f32], from: usize, to: usize) -> Vec<f32> {
    let g = gcd(from, to);
    resample_poly(x, to / g, from / g)
}

/// フレームごとの(対数振幅スペクトル(dB), フレームのパワー)。`[frame]`。全フレームを返す(フレームの対応を崩さないため、
/// 無音の除外は呼び出し側が「正解側のパワー」で決める)。
fn log_spec(x: &[f32]) -> Vec<(Vec<f64>, f64)> {
    let n = 2048;
    let fft = Fft::new(n);
    let win: Vec<f64> = (0..n).map(|i| 0.5 - 0.5 * (2.0 * PI * i as f64 / n as f64).cos()).collect();
    let mut out = Vec::new();
    let mut s = 0;
    while s + n <= x.len() {
        let mut re: Vec<f64> = (0..n).map(|i| x[s + i] as f64 * win[i]).collect();
        let mut im = vec![0f64; n];
        fft.forward(&mut re, &mut im);
        let e: f64 = (0..n / 2).map(|k| re[k] * re[k] + im[k] * im[k]).sum();
        out.push(((0..=n / 2).map(|k| 20.0 * ((re[k] * re[k] + im[k] * im[k]).sqrt() + 1e-9).log10()).collect(), e));
        s += 512;
    }
    out
}

/// 帯域`[lo, hi]`Hzの、フレームごとのRMS差(dB)の平均。
fn lsd(a: &[f32], b: &[f32], lo: f64, hi: f64) -> f64 {
    let n = a.len().min(b.len());
    let (sa, sb) = (log_spec(&a[..n]), log_spec(&b[..n]));
    let bin = OUT_SR as f64 / 2048.0;
    let (k0, k1) = ((lo / bin) as usize, (hi / bin) as usize);
    let frames = sa.len().min(sb.len());
    // 正解(b)側で無音に近いフレームは除く(そこでは比べる意味が無い)
    let idx: Vec<usize> = (0..frames).filter(|&f| sb[f].1 > 1e-3).collect();
    let total: f64 = idx
        .iter()
        .map(|&f| ((k0..k1).map(|k| (sa[f].0[k] - sb[f].0[k]).powi(2)).sum::<f64>() / (k1 - k0) as f64).sqrt())
        .sum();
    total / idx.len().max(1) as f64
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let pcm = parse_wav(&std::fs::read(&args[1]).expect("読めません")).expect("16bit PCMのWAVではありません");
    let limit: f64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(6000.0);
    let model = Lavasr::load(&PathBuf::from(&args[0])).expect("モデルを読み込めません");

    let orig = resample(&pcm.samples, pcm.sample_rate as usize, OUT_SR);
    // 人工的な帯域制限: 制限周波数の2倍のレートまで下げて(Kaiser窓のアンチエイリアス)、48kHzへ戻す
    let mid = (limit * 2.0) as usize;
    let degraded = resample(&resample(&orig, OUT_SR, mid), mid, OUT_SR);
    let cutoff = (limit * 0.92) as f32; // 制限の少し手前から拡張を始める
    let enhanced = extend_channel(&model, &degraded, Some(cutoff), 12.0).expect("拡張に失敗");

    let (lo, hi) = (limit, (pcm.sample_rate as f64 / 2.0 * 0.86).min(10_000.0)); // 正解が存在する帯域(元のナイキスト付近は除く)
    let (d0, d1) = (lsd(&degraded, &orig, lo, hi), lsd(&enhanced, &orig, lo, hi));
    println!("帯域制限 {limit} Hz、評価帯域 {lo:.0}〜{hi:.0} Hz(正解が存在する帯域)");
    println!("LSD 帯域制限のみ: {d0:.2} dB → AI拡張: {d1:.2} dB  ({})", if d1 < d0 { "改善" } else { "改善せず" });
    let (l0, l1) = (lsd(&degraded, &orig, 100.0, limit * 0.9), lsd(&enhanced, &orig, 100.0, limit * 0.9));
    println!("低域(100〜{:.0} Hz)のLSD: 帯域制限のみ {l0:.3} dB / AI拡張 {l1:.3} dB(=入力の帯域を変えていないことの確認)", limit * 0.9);
}
