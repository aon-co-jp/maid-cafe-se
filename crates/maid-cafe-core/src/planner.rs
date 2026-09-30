//! 登録アラームとカレンダー予定から「次に鳴らすもの」を決める。読み上げ文の生成もここ。

use crate::holidays::HolidayCalendar;
use crate::model::*;
use chrono::{Duration, NaiveDateTime};

/// 読み上げ文の生成。TTSが読み間違えやすい記号(♪等)は入れない。
pub struct SpeechText;

/// 本文(基本メッセージ)の後ろにセリフが続くときの間。
const BASE_GAP_MS: i32 = 300;

impl SpeechText {
    /// `base`の後ろに選択されたセリフを、**喋る順**(`ids`の並び)で続ける。何も無ければ`None`。
    pub fn with_phrases(base: Option<&str>, ids: &[String]) -> Option<String> {
        Self::with_phrases_in(base, ids, "ja")
    }

    /// [`with_phrases`](Self::with_phrases)の言語指定版。
    pub fn with_phrases_in(base: Option<&str>, ids: &[String], lang: &str) -> Option<String> {
        let mut parts: Vec<&str> = Vec::new();
        if let Some(b) = base {
            parts.push(b);
        }
        for id in ids {
            if let Some(p) = MaidPhrases::by_id(id) {
                parts.push(crate::lang::phrase_spoken(id, lang).unwrap_or(p.spoken));
            }
        }
        if parts.is_empty() {
            None
        } else {
            Some(parts.join(" "))
        }
    }

    /// [`with_phrases`](Self::with_phrases)と同じ内容を、セリフごとの間・抑揚つきの区切りで返す。
    pub fn segments(base: Option<&str>, ids: &[String]) -> Vec<Segment> {
        Self::segments_in(base, ids, "ja")
    }

    /// [`segments`](Self::segments)の言語指定版。
    pub fn segments_in(base: Option<&str>, ids: &[String], lang: &str) -> Vec<Segment> {
        let phrases: Vec<&Phrase> = ids.iter().filter_map(|id| MaidPhrases::by_id(id)).collect();
        let mut out = Vec::new();
        if let Some(b) = base {
            out.push(Segment {
                text: b.to_string(),
                pitch: 1.0,
                rate: 1.0,
                gap_after_ms: if phrases.is_empty() { 0 } else { BASE_GAP_MS },
            });
        }
        let last = phrases.len().saturating_sub(1);
        for (i, p) in phrases.iter().enumerate() {
            out.push(Segment {
                text: crate::lang::phrase_spoken(p.id, lang).unwrap_or(p.spoken).to_string(),
                pitch: p.pitch,
                rate: p.rate,
                gap_after_ms: if i == last { 0 } else { p.gap_ms },
            });
        }
        out
    }

    /// 指定時刻の基本メッセージ。読み上げ文が空でセリフだけ選ばれている場合は`None`(セリフのみ喋る)。
    pub fn alarm_base(kind: AlarmKind, text: &str, label: &str, voice: VoiceStyle, ids: &[String]) -> Option<String> {
        Self::alarm_base_in(kind, text, label, voice, ids, "ja")
    }

    pub fn alarm_base_in(kind: AlarmKind, text: &str, label: &str, voice: VoiceStyle, ids: &[String], lang: &str) -> Option<String> {
        if kind == AlarmKind::Speech && !(text.trim().is_empty() && !ids.is_empty()) {
            let t = if text.trim().is_empty() { label } else { text };
            Some(Self::alarm_in(t, voice, lang))
        } else {
            None
        }
    }

    /// 指定時刻の読み上げ文(全体)。
    pub fn alarm_speech(kind: AlarmKind, text: &str, label: &str, voice: VoiceStyle, ids: &[String]) -> Option<String> {
        Self::alarm_speech_in(kind, text, label, voice, ids, "ja")
    }

    pub fn alarm_speech_in(kind: AlarmKind, text: &str, label: &str, voice: VoiceStyle, ids: &[String], lang: &str) -> Option<String> {
        Self::with_phrases_in(Self::alarm_base_in(kind, text, label, voice, ids, lang).as_deref(), ids, lang)
    }

