//! TTS音声のAI帯域拡張(低域保持・包絡整合型)。
//!
//! 端末/Windowsの音声合成は、高域(特にサ行などの摩擦音が持つ8kHz以上)が欠けた音声を出すことが多い。
//! 学習済みモデル(LavaSR、Apache-2.0、ONNX、[TigreGotico/audiosronnx-lavasr](https://huggingface.co/TigreGotico/audiosronnx-lavasr))で
//! 高域を生成して足す。推論は純Rustの`tract`、周辺のDSP(リサンプル・STFT・メルフィルタ・ISTFT)もRustで実装している。
//! モデルは初回のみダウンロードする(約56MB、リビジョンを固定しSHA-256で検証)。
//!
//! make-disk(`engine/audio_sr.rs`)から移植した設計。「悪化させない」ための実測に基づく3つの決まり:
//! 1. **入力の帯域は一切変えない**(出力 = 入力 + 生成した高域だけ)。モデルの生の出力は低域まで作り直して元信号から離れる。
//! 2. 入力のカットオフ周波数を自動検出し、それより上だけを生成分で埋める。
//! 3. 生成した高域は、入力のスペクトル包絡(カットオフ直下の傾き)を対数線形に外挿した値を**上限**として頭打ちにする
//!    (生の出力は高域が実際より約12dB大きくなる)。
//!
//! **注意**: 効果を測る指標(LSD)はスペクトル包絡の近さで、聴感品質そのものではない。帯域拡張は「復元」ではなく合成。

use maid_cafe_core::audio::resampler::resample_poly;
use maid_cafe_core::audio::wav::Pcm;
use rustfft::{num_complex::Complex, Fft, FftPlanner};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tract_onnx::prelude::*;

/// 元モデルのライセンス表記(配布物のNOTICEに載せる)。
pub const NOTICE: &str = "LavaSR (https://github.com/ysharma3501/LavaSR) — Apache-2.0. ONNX conversion: TigreGotico/audiosronnx (Apache-2.0). \
Trained on the VCTK corpus (CC BY 4.0).";

const HF_REVISION: &str = "b3df8a262cf44e59bf84a40b7084f4479ca566b4";
const HF_BASE: &str = "https://huggingface.co/TigreGotico/audiosronnx-lavasr/resolve";
/// (ファイル名, SHA-256)。固定したリビジョンの内容の検証用。
const FILES: [(&str, &str); 2] = [
    ("backbone.onnx", "959f7879f58a80ce8e76156e3ee4dd3d56b6a5d62713272806f2c2f2860f56c0"),
    ("spec_head.onnx", "abd7961809fde26b38f3e7499d1d1240a222deee64cf472b9f6d9827d27a4e8a"),
];

/// 出力サンプルレート。
pub const OUT_SR: usize = 48_000;
const MODEL_SR: usize = 16_000;
const ENH_SR: usize = 44_100;
const ENH_NFFT: usize = 2048;
const ENH_HOP: usize = 512;
const ENH_MELS: usize = 80;
/// モデルに一度に渡すフレーム数(固定形状で最適化するため)と、前後の文脈フレーム数。
const CHUNK_FRAMES: usize = 256;
const CONTEXT_FRAMES: usize = 16;
/// 長い音声を処理する区間の長さ(秒)と前後の余白(秒)。メモリを抑えるための分割。
const SEGMENT_SECS: usize = 30;
const MARGIN_SECS: usize = 1;

// ─────────────────────────── DSP ───────────────────────────

fn gcd(a: usize, b: usize) -> usize {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

fn resample_rate(x: &[f32], from: usize, to: usize) -> Vec<f32> {
    let g = gcd(from, to);
    resample_poly(x, to / g, from / g)
}

fn hann_periodic(n: usize) -> Vec<f32> {
    (0..n).map(|k| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * k as f32 / n as f32).cos()).collect()
}

