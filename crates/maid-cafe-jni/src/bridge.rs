//! JNIの向こう側の、型変換だけの薄い層(JVMなしでテストできる純Rust)。
//!
//! 受け渡しは文字列と数値配列だけにしている(JNIの型変換を最小にするため):
//! - アラーム: 保存形式そのもの([`maid_cafe_core::codec`]の1行=1件、Kotlin版と互換)
//! - 時刻: `LocalDateTime.toEpochSecond(ZoneOffset.UTC)`(タイムゾーンを持ち込まない「壁時計の秒」)
//! - カレンダー予定: 1行=1件の`id=..&title=..&start=<秒>`(値はURLエンコード)
//! - カレンダー設定: `enabled=1&pre=1&premin=30&voice=MAID&ph=a,b&pph=&harm=0`
//! - 発火(戻り値): 1行=1件の`t=<秒>&key=..&title=..&sound=..&speech=..&voice=..&harm=0|1&seg=<区切り>`
//!   区切りは`;`区切りで、各要素は`<URLエンコードした文章>,<音程倍率>,<話速倍率>,<後ろの間ms>`

use chrono::{DateTime, NaiveDateTime};
use maid_cafe_core::audio::{dsp, parse_wav, render, to_pcm16, Pcm, SourceGender};
use maid_cafe_core::codec::{decode_all, url_decode, url_encode};
use maid_cafe_core::{
    AlarmKind, CalendarEvent, CalendarSettings, JapaneseHolidays, MaidPhrases, Occurrence, Planner, SpeechText, VoiceStyle,
};
use std::collections::HashMap;

/// 三角ディザの種(Kotlin版・Windows版と同じ)。
const DITHER_SEED: i64 = 1234;

fn from_secs(t: i64) -> Option<NaiveDateTime> {
    DateTime::from_timestamp(t, 0).map(|d| d.naive_utc())
}

fn fields(line: &str) -> HashMap<String, String> {
    let mut m = HashMap::new();
    for part in line.split('&') {
        if let Some(i) = part.find('=') {
            if let Ok(v) = url_decode(&part[i + 1..]) {
                m.insert(part[..i].to_string(), v);
            }
        }
    }
    m
}

fn ids(s: Option<&String>) -> Vec<String> {
    maid_cafe_core::codec::decode_ids(s.map(|x| x.as_str()))
}

/// カレンダー予定。壊れた行は読み飛ばす。
pub fn decode_events(s: &str) -> Vec<CalendarEvent> {
    s.lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| {
            let m = fields(l);
            Some(CalendarEvent { id: m.get("id")?.clone(), title: m.get("title")?.clone(), start: from_secs(m.get("start")?.parse().ok()?)? })
        })
        .collect()
}

/// カレンダー設定。読めない項目は既定値(Kotlin版`Store.calendar`と同じ寛容さ)。
pub fn decode_settings(s: &str) -> CalendarSettings {
    let m = fields(s);
    let d = CalendarSettings::default();
    CalendarSettings {
        enabled: m.get("enabled").map(|v| v == "1").unwrap_or(d.enabled),
        pre_notice: m.get("pre").map(|v| v == "1").unwrap_or(d.pre_notice),
        pre_notice_minutes: m.get("premin").and_then(|v| v.parse::<u32>().ok()).filter(|v| *v > 0).unwrap_or(d.pre_notice_minutes),
        voice: m.get("voice").and_then(|v| VoiceStyle::from_name(v)).unwrap_or(d.voice),
        phrases: ids(m.get("ph")),
        pre_phrases: ids(m.get("pph")),
        harmony: m.get("harm").map(|v| v == "1").unwrap_or(d.harmony),
    }
}

fn encode_occurrence(o: &Occurrence) -> String {
    let segs = o
        .segments
        .iter()
        .map(|s| format!("{},{},{},{}", url_encode(&s.text), s.pitch, s.rate, s.gap_after_ms))
        .collect::<Vec<_>>()
        .join(";");
    format!(
        "t={}&key={}&title={}&sound={}&speech={}&voice={}&harm={}&lang={}&seg={}",
        o.time.and_utc().timestamp(),
        url_encode(&o.key),
        url_encode(&o.title),
        url_encode(o.sound_id.as_deref().unwrap_or("")),
        url_encode(o.speech.as_deref().unwrap_or("")),
        o.voice.name(),
        if o.harmony { 1 } else { 0 },
        o.lang,
        segs
    )
}

