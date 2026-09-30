//! maid-cafe-se のコア。OSにも画面にも依存しない純Rust。
//!
//! - [`recurrence`]: 繰り返しルール(毎日/平日/土日祝/曜日/第N週/N週ごと)と、次の発火時刻の計算
//! - [`holidays`]: 日本の祝日(ルールから計算、2007〜2099年)
//! - [`model`]: アラーム・予定・設定のデータと、メイドのセリフ(喋る順の番号つき)
//! - [`planner`]: 「次に鳴らすもの」と読み上げ文の生成
//! - [`codec`]: 保存形式(旧Kotlin版と互換)
//! - [`audio`]: TTS出力の後処理(声質変換・ハモり・音量統一)

pub mod audio;
pub mod codec;
pub mod days;
pub mod holidays;
pub mod model;
pub mod planner;
pub mod recurrence;

pub use days::DaySet;
pub use holidays::{HolidayCalendar, JapaneseHolidays, NoHolidays};
pub use model::*;
pub use planner::{Planner, SpeechText};
pub use recurrence::{Recurrence, Schedule};