/// scipy.signal.stft/istft互換(hann、boundary=zeros、padded、spectrumスケーリング)。
struct Stft {
    n_fft: usize,
    hop: usize,
    win: Vec<f32>,
    win_sum: f32,
    fwd: Arc<dyn Fft<f32>>,
    inv: Arc<dyn Fft<f32>>,
}

impl Stft {
    fn new(n_fft: usize, hop: usize) -> Self {
        let mut planner = FftPlanner::<f32>::new();
        let win = hann_periodic(n_fft);
        let win_sum = win.iter().sum();
        Stft { n_fft, hop, win, win_sum, fwd: planner.plan_fft_forward(n_fft), inv: planner.plan_fft_inverse(n_fft) }
    }

    fn bins(&self) -> usize {
        self.n_fft / 2 + 1
    }

    /// `(フレーム数, フレームごとのbins複素数の平坦配列)`
    fn forward(&self, x: &[f32]) -> (usize, Vec<Complex<f32>>) {
        let pad = self.n_fft / 2;
        let mut p = vec![0f32; pad];
        p.extend_from_slice(x);
        p.extend(std::iter::repeat(0.0).take(pad));
        let extra = (self.hop - (p.len().saturating_sub(self.n_fft)) % self.hop) % self.hop;
        p.extend(std::iter::repeat(0.0).take(extra));
        let frames = (p.len() - self.n_fft) / self.hop + 1;
        let bins = self.bins();
        let mut out = vec![Complex::new(0.0, 0.0); frames * bins];
        let mut buf = vec![Complex::new(0.0, 0.0); self.n_fft];
        for f in 0..frames {
            for (i, b) in buf.iter_mut().enumerate() {
                *b = Complex::new(p[f * self.hop + i] * self.win[i], 0.0);
            }
            self.fwd.process(&mut buf);
            for k in 0..bins {
                out[f * bins + k] = buf[k] / self.win_sum;
            }
        }
        (frames, out)
    }

    fn inverse(&self, frames: usize, spec: &[Complex<f32>], target_len: usize) -> Vec<f32> {
        let bins = self.bins();
        let total = self.n_fft + (frames - 1) * self.hop;
        let mut x = vec![0f32; total];
        let mut norm = vec![0f32; total];
        let mut buf = vec![Complex::new(0.0, 0.0); self.n_fft];
        let scale = self.win_sum / self.n_fft as f32;
        for f in 0..frames {
            for k in 0..bins {
                buf[k] = spec[f * bins + k];
            }
            for k in 1..self.n_fft / 2 {
                buf[self.n_fft - k] = spec[f * bins + k].conj();
            }
            buf[0].im = 0.0;
            buf[self.n_fft / 2].im = 0.0;
            self.inv.process(&mut buf);
            for i in 0..self.n_fft {
                x[f * self.hop + i] += buf[i].re * scale * self.win[i];
                norm[f * self.hop + i] += self.win[i] * self.win[i];
            }
        }
        let pad = self.n_fft / 2;
        let mut out: Vec<f32> =
            (pad..total.saturating_sub(pad)).map(|i| if norm[i] > 1e-10 { x[i] / norm[i] } else { x[i] }).collect();
        out.resize(target_len, 0.0);
        out
    }
}

fn hz_to_mel(f: f64) -> f64 {
    let (f_sp, min_log_hz) = (200.0 / 3.0, 1000.0);
    let min_log_mel = min_log_hz / f_sp;
    let logstep = (6.4f64).ln() / 27.0;
    if f < min_log_hz {
        f / f_sp
    } else {
        min_log_mel + (f / min_log_hz).ln() / logstep
    }
}

fn mel_to_hz(m: f64) -> f64 {
    let (f_sp, min_log_hz) = (200.0 / 3.0, 1000.0);
    let min_log_mel = min_log_hz / f_sp;
    let logstep = (6.4f64).ln() / 27.0;
    if m < min_log_mel {
        m * f_sp
    } else {
        min_log_hz * (logstep * (m - min_log_mel)).exp()
    }
}

