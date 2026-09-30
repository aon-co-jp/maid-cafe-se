//! TTS出力の後処理。エンジンの`setPitch`に頼らず、ピッチと声の太さ(フォルマント)を自前で変える。
//!  - 高品質リサンプラ([`super::resampler`])で音程とフォルマントを同時に動かし(=声道の大きさが変わった声になる)、
//!    WSOLAで元の長さに戻す。
//!  - 「ハモり」は同じ音声から音程違い(長3度上)の声を作って重ねるため、2人のタイミングが完全に揃う。

use super::resampler::speed_up;
use super::wav::Pcm;
use crate::model::VoiceStyle;
use std::f64::consts::PI;

/// TTSエンジンが出した声の性別(端末の音声名からの推定)。不明は女性扱い(多くの端末の既定が女性声のため)。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceGender {
    Female,
    Male,
    Unknown,
}

/// 声ごとの変換レシピ。`pitch_ratio`は音程(と声の太さ)の倍率。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Recipe {
    pub pitch_ratio: f64,
    pub low_shelf_db: f64,
    pub high_shelf_db: f64,
}

pub fn recipe(style: VoiceStyle, source: SourceGender) -> Recipe {
    let male = source == SourceGender::Male;
    match style {
        VoiceStyle::Maid => {
            if male {
                Recipe { pitch_ratio: 1.45, low_shelf_db: 0.0, high_shelf_db: 3.0 }
            } else {
                Recipe { pitch_ratio: 1.12, low_shelf_db: 0.0, high_shelf_db: 3.0 }
            }
        }
        VoiceStyle::DeepMale => {
            if male {
                Recipe { pitch_ratio: 0.86, low_shelf_db: 4.0, high_shelf_db: -1.0 }
            } else {
                Recipe { pitch_ratio: 0.72, low_shelf_db: 5.0, high_shelf_db: -2.0 }
            }
        }
    }
}

/// 平均律の長3度(4半音)。
pub fn major_third() -> f64 {
    2f64.powf(4.0 / 12.0)
}

/// `pitch_mul`はセリフごとの抑揚(音程への追加倍率)。
pub fn render(pcm: &Pcm, style: VoiceStyle, source: SourceGender, harmony: bool, pitch_mul: f64) -> Pcm {
    let mut r = recipe(style, source);
    r.pitch_ratio *= pitch_mul;
    let sr = pcm.sample_rate;
    let x = high_pass(&pcm.samples, sr, 70.0);
    let voice = |ratio: f64| -> Vec<f32> {
        let mut v = pitch_shift(&x, sr, ratio);
        if r.low_shelf_db != 0.0 {
            v = low_shelf(&v, sr, 180.0, r.low_shelf_db);
        }
        if r.high_shelf_db != 0.0 {
            v = high_shelf(&v, sr, 4000.0, r.high_shelf_db);
        }
        v
    };
    let a = voice(r.pitch_ratio);
    let out = if harmony {
        let b = voice(r.pitch_ratio * major_third());
        let delay = (sr as f64 * 0.018) as usize; // 18msずらして「別の2人」の厚みを出す
        let mut mixed = vec![0f32; a.len().max(b.len() + delay)];
        for (i, v) in a.iter().enumerate() {
            mixed[i] += v * 0.75;
        }
        for (i, v) in b.iter().enumerate() {
            mixed[i + delay] += v * 0.6;
        }
        mixed
    } else {
        a
    };
    let mut y = normalize_loudness(&trim_silence(&out, sr, -50.0, 40), 0.18);
    fade(&mut y, sr);
    Pcm::new(y, sr)
}

/// 長さを保ったまま音程(とフォルマント)を`ratio`倍にする。
pub fn pitch_shift(x: &[f32], sr: u32, ratio: f64) -> Vec<f32> {
    if (ratio - 1.0).abs() < 0.005 || x.is_empty() {
        return x.to_vec();
    }
    let y = speed_up(x, ratio);
    wsola(&y, x.len() as f64 / y.len() as f64, sr, Some(x.len()))
}

