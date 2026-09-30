//! 保存形式。1行=1アラーム、`key=value&key=value`(値はURLエンコード)。JSONライブラリ非依存。
//!
//! Kotlin(Android/旧Windows)版と**バイト単位で互換**: 旧版が保存した行をそのまま読め、こちらが書いた行も旧版が読める
//! (エンコードはJavaの`URLEncoder`と同じ規則)。壊れた行・未知の値は[`decode_all`]が読み飛ばす
//! (アプリ更新で起動不能になるのを避ける)。

use crate::days::{parse_weekday, weekday_name, DaySet};
use crate::model::*;
use crate::recurrence::{Recurrence, Schedule};
use chrono::{NaiveDate, NaiveTime, Timelike};

/// Javaの`URLEncoder.encode(s, "UTF-8")`と同じ規則: 英数字と`.-*_`はそのまま、空白は`+`、他は`%XX`(大文字16進)。
pub fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'.' | b'-' | b'*' | b'_' => out.push(b as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// `URLDecoder.decode(s, "UTF-8")`相当。不正な表記は`Err`。
pub fn url_decode(s: &str) -> Result<String, String> {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' => {
                let hex = s.get(i + 1..i + 3).ok_or("不完全な%エスケープ")?;
                out.push(u8::from_str_radix(hex, 16).map_err(|_| "不正な%エスケープ".to_string())?);
                i += 3;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).map_err(|_| "UTF-8として不正".to_string())
}

fn days_str(d: &DaySet) -> String {
    d.iter().map(weekday_name).collect::<Vec<_>>().join(",")
}

fn parse_days(s: &str) -> Result<DaySet, String> {
    let mut set = DaySet::default();
    for name in s.split(',') {
        set.insert(parse_weekday(name).ok_or_else(|| format!("曜日名が不正: {name}"))?);
    }
    Ok(set)
}

pub fn encode_recurrence(r: &Recurrence) -> String {
    match r {
        Recurrence::Daily => "daily".into(),
        Recurrence::Weekdays { skip_holidays } => format!("weekdays:{}", if *skip_holidays { 1 } else { 0 }),
        Recurrence::WeekendsAndHolidays => "weekends".into(),
        Recurrence::DaysOfWeek(d) => format!("dow:{}", days_str(d)),
        Recurrence::NthWeekdayOfMonth { nth, day } => format!("nth:{nth}:{}", weekday_name(*day)),
        Recurrence::EveryNWeeks { interval, days, anchor } => {
            format!("every:{interval}:{}:{}", days_str(days), anchor.format("%Y-%m-%d"))
        }
    }
}

pub fn decode_recurrence(s: &str) -> Result<Recurrence, String> {
    let p: Vec<&str> = s.split(':').collect();
    let get = |i: usize| p.get(i).copied().ok_or_else(|| format!("繰り返し指定が不完全: {s}"));
    match p[0] {
        "daily" => Ok(Recurrence::Daily),
        "weekdays" => Ok(Recurrence::Weekdays { skip_holidays: p.get(1).copied() != Some("0") }),
        "weekends" => Ok(Recurrence::WeekendsAndHolidays),
        "dow" => Recurrence::days_of_week(parse_days(get(1)?)?),
        "nth" => {
            let nth: i32 = get(1)?.parse().map_err(|_| format!("nthが不正: {s}"))?;
            let day = parse_weekday(get(2)?).ok_or_else(|| format!("曜日名が不正: {s}"))?;
            Recurrence::nth_weekday_of_month(nth, day)
        }
        "every" => {
            let interval: u32 = get(1)?.parse().map_err(|_| format!("intervalが不正: {s}"))?;
            let anchor = NaiveDate::parse_from_str(get(3)?, "%Y-%m-%d").map_err(|e| e.to_string())?;
            Recurrence::every_n_weeks(interval, parse_days(get(2)?)?, anchor)
        }
        _ => Err(format!("unknown recurrence: {s}")),
    }
}

/// Kotlinの`LocalTime.toString()`と同じ: 秒が0なら`HH:mm`、そうでなければ`HH:mm:ss`。
fn time_str(t: NaiveTime) -> String {
    if t.second() == 0 && t.nanosecond() == 0 {
        t.format("%H:%M").to_string()
    } else {
        t.format("%H:%M:%S").to_string()
    }
}

fn parse_time(s: &str) -> Result<NaiveTime, String> {
    NaiveTime::parse_from_str(s, "%H:%M:%S")
        .or_else(|_| NaiveTime::parse_from_str(s, "%H:%M"))
        .map_err(|e| format!("時刻が不正 ({s}): {e}"))
}