/// メルフィルタバンク(`[mel][bin]`)。LavaSRの学習時の前処理と同じ定義(fmin=0、fmax=8000、slaney風の面積正規化)。
fn mel_filterbank(sr: usize, n_fft: usize, n_mels: usize, fmin: f64, fmax: f64) -> Vec<Vec<f32>> {
    let bins = n_fft / 2 + 1;
    let (m0, m1) = (hz_to_mel(fmin), hz_to_mel(fmax));
    let edges: Vec<f64> = (0..n_mels + 2).map(|i| mel_to_hz(m0 + (m1 - m0) * i as f64 / (n_mels + 1) as f64)).collect();
    let mut fb = vec![vec![0f32; bins]; n_mels];
    for m in 0..n_mels {
        let (l, c, r) = (edges[m], edges[m + 1], edges[m + 2]);
        if c <= l || r <= c {
            continue;
        }
        for (k, v) in fb[m].iter_mut().enumerate() {
            let f = k as f64 * (sr as f64 / 2.0) / (bins - 1) as f64;
            let up = (f - l) / (c - l);
            let down = (r - f) / (r - c);
            *v = (up.min(down).max(0.0) * (2.0 / (r - l).max(1e-8))) as f32;
        }
    }
    fb
}

// ─────────────────────────── モデル取得・推論 ───────────────────────────

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// モデル(ONNX 2ファイル、約56MB)が`base_dir`配下に無ければダウンロードして、そのフォルダを返す。導入済みならスキップ。
/// ダウンロードしたファイルは、固定したSHA-256と照合する(不一致なら保存しない)。
pub fn ensure_models(base_dir: &Path) -> Result<PathBuf, String> {
    let dir = model_subdir(base_dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("モデルフォルダを作成できません: {e}"))?;
    for (name, sha) in FILES {
        let path = dir.join(name);
        if path.is_file() {
            continue;
        }
        let url = format!("{HF_BASE}/{HF_REVISION}/{name}");
        let mut bytes = Vec::new();
        let resp = ureq::get(&url).call().map_err(|e| format!("音声モデルのダウンロードに失敗しました({url}): {e}"))?;
        std::io::copy(&mut resp.into_reader(), &mut bytes).map_err(|e| format!("ダウンロードの読み取りに失敗しました: {e}"))?;
        let got = sha256_hex(&bytes);
        if !sha.is_empty() && got != sha {
            return Err(format!("{name}のSHA-256が一致しません(期待 {sha}、実際 {got})。保存しません"));
        }
        let tmp = dir.join(format!("{name}.part"));
        std::fs::write(&tmp, &bytes).and_then(|_| std::fs::rename(&tmp, &path)).map_err(|e| format!("モデルの保存に失敗しました: {e}"))?;
    }
    Ok(dir)
}

fn model_subdir(base_dir: &Path) -> PathBuf {
    base_dir.join(format!("lavasr-{}", &HF_REVISION[..8]))
}

/// `base_dir`に、必要なONNXが2つとも揃っているか。
pub fn models_present(base_dir: &Path) -> bool {
    FILES.iter().all(|(name, _)| model_subdir(base_dir).join(name).is_file())
}

/// 導入済みのモデルのSHA-256(初回にFILESへ固定値を入れるための調査用)。
pub fn installed_model_hashes(base_dir: &Path) -> Vec<(String, String)> {
    let dir = model_subdir(base_dir);
    FILES
        .iter()
        .filter_map(|(name, _)| std::fs::read(dir.join(name)).ok().map(|b| (name.to_string(), sha256_hex(&b))))
        .collect()
}

type Plan = TypedRunnableModel<TypedModel>;

/// 読み込み済みのモデル(初回の最適化に少し時間がかかるので、使い回す)。
pub struct Lavasr {
    backbone: Plan,
    head: Plan,
    mel_fb: Vec<Vec<f32>>,
    stft: Stft,
}

