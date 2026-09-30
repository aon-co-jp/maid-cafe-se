//! 帯域拡張の動作確認ツール。モデルが無ければ取得し(固定リビジョン)、WAVを処理して統計を表示する。
//! 使い方: `cargo run --release -p maid-cafe-enhance --example enhance_wav -- <モデル置き場> <in.wav> [out.wav] [カットオフHz]`
//! `--hashes <モデル置き場>` で、導入済みモデルのSHA-256を表示する(初回に固定値を決めるための調査用)。

use maid_cafe_core::audio::wav::{parse_wav, wav_bytes};
use maid_cafe_enhance::{detect_cutoff_hz, enhance_speech, installed_model_hashes, resample_to_48k_for_analysis, Lavasr, SPEECH_MIN_DROP_DB};
use std::path::PathBuf;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(|s| s.as_str()) == Some("--hashes") {
        for (n, h) in installed_model_hashes(&PathBuf::from(&args[1])) {
            println!("{n}  {h}");
        }
        return;
    }
    let (models, input) = (PathBuf::from(&args[0]), &args[1]);
    let pcm = parse_wav(&std::fs::read(input).expect("入力を読めません")).expect("16bit PCMのWAVではありません");
    let t = Instant::now();
    let model = Lavasr::load(&models).expect("モデルを読み込めません");
    println!("モデル読み込み: {:.1}秒", t.elapsed().as_secs_f64());
    let x48 = resample_to_48k_for_analysis(&pcm);
    let cutoff = args.get(3).and_then(|s| s.parse::<f32>().ok());
    println!("音楽向けの崖検出(しきい値{SPEECH_MIN_DROP_DB}dB): {:?}", detect_cutoff_hz(&x48, SPEECH_MIN_DROP_DB));
    println!("声向けのロールオフ検出: {:?}", maid_cafe_enhance::detect_speech_rolloff_hz(&x48));
    let t = Instant::now();
    let out = enhance_speech(&model, &pcm, cutoff).expect("帯域拡張に失敗");
    println!("処理: {:.2}秒(音声 {:.2}秒)", t.elapsed().as_secs_f64(), pcm.seconds());
    if let Some(path) = args.get(2) {
        std::fs::write(path, wav_bytes(&out)).expect("書き出せません");
        println!("書き出し: {path}");
    }
}