pub fn encode(e: &AlarmEntry) -> String {
    let fields: Vec<(&str, String)> = vec![
        ("id", e.id.clone()),
        ("label", e.label.clone()),
        ("rec", encode_recurrence(&e.schedule.recurrence)),
        ("time", time_str(e.schedule.time)),
        ("start", e.schedule.start_date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_default()),
        ("end", e.schedule.end_date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_default()),
        ("pre", e.schedule.pre_notice_minutes.map(|m| m.to_string()).unwrap_or_default()),
        ("kind", e.kind.name().into()),
        ("sound", e.sound_id.clone()),
        ("text", e.text.clone()),
        ("voice", e.voice.name().into()),
        ("enabled", if e.enabled { "1" } else { "0" }.into()),
        ("ph", e.phrases.join(",")),
        ("pph", e.pre_phrases.join(",")),
        ("harm", if e.harmony { "1" } else { "0" }.into()),
    ];
    // 旧版と同じ行になるよう、オンのときだけ書く(旧版はこの項目を知らないが、読み飛ばすだけで壊れない)
    let mut fields = fields;
    if e.speech_sound {
        fields.push(("ss", "1".into()));
    }
    if e.lang != crate::lang::DEFAULT_LANG {
        fields.push(("lang", e.lang.clone()));
    }
    fields.iter().map(|(k, v)| format!("{k}={}", url_encode(v))).collect::<Vec<_>>().join("&")
}

/// 保存されたセリフidを復元(未知のidは捨てる。旧バージョンのデータには欠けているので空扱い)。
pub fn decode_ids(s: Option<&str>) -> Vec<String> {
    let ids: Vec<String> = s.unwrap_or("").split(',').filter(|x| !x.is_empty()).map(String::from).collect();
    MaidPhrases::known(&ids)
}

pub fn decode(line: &str) -> Result<AlarmEntry, String> {
    let mut m = std::collections::HashMap::new();
    for part in line.split('&') {
        let i = part.find('=').ok_or_else(|| format!("'='が無い項目: {part}"))?;
        m.insert(part[..i].to_string(), url_decode(&part[i + 1..])?);
    }
    let req = |k: &str| m.get(k).cloned().ok_or_else(|| format!("必須項目が無い: {k}"));
    let opt = |k: &str| m.get(k).filter(|v| !v.is_empty()).cloned();

    let parse_date = |s: String| NaiveDate::parse_from_str(&s, "%Y-%m-%d").map_err(|e| e.to_string());
    let schedule = Schedule {
        recurrence: decode_recurrence(&req("rec")?)?,
        time: parse_time(&req("time")?)?,
        start_date: opt("start").map(parse_date).transpose()?,
        end_date: opt("end").map(parse_date).transpose()?,
        pre_notice_minutes: opt("pre").map(|s| s.parse::<u32>().map_err(|e| e.to_string())).transpose()?,
    };
    if schedule.pre_notice_minutes == Some(0) {
        return Err("予告は1分以上前".into());
    }
    let sound = m.get("sound").filter(|s| is_known_sound(s)).cloned().unwrap_or_else(|| DEFAULT_SOUND_ID.into());
    Ok(AlarmEntry {
        id: req("id")?,
        label: req("label")?,
        schedule,
        kind: AlarmKind::from_name(&req("kind")?).ok_or("kindが不正")?,
        sound_id: sound,
        text: m.get("text").cloned().unwrap_or_default(),
        voice: VoiceStyle::from_name(&req("voice")?).ok_or("voiceが不正")?,
        enabled: m.get("enabled").map(|s| s.as_str()) != Some("0"),
        phrases: decode_ids(m.get("ph").map(|s| s.as_str())),
        pre_phrases: decode_ids(m.get("pph").map(|s| s.as_str())),
        harmony: m.get("harm").map(|s| s.as_str()) == Some("1"),
        speech_sound: m.get("ss").map(|s| s.as_str()) == Some("1"),
        lang: crate::lang::normalize(m.get("lang").map(|s| s.as_str()).unwrap_or("ja")).to_string(),
    })
}

pub fn encode_all(list: &[AlarmEntry]) -> String {
    list.iter().map(encode).collect::<Vec<_>>().join("\n")
}

/// 壊れた行は読み飛ばす。
pub fn decode_all(s: &str) -> Vec<AlarmEntry> {
    s.lines().filter(|l| !l.trim().is_empty()).filter_map(|l| decode(l).ok()).collect()
}