    pub fn alarm(text: &str, voice: VoiceStyle) -> String {
        Self::alarm_in(text, voice, "ja")
    }

    pub fn alarm_in(text: &str, voice: VoiceStyle, lang: &str) -> String {
        if let Some(m) = crate::lang::alarm_message(text, voice, lang) {
            return m;
        }
        match voice {
            VoiceStyle::Maid => format!("ご主人様、お時間ですよ。{}。忘れずにお願いしますね", text.trim()),
            VoiceStyle::DeepMale => format!("時間だ。{}", text.trim()),
        }
    }

    pub fn pre_notice(title: &str, minutes: u32, voice: VoiceStyle) -> String {
        Self::pre_notice_in(title, minutes, voice, "ja")
    }

    pub fn pre_notice_in(title: &str, minutes: u32, voice: VoiceStyle, lang: &str) -> String {
        if let Some(m) = crate::lang::pre_notice_message(title, minutes, voice, lang) {
            return m;
        }
        match voice {
            VoiceStyle::Maid => format!("ご主人様、あと{minutes}分で、{title}のお時間ですわ"),
            VoiceStyle::DeepMale => format!("あと{minutes}分で、{title}の時間だ"),
        }
    }

    pub fn calendar(title: &str, voice: VoiceStyle) -> String {
        match voice {
            VoiceStyle::Maid => format!("ご主人様、{title}のお時間です"),
            VoiceStyle::DeepMale => format!("{title}の時間だ"),
        }
    }
}

/// 登録アラームとカレンダー予定から「次に鳴らすもの」を決める純粋関数。
pub struct Planner;

impl Planner {
    /// `after`より後で最も早い発火時刻の[`Occurrence`]を全件返す(同時刻が複数あれば複数)。無ければ空。
    /// カレンダー予定は開始時刻に読み上げ、`settings.pre_notice`なら`pre_notice_minutes`分前にも予告。
    pub fn next(
        entries: &[AlarmEntry],
        events: &[CalendarEvent],
        settings: &CalendarSettings,
        after: NaiveDateTime,
        holidays: &dyn HolidayCalendar,
    ) -> Vec<Occurrence> {
        let mut candidates: Vec<Occurrence> = Vec::new();
        for e in entries {
            if !e.enabled {
                continue;
            }
            if let Some(t) = e.schedule.next_trigger(after, holidays) {
                let base = SpeechText::alarm_base_in(e.kind, &e.text, &e.label, e.voice, &e.phrases, &e.lang);
                candidates.push(Occurrence {
                    time: t,
                    key: format!("alarm:{}", e.id),
                    title: e.label.clone(),
                    sound_id: if e.kind == AlarmKind::Sound || e.speech_sound { Some(e.sound_id.clone()) } else { None },
                    speech: SpeechText::alarm_speech_in(e.kind, &e.text, &e.label, e.voice, &e.phrases, &e.lang),
                    voice: e.voice,
                    harmony: e.harmony,
                    segments: SpeechText::segments_in(base.as_deref(), &e.phrases, &e.lang),
                    lang: e.lang.clone(),
                });
            }
            if let (Some(m), Some(t)) = (e.schedule.pre_notice_minutes, e.schedule.next_pre_notice(after, holidays)) {
                let base = SpeechText::pre_notice_in(&e.label, m, e.voice, &e.lang);
                candidates.push(Occurrence {
                    time: t,
                    key: format!("pre:{}", e.id),
                    title: e.label.clone(),
                    sound_id: None,
                    speech: SpeechText::with_phrases_in(Some(&base), &e.pre_phrases, &e.lang),
                    voice: e.voice,
                    harmony: e.harmony,
                    segments: SpeechText::segments_in(Some(&base), &e.pre_phrases, &e.lang),
                    lang: e.lang.clone(),
                });
            }
        }
        if settings.enabled {
            for ev in events {
                if ev.start > after {
                    let base = SpeechText::calendar(&ev.title, settings.voice);
                    candidates.push(Occurrence {
                        time: ev.start,
                        key: format!("cal:{}@{}", ev.id, iso(ev.start)),
                        title: ev.title.clone(),
                        sound_id: None,
                        speech: SpeechText::with_phrases(Some(&base), &settings.phrases),
                        voice: settings.voice,
                        harmony: settings.harmony,
                        segments: SpeechText::segments(Some(&base), &settings.phrases),
                        lang: "ja".to_string(),
                    });
                }
                let pre = ev.start - Duration::minutes(settings.pre_notice_minutes as i64);
                if settings.pre_notice && pre > after {
                    let base = SpeechText::pre_notice(&ev.title, settings.pre_notice_minutes, settings.voice);
                    candidates.push(Occurrence {
                        time: pre,
                        key: format!("calpre:{}@{}", ev.id, iso(ev.start)),
                        title: ev.title.clone(),
                        sound_id: None,
                        speech: SpeechText::with_phrases(Some(&base), &settings.pre_phrases),
                        voice: settings.voice,
                        harmony: settings.harmony,
                        segments: SpeechText::segments(Some(&base), &settings.pre_phrases),
                        lang: "ja".to_string(),
                    });
                }
            }
        }
        let Some(first) = candidates.iter().map(|c| c.time).min() else {
            return Vec::new();
        };
        candidates.into_iter().filter(|c| c.time == first).collect()
    }
}