impl Lavasr {
    /// まず`bundled_dirs`(アプリに同梱したモデル置き場)を探し、無ければ`cache_dir`(無ければ取得して保存)から読み込む。
    /// 同梱モデルは、インストーラーのビルド時に固定リビジョンから取得してSHA-256を検証したもの。
    pub fn load_bundled_or_download(bundled_dirs: &[PathBuf], cache_dir: &Path) -> Result<Self, String> {
        for d in bundled_dirs {
            if models_present(d) {
                return Self::load_from(&model_subdir(d));
            }
        }
        Self::load(cache_dir)
    }

    /// `base_dir`にモデルが無ければダウンロードして読み込む。
    pub fn load(base_dir: &Path) -> Result<Self, String> {
        let dir = ensure_models(base_dir)?;
        Self::load_from(&dir)
    }

    /// モデルのONNXが入ったフォルダ(`lavasr-<リビジョン>`)から読み込む。
    pub fn load_from(dir: &Path) -> Result<Self, String> {
        let build = |name: &str, shape: [usize; 3]| -> Result<Plan, String> {
            tract_onnx::onnx()
                .model_for_path(dir.join(name))
                .and_then(|m| m.with_input_fact(0, f32::fact(shape).into()))
                .and_then(|m| m.into_optimized())
                .and_then(|m| m.into_runnable())
                .map_err(|e| format!("{name}を読み込めません: {e}"))
        };
        let t = CHUNK_FRAMES + 2 * CONTEXT_FRAMES;
        Ok(Lavasr {
            backbone: build("backbone.onnx", [1, ENH_MELS, t])?,
            head: build("spec_head.onnx", [1, t, 512])?,
            mel_fb: mel_filterbank(ENH_SR, ENH_NFFT, ENH_MELS, 0.0, 8000.0),
            stft: Stft::new(ENH_NFFT, ENH_HOP),
        })
    }

    /// 44.1kHzのモノラル信号を、モデルで高域まで再合成した信号(同じ長さ)にする。
    fn enhance(&self, wave: &[f32]) -> Result<Vec<f32>, String> {
        let (frames, spec) = self.stft.forward(wave);
        let bins = self.stft.bins();
        // メル(対数)を [mel][frame] で作る
        let mut mel = vec![0f32; ENH_MELS * frames];
        for f in 0..frames {
            for m in 0..ENH_MELS {
                let mut acc = 0f32;
                for (k, w) in self.mel_fb[m].iter().enumerate() {
                    if *w != 0.0 {
                        acc += w * spec[f * bins + k].norm();
                    }
                }
                mel[m * frames + f] = acc.max(1e-5).ln();
            }
        }
        let t = CHUNK_FRAMES + 2 * CONTEXT_FRAMES;
        let mut out_spec = vec![Complex::new(0.0, 0.0); frames * bins];
        let mut start = 0;
        while start < frames {
            let core = CHUNK_FRAMES.min(frames - start);
            // 文脈フレームを含む窓 [start-CTX, start-CTX+t) を、範囲外は端のフレームで埋めて作る。
            let mut input = vec![0f32; ENH_MELS * t];
            for m in 0..ENH_MELS {
                for j in 0..t {
                    let src = (start as isize - CONTEXT_FRAMES as isize + j as isize).clamp(0, frames as isize - 1) as usize;
                    input[m * t + j] = mel[m * frames + src];
                }
            }
            let x = tract_ndarray::Array3::from_shape_vec((1, ENH_MELS, t), input).map_err(|e| e.to_string())?.into_tensor();
            let hidden = self.backbone.run(tvec!(x.into())).map_err(|e| format!("backbone推論に失敗しました: {e}"))?;
            let h = hidden[0].clone().into_tensor();
            let heads = self.head.run(tvec!(h.into())).map_err(|e| format!("spec_head推論に失敗しました: {e}"))?;
            let re = heads[0].to_array_view::<f32>().map_err(|e| e.to_string())?; // [1][bins][t]
            let im = heads[1].to_array_view::<f32>().map_err(|e| e.to_string())?;
            for j in 0..core {
                for k in 0..bins {
                    out_spec[(start + j) * bins + k] = Complex::new(re[[0, k, CONTEXT_FRAMES + j]], im[[0, k, CONTEXT_FRAMES + j]]);
                }
            }
            start += core;
        }
        Ok(self.stft.inverse(frames, &out_spec, wave.len()))
    }

