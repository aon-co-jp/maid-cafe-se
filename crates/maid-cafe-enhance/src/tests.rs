//! 帯域拡張のユニットテスト(モデル不要)。

use super::*;
use std::f64::consts::PI;

/// 疑似ランダム(再現可能): 簡単な線形合同法で[-1,1)。
fn noise(n: usize, seed: u64) -> Vec<f32> {
    let mut s = seed;
    (0..n)
        .map(|_| {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((s >> 33) as f64 / (1u64 << 31) as f64 * 2.0 - 1.0) as f32
        })
        .collect()
}

/// 高域ほど弱いスペクトル(声のような傾き)の48kHzノイズ。`lowpass_hz`より上は(ほぼ)無音。
fn tilted_noise(seconds: f64, lowpass_hz: Option<f64>) -> Vec<f32> {
    let n = (OUT_SR as f64 * seconds) as usize;
    let mut x = noise(n, 1);
    // 1次ローパスの3段で、なだらかな減衰(声の傾きに近い)を作る
    for _ in 0..3 {
        let a = 0.55f32;
        let mut y = 0.0;
        for v in x.iter_mut() {
            y = a * y + (1.0 - a) * *v;
            *v = y;
        }
    }
    if let Some(fc) = lowpass_hz {
        // 急峻に切る(48kHz→2*fcまで下げて戻す=Kaiser窓のアンチエイリアス)
        let mid = (fc * 2.0) as usize;
        x = resample_rate(&resample_rate(&x, OUT_SR, mid), mid, OUT_SR);
    }
    x
}

/// 帯域`[lo, hi]`Hzの平均パワー(dB)。
fn band_level(x: &[f32], lo: f64, hi: f64) -> f64 {
    let stft = Stft::new(2048, 512);
    let (frames, spec) = stft.forward(x);
    let bins = stft.bins();
    let bin_hz = OUT_SR as f64 / 2.0 / (bins - 1) as f64;
    let (k0, k1) = ((lo / bin_hz) as usize, (hi / bin_hz) as usize);
    let mut acc = 0.0;
    for f in 0..frames {
        for k in k0..k1 {
            acc += spec[f * bins + k].norm_sqr() as f64;
        }
    }
    10.0 * (acc / (frames * (k1 - k0)) as f64 + 1e-20).log10()
}

#[test]
fn sha256_known_vector() {
    assert_eq!("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad", sha256_hex(b"abc"));
}

#[test]
fn stft_round_trip_reconstructs_the_signal() {
    let stft = Stft::new(ENH_NFFT, ENH_HOP);
    let x: Vec<f32> = (0..20_000).map(|i| ((2.0 * PI * 440.0 * i as f64 / 44_100.0).sin() * 0.5) as f32).collect();
    let (frames, spec) = stft.forward(&x);
    let y = stft.inverse(frames, &spec, x.len());
    assert_eq!(x.len(), y.len());
    let max = x.iter().zip(&y).map(|(a, b)| (a - b).abs()).fold(0f32, f32::max);
    assert!(max < 1e-3, "再構成誤差 {max}");
}

#[test]
fn mel_filterbank_has_the_expected_shape() {
    let fb = mel_filterbank(ENH_SR, ENH_NFFT, ENH_MELS, 0.0, 8000.0);
    assert_eq!(ENH_MELS, fb.len());
    assert!(fb.iter().all(|row| row.len() == ENH_NFFT / 2 + 1 && row.iter().all(|v| *v >= 0.0)));
    // 8kHzより上のビンには重みが無い(モデルの入力は8kHzまで)
    let bin_8k = (8000.0 / (ENH_SR as f64 / 2.0) * (ENH_NFFT / 2) as f64) as usize + 2;
    assert!(fb.iter().all(|row| row[bin_8k..].iter().all(|v| *v == 0.0)));
    assert!(fb.iter().all(|row| row.iter().any(|v| *v > 0.0)));
}

#[test]
fn speech_rolloff_is_found_for_a_band_limited_voice_and_not_for_full_band() {
    let limited = tilted_noise(3.0, Some(8000.0));
    let fc = detect_speech_rolloff_hz(&limited).expect("8kHzで帯域が切れた声からロールオフを見つけられる");
    assert!((5000.0..=8000.0).contains(&fc), "fc={fc}");
    // 全帯域に十分な(白色に近い)音声には拡張の必要が無い
    assert_eq!(None, detect_speech_rolloff_hz(&noise(OUT_SR * 3, 7)));
    assert_eq!(None, detect_speech_rolloff_hz(&vec![0f32; OUT_SR])); // 無音
}

#[test]
fn generated_highs_are_capped_by_the_extrapolated_envelope() {
    let fc = 8000.0f32;
    let y = tilted_noise(2.0, Some(fc as f64 + 500.0));
    // 生成信号としてわざと非常に大きな白色ノイズを与える(生の出力が実際より大きい状況)
    let generated: Vec<f32> = noise(y.len(), 3).iter().map(|v| v * 5.0).collect();
    let hf = limit_generated_hf(&y, &generated, fc);
    assert_eq!(y.len(), hf.len());
    // 足した高域(12〜16kHz)は、入力の直下(5〜7kHz)のレベルより弱く、生成信号そのもの(白色5倍)よりずっと弱い
    let (l_hf, l_below, l_gen) =
        (band_level(&hf, 12_000.0, 16_000.0), band_level(&y, 5_000.0, 7_000.0), band_level(&generated, 12_000.0, 16_000.0));
    assert!(l_hf < l_below, "足した高域 {l_hf}dB が入力直下 {l_below}dB より強い");
    assert!(l_hf < l_gen - 20.0, "頭打ちが効いていない: {l_hf}dB vs 生成 {l_gen}dB");
    // カットオフより下には何も足さない
    assert!(band_level(&hf, 500.0, 6_000.0) < l_below - 60.0, "低域に足している");
}

#[test]
fn models_present_and_ensure_skip_the_download_when_files_exist() {
    let base = std::env::temp_dir().join(format!("mcs-enh-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    assert!(!models_present(&base));
    let dir = model_subdir(&base);
    std::fs::create_dir_all(&dir).unwrap();
    for (name, _) in FILES {
        std::fs::write(dir.join(name), b"dummy").unwrap();
    }
    assert!(models_present(&base));
    // 既にあるので、ネットワークに出ずにそのフォルダを返す
    assert_eq!(dir, ensure_models(&base).unwrap());
    std::fs::remove_dir_all(&base).unwrap();
}

#[test]
fn pinned_hashes_are_well_formed() {
    for (name, sha) in FILES {
        assert_eq!(64, sha.len(), "{name}: SHA-256が固定されていない");
        assert!(sha.chars().all(|c| c.is_ascii_hexdigit()));
    }
    assert_eq!(40, HF_REVISION.len());
}