/// Kotlin版の`LocalDateTime.toString()`に合わせた表記(キーの一意性用)。
fn iso(t: NaiveDateTime) -> String {
    t.format("%Y-%m-%dT%H:%M").to_string()
}

impl SpeechText {
    /// メイドちゃんの声に**抑揚**を付ける。端末のTTSは一本調子に読むので、区切り(セリフ・本文)を読点・句点・感嘆符で
    /// 文節に割り、文節ごとに音程と話速を変えて合成し直す(短い間を挟む)。
    /// 音程は文節ごとに少しずつ上下させ、「！」「？」で終わる文節は語尾を上げてゆっくり、「。」で終わる文節は少し下げる。
    /// 低い男性の声には使わない(呼び出し側で声質を見る)。短い区切り(8文字未満)はそのまま。
    pub fn intonate(segments: Vec<Segment>) -> Vec<Segment> {
        const BOUNCE: [f64; 4] = [1.00, 1.05, 0.98, 1.06];
        let mut out = Vec::new();
        for s in segments {
            let clauses = split_clauses(&s.text);
            if s.text.chars().count() < 8 || clauses.len() < 2 {
                out.push(s);
                continue;
            }
            let last = clauses.len() - 1;
            for (i, c) in clauses.into_iter().enumerate() {
                let mut pitch = s.pitch * BOUNCE[i % BOUNCE.len()];
                let mut rate = s.rate;
                match c.chars().last() {
                    Some('！') | Some('!') | Some('？') | Some('?') | Some('؟') => {
                        pitch *= 1.06;
                        rate *= 0.95;
                    }
                    Some('。') | Some('.') => pitch *= 0.97,
                    _ => {}
                }
                let gap = if i == last {
                    s.gap_after_ms
                } else if c.ends_with(['、', ',', '，', '،']) {
                    90
                } else {
                    140
                };
                out.push(Segment {
                    text: c,
                    pitch,
                    rate,
                    gap_after_ms: gap,
                });
            }
        }
        out
    }
}

/// 読点・句点・感嘆符・疑問符の後ろで割る(区切り記号は前の文節に残す)。3文字未満の文節は次の文節にくっつける。
fn split_clauses(text: &str) -> Vec<String> {
    let mut raw: Vec<String> = Vec::new();
    let mut cur = String::new();
    for ch in text.chars() {
        cur.push(ch);
        if matches!(ch, '、' | '。' | '！' | '？' | '!' | '?' | ',' | '.' | '，' | '；' | '؟' | '،' | ';') {
            raw.push(std::mem::take(&mut cur));
        }
    }
    if !cur.trim().is_empty() {
        raw.push(cur);
    }
    let mut out: Vec<String> = Vec::new();
    let mut carry = String::new();
    for r in raw {
        carry.push_str(&r);
        if carry.chars().filter(|c| !c.is_whitespace()).count() >= 3 {
            out.push(std::mem::take(&mut carry));
        }
    }
    if !carry.is_empty() {
        match out.last_mut() {
            Some(l) => l.push_str(&carry),
            None => out.push(carry),
        }
    }
    out.into_iter()
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty())
        .collect()
}
