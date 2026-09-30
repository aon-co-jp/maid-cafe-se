//! 発火した[`Occurrence`]を鳴らす。
//!
//! 読み上げは、Windowsの音声(SAPI)で区切り(セリフ)ごとにWAVを作り、`open-runo-voice`で声を加工し(音程と声の太さを独立に制御、
//! ハモり、無音トリム、音量統一)、区切りごとの「間」を挟んで連結して、1本にして鳴らす(Kotlin/Android版と同じ処理)。
//! 音は最大30秒ループ。音と読み上げが同時のときは、音を読み上げの下に混ぜた1本にする
//! (Windowsの`PlaySound`は同時に1つしか鳴らせないため)。読み上げに失敗したら、無音で終わらないようチャイムに切り替える。

use crate::sapi::{self, SapiVoice};
use crate::store::Store;
use maid_cafe_core::audio::{dsp, parse_wav, render, resampler::resample_poly, wav_bytes, Pcm};
use maid_cafe_core::{Occurrence, Segment, VoiceStyle};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// 音のループの最大の長さ。
pub const SOUND_MAX: Duration = Duration::from_secs(30);

const CHIME: &[u8] = include_bytes!("../assets/sounds/chime.wav");
const ALARM: &[u8] = include_bytes!("../assets/sounds/alarm.wav");
const MELODY: &[u8] = include_bytes!("../assets/sounds/melody.wav");

/// 同梱の音(`chime`/`alarm`/`melody`)。
pub fn sound_pcm(id: &str) -> Option<Pcm> {
    let bytes = match id {
        "chime" => CHIME,
        "alarm" => ALARM,
        "melody" => MELODY,
        _ => return None,
    };
    parse_wav(bytes)
}

fn gcd(a: usize, b: usize) -> usize {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

/// `x`のサンプルレートを`from`→`to`に変える。
pub fn resample(x: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to {
        return x.to_vec();
    }
    let g = gcd(from as usize, to as usize);
    resample_poly(x, to as usize / g, from as usize / g)
}

/// 音(`sound`)をループさせ、読み上げ(`speech`)の下に混ぜた1本にする。長さは、読み上げと`sound_max`の長いほう。
/// 混ぜる音量は控えめ(読み上げが聞き取れるように)。ピークが1に近ければ縮める。
pub fn mix_sound_under_speech(speech: &Pcm, sound: &Pcm, sound_max: Duration) -> Pcm {
    let sr = speech.sample_rate;
    let bed = resample(&sound.samples, sound.sample_rate, sr);
    let total = speech.samples.len().max((sound_max.as_secs_f64() * sr as f64) as usize);
    let mut out = vec![0f32; total];
    if !bed.is_empty() {
        for (i, o) in out.iter_mut().enumerate() {
            *o = bed[i % bed.len()] * 0.35;
        }
    }
    for (i, v) in speech.samples.iter().enumerate() {
        out[i] += v;
    }
    let peak = out.iter().fold(0f32, |m, v| m.max(v.abs()));
    if peak > 0.98 {
        let g = 0.95 / peak;
        out.iter_mut().for_each(|v| *v *= g);
    }
    Pcm::new(out, sr)
}

/// 複数の読み上げ(同時刻の発火が複数あるとき)を、`gap_ms`の間を挟んで連結する。
pub fn concat_speeches(parts: &[Pcm], gap_ms: i32) -> Option<Pcm> {
    let sr = parts.first()?.sample_rate;
    let resampled: Vec<Vec<f32>> = parts.iter().map(|p| resample(&p.samples, p.sample_rate, sr)).collect();
    let gaps: Vec<i32> = (0..resampled.len()).map(|i| if i + 1 < resampled.len() { gap_ms } else { 0 }).collect();
    Some(Pcm::new(dsp::join(&resampled, &gaps, sr), sr))
}

#[cfg(windows)]
mod winaudio {
    use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_LOOP, SND_MEMORY, SND_NODEFAULT, SND_SYNC};

    /// WAV(メモリ上)を最後まで鳴らす。他スレッドから[`stop_all`]されたら途中で返る。
    pub fn play_sync(wav: &[u8]) -> bool {
        // SAFETY: `wav`は呼び出しの間、有効なバッファ。SND_MEMORYでは、ポインタをWAVイメージとして読む。
        unsafe { PlaySoundW(wav.as_ptr() as *const u16, std::ptr::null_mut(), SND_MEMORY | SND_SYNC | SND_NODEFAULT) != 0 }
    }

    /// WAVをループ再生し始める(すぐ返る)。`wav`は、[`stop_all`]するまで生きていなければならない。
    pub fn play_loop(wav: &[u8]) -> bool {
        // SAFETY: 呼び出し側が、停止までバッファを保持する(`Player::play`参照)。
        unsafe { PlaySoundW(wav.as_ptr() as *const u16, std::ptr::null_mut(), SND_MEMORY | SND_ASYNC | SND_LOOP | SND_NODEFAULT) != 0 }
    }

    pub fn stop_all() {
        // SAFETY: NULLを渡すと、再生中の音を止める。
        unsafe {
            PlaySoundW(std::ptr::null(), std::ptr::null_mut(), 0);
        }
    }
}