    /// 48kHzのモノラル信号`x48`から、モデルが生成した48kHz信号を返す(長さは`x48`と同じ)。
    fn generate(&self, x48: &[f32]) -> Result<Vec<f32>, String> {
        let wave16 = resample_rate(x48, OUT_SR, MODEL_SR);
        let enh_in = resample_rate(&wave16, MODEL_SR, ENH_SR);
        let enhanced = self.enhance(&enh_in)?;
        let mut o48 = resample_rate(&enhanced, ENH_SR, OUT_SR);
        o48.resize(x48.len(), 0.0);
        Ok(o48)
    }
}

// ─────────────────────────── 包絡整合スプライス ───────────────────────────

/// 入力信号`y`(48kHz)のスペクトル包絡(カットオフ直下の傾き)を外挿して、生成信号`generated`の高域(`fc`以上)の
/// 各フレーム・各binの振幅を頭打ちにし、その高域だけの信号を返す。
pub fn limit_generated_hf(y: &[f32], generated: &[f32], fc: f32) -> Vec<f32> {
    let stft = Stft::new(2048, 512);
    let bins = stft.bins();
    let (frames, sy) = stft.forward(y);
    let (_, sg) = stft.forward(generated);
    let freq = |k: usize| k as f32 * (OUT_SR as f32 / 2.0) / (bins - 1) as f32;
    let (lo_a, lo_b) = (fc * 0.6, fc * 0.95);
    let fit_bins: Vec<usize> = (0..bins).filter(|&k| freq(k) >= lo_a && freq(k) < lo_b).collect();
    let mut out = vec![Complex::new(0.0, 0.0); frames * bins];
    for f in 0..frames {
        // log10パワーを周波数に対して最小二乗で直線当てはめ
        let (mut sx, mut sy_, mut sxx, mut sxy) = (0f64, 0f64, 0f64, 0f64);
        for &k in &fit_bins {
            let x = freq(k) as f64;
            let p = ((sy[f * bins + k].norm_sqr() + 1e-12) as f64).log10();
            sx += x;
            sy_ += p;
            sxx += x * x;
            sxy += x * p;
        }
        let n = fit_bins.len() as f64;
        let denom = n * sxx - sx * sx;
        let (slope, icpt) = if denom.abs() < 1e-9 {
            (0.0, sy_ / n.max(1.0))
        } else {
            ((n * sxy - sx * sy_) / denom, (sy_ - (n * sxy - sx * sy_) / denom * sx) / n)
        };
        let slope = slope.min(-1e-5); // 上向きには外挿しない
        for k in 0..bins {
            let fk = freq(k);
            if fk < fc {
                continue;
            }
            let target = icpt + slope * fk as f64;
            let g = sg[f * bins + k];
            let gp = ((g.norm_sqr() + 1e-12) as f64).log10();
            let gain = 10f64.powf((target - gp) / 2.0).min(1.0) as f32;
            out[f * bins + k] = g * gain;
        }
    }
    stft.inverse(frames, &out, y.len())
}

