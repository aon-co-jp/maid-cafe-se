//! WAVの周波数帯ごとの平均レベル(dB)を表示する調査用ツール。
//! 使い方: `cargo run --release -p maid-cafe-core --example spectrum -- <file.wav>`

use maid_cafe_core::audio::fft::Fft;
use maid_cafe_core::audio::wav::parse_wav;
use std::f64::consts::PI;

fn main() {
    let path = std::env::args().nth(1).expect("WAVファイルを指定してください");
    let pcm = parse_wav(&std::fs::read(&path).expect("読めません")).expect("16bit PCMのWAVではありません");
    let n = 2048;
    let fft = Fft::new(n);
    let win: Vec<f64> = (0..n).map(|i| 0.5 - 0.5 * (2.0 * PI * i as f64 / n as f64).cos()).collect();
    let mut acc = vec![0f64; n / 2 + 1];
    let mut frames = 0;
    let mut start = 0;
    while start + n <= pcm.samples.len() {
        let mut re: Vec<f64> = (0..n).map(|i| pcm.samples[start + i] as f64 * win[i]).collect();
        let mut im = vec![0f64; n];
        fft.forward(&mut re, &mut im);
        // 無音に近いフレームは平均から外す
        let e: f64 = (0..=n / 2).map(|k| re[k] * re[k] + im[k] * im[k]).sum();
        if e > 1e-3 {
            for k in 0..=n / 2 {
                acc[k] += re[k] * re[k] + im[k] * im[k];
            }
            frames += 1;
        }
        start += n / 2;
    }
    let bin = pcm.sample_rate as f64 / n as f64;
    println!("{path}: {} Hz, {:.2} s, 有効フレーム {frames}", pcm.sample_rate, pcm.seconds());
    let step = 1000.0;
    let mut f = 0.0;
    while f < pcm.sample_rate as f64 / 2.0 {
        let (lo, hi) = ((f / bin) as usize, (((f + step) / bin) as usize).min(n / 2));
        let p: f64 = acc[lo..hi].iter().sum::<f64>() / ((hi - lo).max(1) as f64 * frames.max(1) as f64);
        println!("{:>5.0}-{:>5.0} Hz: {:>6.1} dB", f, f + step, 10.0 * (p + 1e-15).log10());
        f += step;
    }
}
