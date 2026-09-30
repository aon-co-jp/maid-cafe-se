//! 画面(Web UI)とやり取りするJSONの型と、coreの型との変換・検証。

use chrono::{NaiveDate, NaiveTime};
use maid_cafe_core::codec;
use maid_cafe_core::days::{parse_weekday, weekday_name};
use maid_cafe_core::{
    is_known_sound, AlarmEntry, AlarmKind, DaySet, MaidPhrases, Recurrence, Schedule, VoiceStyle, DEFAULT_SOUND_ID,
};
use serde::{Deserialize, Serialize};

/// 繰り返しルール(画面用のJSON表現)。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RecurrenceDto {
    Daily,
    Weekdays { skip_holidays: bool },
    Weekends,
    Dow { days: Vec<String> },
    Nth { nth: i32, day: String },
    Every { interval: u32, days: Vec<String>, anchor: String },
}

/// アラーム1件(画面用のJSON表現)。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AlarmDto {
    pub id: String,
    pub label: String,
    /// `HH:MM`
    pub time: String,
    pub recurrence: RecurrenceDto,
    /// `SOUND` または `SPEECH`
    pub kind: String,
    #[serde(default)]
    pub sound: String,
    #[serde(default)]
    pub text: String,
    /// `MAID` または `DEEP_MALE`
    pub voice: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    /// 指定時刻に喋るセリフ(喋る順)
    #[serde(default)]
    pub phrases: Vec<String>,
    /// 予告時に喋るセリフ(喋る順)
    #[serde(default)]
    pub pre_phrases: Vec<String>,
    #[serde(default)]
    pub harmony: bool,
    /// 何分前に予告するか(予告しないなら`null`)
    #[serde(default)]
    pub pre_notice_minutes: Option<u32>,
}

fn yes() -> bool {
    true
}

fn days_to_dto(d: &DaySet) -> Vec<String> {
    d.iter().map(|w| weekday_name(w).to_string()).collect()
}

fn days_from_dto(v: &[String]) -> Result<DaySet, String> {
    let mut set = DaySet::default();
    for n in v {
        set.insert(parse_weekday(n).ok_or_else(|| format!("曜日名が不正です: {n}"))?);
    }
    Ok(set)
}

impl RecurrenceDto {
    pub fn from_core(r: &Recurrence) -> Self {
        match r {
            Recurrence::Daily => RecurrenceDto::Daily,
            Recurrence::Weekdays { skip_holidays } => RecurrenceDto::Weekdays { skip_holidays: *skip_holidays },
            Recurrence::WeekendsAndHolidays => RecurrenceDto::Weekends,
            Recurrence::DaysOfWeek(d) => RecurrenceDto::Dow { days: days_to_dto(d) },
            Recurrence::NthWeekdayOfMonth { nth, day } => RecurrenceDto::Nth { nth: *nth, day: weekday_name(*day).to_string() },
            Recurrence::EveryNWeeks { interval, days, anchor } => {
                RecurrenceDto::Every { interval: *interval, days: days_to_dto(days), anchor: anchor.format("%Y-%m-%d").to_string() }
            }
        }
    }

    pub fn to_core(&self) -> Result<Recurrence, String> {
        match self {
            RecurrenceDto::Daily => Ok(Recurrence::Daily),
            RecurrenceDto::Weekdays { skip_holidays } => Ok(Recurrence::Weekdays { skip_holidays: *skip_holidays }),
            RecurrenceDto::Weekends => Ok(Recurrence::WeekendsAndHolidays),
            RecurrenceDto::Dow { days } => Recurrence::days_of_week(days_from_dto(days)?),
            RecurrenceDto::Nth { nth, day } => {
                Recurrence::nth_weekday_of_month(*nth, parse_weekday(day).ok_or_else(|| format!("曜日名が不正です: {day}"))?)
            }
            RecurrenceDto::Every { interval, days, anchor } => {
                let a = NaiveDate::parse_from_str(anchor, "%Y-%m-%d").map_err(|_| format!("基準日が不正です: {anchor}"))?;
                Recurrence::every_n_weeks(*interval, days_from_dto(days)?, a)
            }
        }
    }
}

impl AlarmDto {
    pub fn from_core(e: &AlarmEntry) -> Self {
        AlarmDto {
            id: e.id.clone(),
            label: e.label.clone(),
            time: e.schedule.time.format("%H:%M").to_string(),
            recurrence: RecurrenceDto::from_core(&e.schedule.recurrence),
            kind: e.kind.name().to_string(),
            sound: e.sound_id.clone(),
            text: e.text.clone(),
            voice: e.voice.name().to_string(),
            enabled: e.enabled,
            phrases: e.phrases.clone(),
            pre_phrases: e.pre_phrases.clone(),
            harmony: e.harmony,
            pre_notice_minutes: e.schedule.pre_notice_minutes,
        }
    }