/// WSOLAによるタイムストレッチ(出力長 = 入力長 x `alpha`、`target_length`指定時はその長さ)。音程は変えない。
pub fn wsola(x: &[f32], alpha: f64, sr: u32, target_length: Option<usize>) -> Vec<f32> {
    let n = ((sr as f64 * 0.030) as usize / 2 * 2).max(128);
    let hs = n / 2;
    let tol = (sr as f64 * 0.010) as usize;
    let out_len = target_length.unwrap_or((x.len() as f64 * alpha) as usize);
    let mut out = vec![0f32; out_len];
    if x.len() < n * 2 || out_len == 0 {
        let m = x.len().min(out_len);
        out[..m].copy_from_slice(&x[..m]);
        return out;
    }
    let win: Vec<f32> = (0..n).map(|i| (0.5 - 0.5 * (2.0 * PI * i as f64 / n as f64).cos()) as f32).collect();
    let mut acc = vec![0f32; out_len + n];
    let mut norm = vec![0f32; out_len + n];
    let mut add = |pos: usize, out_pos: usize| {
        for i in 0..n {
            acc[out_pos + i] += x[pos + i] * win[i];
            norm[out_pos + i] += win[i];
        }
    };
    let mut prev = 0usize;
    add(0, 0);
    let mut k = 1usize;
    while k * hs < out_len {
        let nominal = ((k * hs) as f64 / alpha) as usize;
        let lo = nominal.saturating_sub(tol);
        let hi = (x.len() - n).min(nominal + tol);
        if lo > hi {
            break;
        }
        let tmpl = prev + hs;
        let mut best = nominal.max(lo).min(hi);
        if tmpl + n <= x.len() {
            let mut best_score = f64::NEG_INFINITY;
            for c in lo..=hi {
                let (mut dot, mut e) = (0f64, 1e-9f64);
                let mut i = 0;
                while i < n {
                    // 4サンプル間引きで相関を計算(速度優先、位相合わせには十分)
                    let v = x[c + i] as f64;
                    dot += v * x[tmpl + i] as f64;
                    e += v * v;
                    i += 4;
                }
                let score = dot / e.sqrt();
                if score > best_score {
                    best_score = score;
                    best = c;
                }
            }
        }
        add(best, k * hs);
        prev = best;
        k += 1;
    }
    for i in 0..out_len {
        out[i] = if norm[i] > 1e-3 { acc[i] / norm[i] } else { 0.0 };
    }
    out
}

struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

impl Biquad {
    fn new(b0: f64, b1: f64, b2: f64, a0: f64, a1: f64, a2: f64) -> Self {
        Biquad { b0: (b0 / a0) as f32, b1: (b1 / a0) as f32, b2: (b2 / a0) as f32, a1: (a1 / a0) as f32, a2: (a2 / a0) as f32 }
    }

    fn process(&self, x: &[f32]) -> Vec<f32> {
        let mut y = vec![0f32; x.len()];
        let (mut x1, mut x2, mut y1, mut y2) = (0f32, 0f32, 0f32, 0f32);
        for i in 0..x.len() {
            let v = self.b0 * x[i] + self.b1 * x1 + self.b2 * x2 - self.a1 * y1 - self.a2 * y2;
            x2 = x1;
            x1 = x[i];
            y2 = y1;
            y1 = v;
            y[i] = v;
        }
        y
    }
}

pub fn high_pass(x: &[f32], sr: u32, freq: f64) -> Vec<f32> {
    let w = 2.0 * PI * freq / sr as f64;
    let alpha = w.sin() / (2.0 * 0.7071);
    let c = w.cos();
    Biquad::new((1.0 + c) / 2.0, -(1.0 + c), (1.0 + c) / 2.0, 1.0 + alpha, -2.0 * c, 1.0 - alpha).process(x)
}

pub fn low_shelf(x: &[f32], sr: u32, freq: f64, gain_db: f64) -> Vec<f32> {
    let a = 10f64.powf(gain_db / 40.0);
    let w = 2.0 * PI * freq / sr as f64;
    let c = w.cos();
    let al = w.sin() / 2.0 * 2f64.sqrt();
    let sa = 2.0 * a.sqrt() * al;
    Biquad::new(
        a * ((a + 1.0) - (a - 1.0) * c + sa),
        2.0 * a * ((a - 1.0) - (a + 1.0) * c),
        a * ((a + 1.0) - (a - 1.0) * c - sa),
        (a + 1.0) + (a - 1.0) * c + sa,
        -2.0 * ((a - 1.0) + (a + 1.0) * c),
        (a + 1.0) + (a - 1.0) * c - sa,
    )
    .process(x)
}