#[cfg(not(windows))]
mod winaudio {
    pub fn play_sync(_wav: &[u8]) -> bool {
        false
    }
    pub fn play_loop(_wav: &[u8]) -> bool {
        false
    }
    pub fn stop_all() {}
}

pub struct Player {
    store: Arc<Store>,
    stop: AtomicBool,
    playing: Mutex<Option<String>>,
    /// 同時に鳴らすのは1つだけ(次の発火は、前の再生が終わるまで待つ)
    run_lock: Mutex<()>,
}

impl Player {
    pub fn new(store: Arc<Store>) -> Self {
        Player { store, stop: AtomicBool::new(false), playing: Mutex::new(None), run_lock: Mutex::new(()) }
    }

    /// 再生中の内容(なければ`None`)。
    pub fn playing(&self) -> Option<String> {
        self.playing.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// 再生中の音を止める。
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
        winaudio::stop_all();
    }

    /// 読み上げの音声を作る(SAPIで区切りごとに合成→声を加工→間を挟んで連結)。失敗したら`Err`(原因つき)。
    pub fn render(&self, o: &Occurrence, voice_name: Option<&str>) -> Result<(Pcm, SapiVoice), String> {
        let segs: Vec<Segment> = if o.segments.is_empty() { vec![Segment::plain(o.speech.as_deref().unwrap_or(""))] } else { o.segments.clone() };
        let base_rate = if o.voice == VoiceStyle::DeepMale { 0.92 } else { 1.0 };
        let items: Vec<(i32, String)> = segs.iter().map(|s| (sapi::rate_step(base_rate * s.rate as f64), s.text.clone())).collect();
        let synthesized = sapi::synthesize(&items, voice_name)?;
        let mut parts: Vec<Vec<f32>> = Vec::new();
        let mut sr = 0;
        for (k, wav) in synthesized.wavs.iter().enumerate() {
            let pcm = parse_wav(wav).ok_or("合成した音声(WAV)を読めません")?;
            sr = pcm.sample_rate;
            parts.push(render(&pcm, o.voice, synthesized.voice.gender, o.harmony, segs[k].pitch).samples);
        }
        let gaps: Vec<i32> = segs.iter().map(|s| s.gap_after_ms).collect();
        let joined = Pcm::new(dsp::join(&parts, &gaps, sr), sr);
        self.store.log(&format!(
            "speech {} segments voice='{}' {:.2}s style={:?} harmony={}",
            segs.len(),
            synthesized.voice.name,
            joined.seconds(),
            o.voice,
            o.harmony
        ));
        Ok((joined, synthesized.voice))
    }