/// 次に鳴らすもの(同時刻が複数なら複数行)。`after`は「壁時計の秒」。無ければ空文字列。
pub fn plan_next(entries: &str, events: &str, settings: &str, after: i64) -> String {
    let Some(after) = from_secs(after) else { return String::new() };
    let list = Planner::next(&decode_all(entries), &decode_events(events), &decode_settings(settings), after, &JapaneseHolidays);
    list.into_iter().map(|mut o| {
        if o.voice == VoiceStyle::Maid {
            o.segments = SpeechText::intonate(std::mem::take(&mut o.segments));
        }
        encode_occurrence(&o)
    }).collect::<Vec<_>>().join("\n")
}

/// 指定時刻の読み上げ文(全体)。読み上げない設定なら`None`。
pub fn alarm_speech(kind: &str, text: &str, label: &str, voice: &str, ids_csv: &str) -> Option<String> {
    let kind = AlarmKind::from_name(kind)?;
    let voice = VoiceStyle::from_name(voice)?;
    let ids = maid_cafe_core::codec::decode_ids(Some(ids_csv));
    SpeechText::alarm_speech(kind, text, label, voice, &ids)
}

fn parse_numbers(s: &str) -> Vec<(String, i32)> {
    s.split(',')
        .filter_map(|p| {
            let (id, n) = p.rsplit_once(':')?;
            Some((id.to_string(), n.parse().ok()?))
        })
        .collect()
}

fn format_numbers(n: &[(String, i32)]) -> String {
    n.iter().map(|(id, v)| format!("{id}:{v}")).collect::<Vec<_>>().join(",")
}

/// セリフ番号表(`id:番号,id:番号`)の編集。`op`は`add`/`remove`/`assign`。未知のセリフ・操作は`Err`。
pub fn phrase_edit(numbers: &str, op: &str, id: &str, n: i32) -> Result<String, String> {
    if MaidPhrases::by_id(id).is_none() {
        return Err(format!("セリフが不正です: {id}"));
    }
    let cur = parse_numbers(numbers);
    let out = match op {
        "add" => MaidPhrases::add(&cur, id),
        "remove" => MaidPhrases::remove(&cur, id),
        "assign" => MaidPhrases::assign(&cur, id, n),
        _ => return Err(format!("操作が不正です: {op}")),
    };
    Ok(format_numbers(&out))
}

/// 番号の小さい順のid(`,`区切り)。
pub fn phrase_order(numbers: &str) -> String {
    MaidPhrases::ordered(&parse_numbers(numbers)).join(",")
}

/// 保存された順序(喋る順)から`1,2,3…`の番号を振った番号表。
pub fn phrase_ranks(ids_csv: &str) -> String {
    let ids = maid_cafe_core::codec::decode_ids(Some(ids_csv));
    format_numbers(&MaidPhrases::ranks(&ids))
}

fn gender(s: &str) -> SourceGender {
    match s {
        "FEMALE" => SourceGender::Female,
        "MALE" => SourceGender::Male,
        _ => SourceGender::Unknown,
    }
}

/// WAVのサンプルレート。読めなければ0。
pub fn wav_sample_rate(wav: &[u8]) -> i32 {
    parse_wav(wav).map(|p| p.sample_rate as i32).unwrap_or(0)
}

/// 1区切り分のTTS出力(WAV)を、声質に合わせて加工する。読めない・短すぎる(0.05秒未満)ときは`None`。
pub fn render_segment(wav: &[u8], voice: &str, source_gender: &str, harmony: bool, pitch: f64) -> Option<Vec<f32>> {
    let pcm = parse_wav(wav)?;
    if pcm.samples.len() < (pcm.sample_rate / 20) as usize {
        return None;
    }
    let style = VoiceStyle::from_name(voice)?;
    Some(render(&pcm, style, gender(source_gender), harmony, pitch).samples)
}

/// 区切りを、後ろの間を挟んで連結し、16bit PCM(三角ディザつき)にする。
pub fn join_pcm16(parts: &[Vec<f32>], gaps_ms: &[i32], sample_rate: i32) -> Vec<i16> {
    let sr = sample_rate.max(1) as u32;
    let joined = dsp::join(parts, gaps_ms, sr);
    to_pcm16(&Pcm::new(joined, sr).samples, DITHER_SEED)
}

#[cfg(test)]
mod tests;