    /// 検証して、coreの型にする。不正な入力は日本語のメッセージの`Err`(画面にそのまま出せる)。
    pub fn to_core(&self) -> Result<AlarmEntry, String> {
        if self.id.is_empty() || self.id.len() > 64 {
            return Err("idが不正です".into());
        }
        if self.label.chars().count() > 100 {
            return Err("名前が長すぎます(100文字まで)".into());
        }
        if self.text.chars().count() > 500 {
            return Err("読み上げる文章が長すぎます(500文字まで)".into());
        }
        let time = NaiveTime::parse_from_str(&self.time, "%H:%M").map_err(|_| format!("時刻が不正です: {}", self.time))?;
        let mut schedule = Schedule::new(self.recurrence.to_core()?, time);
        if let Some(m) = self.pre_notice_minutes {
            if m == 0 || m > 24 * 60 {
                return Err("予告は1分〜24時間前で指定してください".into());
            }
            schedule = schedule.with_pre_notice(m)?;
        }
        let kind = AlarmKind::from_name(&self.kind).ok_or_else(|| format!("鳴らし方が不正です: {}", self.kind))?;
        let voice = VoiceStyle::from_name(&self.voice).ok_or_else(|| format!("声が不正です: {}", self.voice))?;
        let sound = if self.sound.is_empty() { DEFAULT_SOUND_ID.to_string() } else { self.sound.clone() };
        if !is_known_sound(&sound) {
            return Err(format!("音が不正です: {sound}"));
        }
        let mut e = AlarmEntry::new(&self.id, if self.label.trim().is_empty() { "アラーム" } else { self.label.trim() }, schedule, kind);
        e.sound_id = sound;
        e.text = self.text.clone();
        e.voice = voice;
        e.enabled = self.enabled;
        // 未知のセリフidと重複は捨てる(喋る順は保つ)
        e.phrases = MaidPhrases::known(&self.phrases);
        e.pre_phrases = MaidPhrases::known(&self.pre_phrases);
        e.harmony = self.harmony;
        // 保存形式で往復できることを保証する
        codec::decode(&codec::encode(&e)).map_err(|err| format!("保存できない値が含まれています: {err}"))?;
        // 読み上げなのに、文章もセリフも名前も無いものは作れない
        if e.kind == AlarmKind::Speech && e.text.trim().is_empty() && e.phrases.is_empty() && self.label.trim().is_empty() {
            return Err("読み上げる文章、メイドのセリフ、名前のどれかを入れてください".into());
        }
        Ok(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> AlarmDto {
        AlarmDto {
            id: "a1".into(),
            label: "起床".into(),
            time: "07:30".into(),
            recurrence: RecurrenceDto::Weekdays { skip_holidays: true },
            kind: "SPEECH".into(),
            sound: "chime".into(),
            text: "薬を飲む".into(),
            voice: "MAID".into(),
            enabled: true,
            phrases: vec!["okite".into(), "okaeri".into()],
            pre_phrases: vec!["fight".into()],
            harmony: true,
            pre_notice_minutes: Some(30),
        }
    }

    #[test]
    fn dto_round_trips_through_core_for_every_recurrence() {
        let recs = vec![
            RecurrenceDto::Daily,
            RecurrenceDto::Weekdays { skip_holidays: false },
            RecurrenceDto::Weekends,
            RecurrenceDto::Dow { days: vec!["MONDAY".into(), "FRIDAY".into()] },
            RecurrenceDto::Nth { nth: -1, day: "SUNDAY".into() },
            RecurrenceDto::Every { interval: 3, days: vec!["MONDAY".into(), "THURSDAY".into()], anchor: "2026-09-07".into() },
        ];
        for r in recs {
            let dto = AlarmDto { recurrence: r, ..sample() };
            let core = dto.to_core().expect("有効な入力");
            assert_eq!(dto, AlarmDto::from_core(&core));
        }
    }

    #[test]
    fn json_shape_is_stable() {
        let json = serde_json::to_value(sample()).unwrap();
        assert_eq!("weekdays", json["recurrence"]["kind"]);
        assert_eq!(true, json["recurrence"]["skip_holidays"]);
        assert_eq!(30, json["pre_notice_minutes"]);
        let back: AlarmDto = serde_json::from_value(json).unwrap();
        assert_eq!(sample(), back);
        // 省略できる項目(enabled等)には既定値が入る
        let minimal: AlarmDto = serde_json::from_str(
            r#"{"id":"x","label":"","time":"06:00","recurrence":{"kind":"daily"},"kind":"SOUND","voice":"MAID"}"#,
        )
        .unwrap();
        assert!(minimal.enabled && minimal.phrases.is_empty() && minimal.pre_notice_minutes.is_none());
    }

    #[test]
    fn validation_rejects_bad_input_with_japanese_messages() {
        let bad = |f: &dyn Fn(&mut AlarmDto)| {
            let mut d = sample();
            f(&mut d);
            d.to_core().expect_err("不正な入力")
        };
        assert!(bad(&|d| d.time = "25:00".into()).contains("時刻"));
        assert!(bad(&|d| d.kind = "NOISE".into()).contains("鳴らし方"));
        assert!(bad(&|d| d.voice = "ROBOT".into()).contains("声"));
        assert!(bad(&|d| d.sound = "siren".into()).contains("音"));
        assert!(bad(&|d| d.pre_notice_minutes = Some(0)).contains("予告"));
        assert!(bad(&|d| d.id = String::new()).contains("id"));
        assert!(bad(&|d| d.recurrence = RecurrenceDto::Dow { days: vec![] }).contains("曜日"));
        assert!(bad(&|d| d.recurrence = RecurrenceDto::Nth { nth: 6, day: "MONDAY".into() }).contains("nth"));
        assert!(bad(&|d| d.recurrence = RecurrenceDto::Dow { days: vec!["FUNDAY".into()] }).contains("曜日名"));
        assert!(bad(&|d| d.text = "あ".repeat(501)).contains("長すぎ"));
        assert!(bad(&|d| {
            d.label = "  ".into();
            d.text = String::new();
            d.phrases.clear();
        })
        .contains("どれか"));
    }

    #[test]
    fn unknown_phrases_are_dropped_and_blank_label_gets_a_default() {
        let mut d = sample();
        d.phrases = vec!["okaeri".into(), "bogus".into(), "okaeri".into(), "fight".into()];
        d.label = "   ".into();
        let e = d.to_core().unwrap();
        assert_eq!(vec!["okaeri".to_string(), "fight".to_string()], e.phrases);
        assert_eq!("アラーム", e.label);
    }
}