pub fn high_shelf(x: &[f32], sr: u32, freq: f64, gain_db: f64) -> Vec<f32> {
    let a = 10f64.powf(gain_db / 40.0);
    let w = 2.0 * PI * freq / sr as f64;
    let c = w.cos();
    let al = w.sin() / 2.0 * 2f64.sqrt();
    let sa = 2.0 * a.sqrt() * al;
    Biquad::new(
        a * ((a + 1.0) + (a - 1.0) * c + sa),
        -2.0 * a * ((a - 1.0) + (a + 1.0) * c),
        a * ((a + 1.0) + (a - 1.0) * c - sa),
        (a + 1.0) - (a - 1.0) * c + sa,
        2.0 * ((a - 1.0) - (a + 1.0) * c),
        (a + 1.0) - (a - 1.0) * c - sa,
    )
    .process(x)
}

/// 前後の無音を削る(TTSエンジンは前後に数百msの無音を付けることがあり、間の長さを制御できなくなるため)。
/// 10ms窓のRMSが`threshold_db`を超えた最初/最後の位置から`keep_ms`だけ残す。全体が無音ならそのまま返す。
pub fn trim_silence(x: &[f32], sr: u32, threshold_db: f64, keep_ms: u32) -> Vec<f32> {
    let win = ((sr / 100) as usize).max(1);
    let thr = 10f64.powf(threshold_db / 20.0);
    let loud = |from: usize| -> bool {
        let to = x.len().min(from + win);
        let e: f64 = x[from..to].iter().map(|v| *v as f64 * *v as f64).sum();
        (e / (to - from) as f64).sqrt() > thr
    };
    let mut first: Option<usize> = None;
    let mut i = 0;
    while i < x.len() {
        if loud(i) {
            first = Some(i);
            break;
        }
        i += win;
    }
    let Some(first) = first else {
        return x.to_vec();
    };
    let mut last = first;
    let mut i = (x.len() - 1) / win * win;
    loop {
        if i < first {
            break;
        }
        if loud(i) {
            last = x.len().min(i + win);
            break;
        }
        if i < win {
            break;
        }
        i -= win;
    }
    let keep = (sr as usize * keep_ms as usize) / 1000;
    x[first.saturating_sub(keep)..x.len().min(last + keep)].to_vec()
}

const KNEE: f32 = 0.7;
const CEILING: f32 = 0.9;

fn soft_limit(v: f32) -> f32 {
    let a = v.abs();
    if a <= KNEE {
        return v;
    }
    let over = (((a - KNEE) / (CEILING - KNEE)) as f64).tanh() as f32;
    let y = KNEE + (CEILING - KNEE) * over;
    if v < 0.0 {
        -y
    } else {
        y
    }
}

/// 声の大きさを揃える: RMSを`target_rms`に合わせ、KNEEを超える部分はtanhでなだらかに丸めてCEILING以内に収める
/// (ピーク基準だと短い言葉と長い文で聴こえの大きさがばらつくため)。無音はそのまま。
pub fn normalize_loudness(x: &[f32], target_rms: f32) -> Vec<f32> {
    let e: f64 = x.iter().map(|v| *v as f64 * *v as f64).sum();
    let rms = (e / x.len().max(1) as f64).sqrt() as f32;
    if rms < 1e-6 {
        return x.to_vec();
    }
    let g = target_rms / rms;
    x.iter().map(|v| soft_limit(v * g)).collect()
}

/// 区切りを、それぞれの後ろの間(`gaps_ms`ミリ秒の無音)を挟んで連結する。
pub fn join(parts: &[Vec<f32>], gaps_ms: &[i32], sr: u32) -> Vec<f32> {
    let gap = |i: usize| -> usize { (gaps_ms.get(i).copied().unwrap_or(0) as i64 * sr as i64 / 1000).max(0) as usize };
    let total: usize = parts.iter().enumerate().map(|(i, p)| p.len() + gap(i)).sum();
    let mut out = vec![0f32; total];
    let mut pos = 0;
    for (i, p) in parts.iter().enumerate() {
        out[pos..pos + p.len()].copy_from_slice(p);
        pos += p.len() + gap(i);
    }
    out
}

/// ピークを`peak`に揃える(無音はそのまま)。
pub fn normalize_peak(x: &[f32], peak: f32) -> Vec<f32> {
    let m = x.iter().fold(0f32, |m, v| m.max(v.abs()));
    if m < 1e-6 {
        return x.to_vec();
    }
    let g = peak / m;
    x.iter().map(|v| v * g).collect()
}

/// 先頭/末尾5msのフェード(クリックノイズ防止)。
pub fn fade(x: &mut [f32], sr: u32) {
    let n = ((sr as f64 * 0.005) as usize).min(x.len() / 2);
    let len = x.len();
    for i in 0..n {
        let g = i as f32 / n as f32;
        x[i] *= g;
        x[len - 1 - i] *= g;
    }
}