/// 入力(48kHz)の帯域のカットオフ周波数(Hz)を推定する。
///
/// 平均パワースペクトル(dB)の中で、低域側800Hzの平均と、600Hz先の高域側800Hzの平均との差が最大になる位置(=急峻な崖)を探す。
/// 崖が`min_drop_db`未満(=帯域が欠けていない、自然な緩やかな減衰)、または約19.5kHz以上なら`None`を返す。
/// 音楽なら25dB、TTSの声(緩やかに減衰する)なら12dB程度が目安。
pub fn detect_cutoff_hz(x48: &[f32], min_drop_db: f64) -> Option<f32> {
    let n_fft = 4096;
    let stft = Stft::new(n_fft, 2048);
    let take = x48.len().min(OUT_SR * 120);
    let (frames, spec) = stft.forward(&x48[..take]);
    let bins = stft.bins();
    let mut power = vec![0f64; bins];
    for f in 0..frames {
        for k in 0..bins {
            power[k] += spec[f * bins + k].norm_sqr() as f64;
        }
    }
    let db: Vec<f64> = power.iter().map(|p| 10.0 * (p / frames as f64 + 1e-20).log10()).collect();
    let mut prefix = vec![0f64; bins + 1];
    for k in 0..bins {
        prefix[k + 1] = prefix[k] + db[k];
    }
    let bin_hz = OUT_SR as f64 / 2.0 / (bins - 1) as f64;
    let (win, gap) = ((800.0 / bin_hz) as usize, (600.0 / bin_hz) as usize);
    let mean = |a: usize, b: usize| (prefix[b] - prefix[a]) / (b - a) as f64;
    let (k_lo, k_hi) = ((3_000.0 / bin_hz) as usize + win, (19_500.0 / bin_hz) as usize);
    let (mut best_k, mut best_drop) = (0usize, f64::MIN);
    for k in k_lo..=k_hi.min(bins - 1 - gap - win) {
        let drop = mean(k - win, k) - mean(k + gap, k + gap + win);
        if drop > best_drop {
            (best_k, best_drop) = (k, drop);
        }
    }
    (best_drop >= min_drop_db).then(|| ((best_k + gap / 2) as f64 * bin_hz) as f32)
}

/// 1チャンネル(48kHz)を処理して、入力+頭打ちした生成高域の信号を返す。
/// `cutoff_hz`が`None`なら`min_drop_db`で自動検出し、帯域が欠けていなければ入力をそのまま返す。
pub fn extend_channel(model: &Lavasr, x48: &[f32], cutoff_hz: Option<f32>, min_drop_db: f64) -> Result<Vec<f32>, String> {
    let Some(fc) = cutoff_hz.or_else(|| detect_cutoff_hz(x48, min_drop_db)) else {
        return Ok(x48.to_vec());
    };
    let (seg, margin) = (SEGMENT_SECS * OUT_SR, MARGIN_SECS * OUT_SR);
    let mut hf = Vec::with_capacity(x48.len());
    let mut start = 0;
    while start < x48.len() {
        let end = (start + seg).min(x48.len());
        let (a, b) = (start.saturating_sub(margin), (end + margin).min(x48.len()));
        let piece = &x48[a..b];
        let generated = model.generate(piece)?;
        let limited = limit_generated_hf(piece, &generated, fc);
        hf.extend_from_slice(&limited[start - a..start - a + (end - start)]);
        start = end;
    }
    Ok(x48.iter().zip(&hf).map(|(y, h)| y + h).collect())
}