    /// 鳴らし終えるまでブロックする(呼び出し側のスレッドで実行)。`voice_name`は声質ごとの、声の元のWindows音声の指定。
    pub fn play(&self, list: &[Occurrence], voice_name: &dyn Fn(VoiceStyle) -> Option<String>) {
        let _run = self.run_lock.lock().unwrap_or_else(|e| e.into_inner());
        self.stop.store(false, Ordering::SeqCst);
        *self.playing.lock().unwrap_or_else(|e| e.into_inner()) = Some(list.iter().map(|o| o.title.as_str()).collect::<Vec<_>>().join(" / "));

        let mut sound = list.iter().find_map(|o| o.sound_id.as_deref()).and_then(sound_pcm);
        let mut speeches: Vec<Pcm> = Vec::new();
        for o in list.iter().filter(|o| o.speech.is_some()) {
            if self.stop.load(Ordering::SeqCst) {
                break;
            }
            match self.render(o, voice_name(o.voice).as_deref()) {
                Ok((pcm, _)) => speeches.push(pcm),
                Err(e) => {
                    // 読み上げできない環境(日本語音声が無い等)でも、無音で終わらないようチャイムに切り替える
                    self.store.log(&format!("speech failed: {e}"));
                    if sound.is_none() {
                        sound = sound_pcm("chime");
                    }
                }
            }
        }

        if !self.stop.load(Ordering::SeqCst) {
            match (concat_speeches(&speeches, 500), sound) {
                (Some(speech), Some(bed)) => {
                    let mixed = mix_sound_under_speech(&speech, &bed, SOUND_MAX);
                    winaudio::play_sync(&wav_bytes(&mixed));
                }
                (Some(speech), None) => {
                    winaudio::play_sync(&wav_bytes(&speech));
                }
                (None, Some(bed)) => {
                    let wav = wav_bytes(&bed);
                    if winaudio::play_loop(&wav) {
                        let start = Instant::now();
                        while start.elapsed() < SOUND_MAX && !self.stop.load(Ordering::SeqCst) {
                            std::thread::sleep(Duration::from_millis(100));
                        }
                        winaudio::stop_all();
                    }
                    drop(wav); // 停止した後で解放する
                }
                (None, None) => {}
            }
        }
        *self.playing.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(f: f64, sr: u32, secs: f64, amp: f32) -> Pcm {
        Pcm::new(
            (0..(sr as f64 * secs) as usize).map(|i| ((2.0 * std::f64::consts::PI * f * i as f64 / sr as f64).sin() as f32) * amp).collect(),
            sr,
        )
    }

    #[test]
    fn bundled_sounds_are_valid_wavs() {
        for id in ["chime", "alarm", "melody"] {
            let p = sound_pcm(id).unwrap_or_else(|| panic!("{id}を読めない"));
            assert_eq!(22_050, p.sample_rate);
            assert!(p.seconds() > 1.0, "{id}");
        }
        assert!(sound_pcm("siren").is_none());
    }

    #[test]
    fn mix_loops_the_sound_for_30_seconds_and_keeps_speech_audible() {
        let speech = sine(300.0, 24_000, 2.0, 0.5);
        let bed = sound_pcm("chime").unwrap(); // 22.05kHz: 読み上げのレート(24kHz)に揃えて混ぜる
        let mixed = mix_sound_under_speech(&speech, &bed, Duration::from_secs(30));
        assert_eq!(24_000, mixed.sample_rate);
        assert_eq!(30 * 24_000, mixed.samples.len()); // 読み上げより音のほうが長い
        let rms = |x: &[f32]| (x.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / x.len() as f64).sqrt();
        let speech_part = rms(&mixed.samples[..48_000]);
        let tail = rms(&mixed.samples[48_000 + 24_000..]);
        assert!(tail > 0.01, "読み上げが終わった後も、音が鳴り続ける");
        assert!(speech_part > 0.3, "読み上げの部分は、読み上げが主役: {speech_part}");
        assert!(mixed.samples.iter().all(|v| v.abs() <= 1.0));
    }

    #[test]
    fn mix_is_as_long_as_the_speech_when_the_speech_is_longer() {
        let speech = sine(300.0, 24_000, 40.0, 0.5);
        let mixed = mix_sound_under_speech(&speech, &sound_pcm("alarm").unwrap(), Duration::from_secs(30));
        assert_eq!(speech.samples.len(), mixed.samples.len());
    }

    #[test]
    fn mix_never_clips() {
        let loud = sine(200.0, 22_050, 1.0, 1.0);
        let mixed = mix_sound_under_speech(&loud, &sound_pcm("alarm").unwrap(), Duration::from_secs(2));
        assert!(mixed.samples.iter().fold(0f32, |m, v| m.max(v.abs())) <= 0.96);
    }

    #[test]
    fn concat_speeches_inserts_gaps_and_handles_rate_differences() {
        assert!(concat_speeches(&[], 500).is_none());
        let a = sine(300.0, 24_000, 1.0, 0.5);
        let b = sine(300.0, 22_050, 1.0, 0.5);
        let c = concat_speeches(&[a, b], 500).unwrap();
        assert_eq!(24_000, c.sample_rate);
        assert!((c.seconds() - 2.5).abs() < 0.01, "{}", c.seconds()); // 1s + 0.5s(間) + 1s
        let one = concat_speeches(&[sine(300.0, 24_000, 1.0, 0.5)], 500).unwrap();
        assert!((one.seconds() - 1.0).abs() < 0.001); // 1つだけなら、後ろに間を足さない
    }

    #[test]
    fn resample_keeps_duration() {
        let x: Vec<f32> = vec![0.1; 22_050];
        let y = resample(&x, 22_050, 24_000);
        assert_eq!(24_000, y.len());
        assert_eq!(x, resample(&x, 22_050, 22_050));
    }
}