/// TTSの声の高域の減衰(ロールオフ)の位置から、帯域拡張を始めるカットオフ(Hz)を決める。
///
/// 音楽向けの[`detect_cutoff_hz`]は「急峻な崖」を探すが、TTSの声は8kHzあたりから**なだらかに**減衰するだけで崖にならない
/// (実測: Windowsの音声Harukaは、3〜6kHzが約-13dBのとき、7〜8kHzで-18dB、8〜9kHzで-26dB、9〜10kHzで-36dB)。
/// そこで、声の主要帯域(3〜6kHz)の平均レベルより`ROLLOFF_DB`下がった最初の周波数を見つけ、
/// その少し手前(`ROLLOFF_MARGIN_HZ`)をカットオフにする(手前の帯域の傾きから高域を外挿するため)。
/// 見つからない(=高域まで十分ある)、または拡張する価値のある範囲(5〜16kHz)を外れたら`None`。
pub fn detect_speech_rolloff_hz(x48: &[f32]) -> Option<f32> {
    let n_fft = 4096;
    let stft = Stft::new(n_fft, 2048);
    let take = x48.len().min(OUT_SR * 120);
    let (frames, spec) = stft.forward(&x48[..take]);
    let bins = stft.bins();
    let mut power = vec![0f64; bins];
    let mut used = 0usize;
    for f in 0..frames {
        let e: f64 = (0..bins).map(|k| spec[f * bins + k].norm_sqr() as f64).sum();
        if e > 1e-9 {
            // 無音に近いフレームは平均に入れない
            for k in 0..bins {
                power[k] += spec[f * bins + k].norm_sqr() as f64;
            }
            used += 1;
        }
    }
    if used == 0 {
        return None;
    }
    let bin_hz = OUT_SR as f64 / 2.0 / (bins - 1) as f64;
    let db: Vec<f64> = power.iter().map(|p| 10.0 * (p / used as f64 + 1e-20).log10()).collect();
    let band = |lo: f64, hi: f64| -> f64 {
        let (a, b) = ((lo / bin_hz) as usize, (hi / bin_hz) as usize);
        db[a..b].iter().sum::<f64>() / (b - a) as f64
    };
    let reference = band(3_000.0, 6_000.0);
    let half = (300.0 / bin_hz) as usize;
    let smooth = |k: usize| -> f64 {
        let (a, b) = (k.saturating_sub(half), (k + half).min(bins - 1));
        db[a..=b].iter().sum::<f64>() / (b - a + 1) as f64
    };
    let start = (6_000.0 / bin_hz) as usize;
    let end = (19_500.0 / bin_hz) as usize;
    let crossing = (start..end.min(bins)).find(|&k| smooth(k) < reference - ROLLOFF_DB)?;
    let fc = crossing as f64 * bin_hz - ROLLOFF_MARGIN_HZ;
    (5_000.0..=16_000.0).contains(&fc).then_some(fc as f32)
}

/// 声の主要帯域よりこれだけ(dB)下がった所を「高域が欠けている」とみなす。
const ROLLOFF_DB: f64 = 18.0;
/// 拡張の開始点は、下がり始めの少し手前に置く。
const ROLLOFF_MARGIN_HZ: f64 = 1_500.0;

/// 解析用に48kHzへ上げた信号を返す(カットオフ検出の確認用)。
pub fn resample_to_48k_for_analysis(pcm: &Pcm) -> Vec<f32> {
    resample_rate(&pcm.samples, pcm.sample_rate as usize, OUT_SR)
}

/// TTSの声向けの既定: カットオフ検出のしきい値(dB)。
pub const SPEECH_MIN_DROP_DB: f64 = 12.0;

/// TTS音声(任意のサンプルレート)を、48kHzへ上げて高域を足した音声にする。帯域が欠けていない音声は、
/// 48kHzへ上げるだけで高域は足さない。
pub fn enhance_speech(model: &Lavasr, pcm: &Pcm, cutoff_hz: Option<f32>) -> Result<Pcm, String> {
    let x48 = resample_rate(&pcm.samples, pcm.sample_rate as usize, OUT_SR);
    // 指定が無ければ、声のなだらかな減衰から決める。見つからなければ(高域が十分ある声)、48kHzへ上げるだけ。
    let cutoff = cutoff_hz.or_else(|| detect_speech_rolloff_hz(&x48));
    let y = match cutoff {
        Some(fc) => extend_channel(model, &x48, Some(fc), SPEECH_MIN_DROP_DB)?,
        None => x48,
    };
    Ok(Pcm::new(y, OUT_SR as u32))
}

#[cfg(test)]
mod tests;
