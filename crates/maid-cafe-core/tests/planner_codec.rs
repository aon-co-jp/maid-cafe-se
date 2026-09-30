mod common;
use chrono::{Datelike, NaiveDateTime, Weekday};
use common::{date, dt, s, time};
use maid_cafe_core::codec;
use maid_cafe_core::*;

fn entry(id: &str, kind: AlarmKind, pre: Option<u32>, at: (u32, u32), enabled: bool, voice: VoiceStyle) -> AlarmEntry {
    let mut sch = Schedule::new(Recurrence::Daily, time(at.0, at.1));
    if let Some(p) = pre {
        sch = sch.with_pre_notice(p).unwrap();
    }
    let mut e = AlarmEntry::new(id, "起床", sch, kind);
    e.text = "薬を飲む".into();
    e.voice = voice;
    e.enabled = enabled;
    e
}

fn plain(kind: AlarmKind) -> AlarmEntry {
    entry("a", kind, None, (7, 0), true, VoiceStyle::Maid)
}

fn next(entries: &[AlarmEntry], events: &[CalendarEvent], settings: &CalendarSettings, after: NaiveDateTime) -> Vec<Occurrence> {
    Planner::next(entries, events, settings, after, &NoHolidays)
}

#[test]
fn sound_entry() {
    let r = next(&[plain(AlarmKind::Sound)], &[], &CalendarSettings::default(), dt(2026, 9, 30, 0, 0));
    assert_eq!(1, r.len());
    assert_eq!(dt(2026, 9, 30, 7, 0), r[0].time);
    assert_eq!(Some("chime".to_string()), r[0].sound_id);
    assert_eq!(None, r[0].speech);
}

#[test]
fn speech_entry_maid_and_deep() {
    let maid = next(&[plain(AlarmKind::Speech)], &[], &CalendarSettings::default(), dt(2026, 9, 30, 0, 0));
    assert_eq!(None, maid[0].sound_id);
    let sp = maid[0].speech.clone().unwrap();
    assert!(sp.contains("ご主人様") && sp.contains("薬を飲む"));
    let deep = next(
        &[entry("a", AlarmKind::Speech, None, (7, 0), true, VoiceStyle::DeepMale)],
        &[],
        &CalendarSettings::default(),
        dt(2026, 9, 30, 0, 0),
    );
    assert_eq!(Some("時間だ。薬を飲む".to_string()), deep[0].speech);
    assert_eq!(VoiceStyle::DeepMale, deep[0].voice);
}

#[test]
fn pre_notice_comes_first() {
    let r = next(
        &[entry("a", AlarmKind::Sound, Some(30), (7, 0), true, VoiceStyle::Maid)],
        &[],
        &CalendarSettings::default(),
        dt(2026, 9, 30, 0, 0),
    );
    assert_eq!(dt(2026, 9, 30, 6, 30), r[0].time);
    assert!(r[0].key.starts_with("pre:"));
    assert!(r[0].speech.clone().unwrap().contains("30分"));
}

#[test]
fn disabled_ignored() {
    let e = entry("a", AlarmKind::Sound, None, (7, 0), false, VoiceStyle::Maid);
    assert!(next(&[e], &[], &CalendarSettings::default(), dt(2026, 9, 30, 0, 0)).is_empty());
}

fn event(h: u32) -> CalendarEvent {
    CalendarEvent { id: "e1".into(), title: "会議".into(), start: dt(2026, 9, 30, h, 0) }
}

#[test]
fn calendar_events_with_pre_notice() {
    let on = CalendarSettings { enabled: true, pre_notice: true, ..Default::default() };
    let r = next(&[], &[event(10)], &on, dt(2026, 9, 30, 8, 0));
    assert_eq!(dt(2026, 9, 30, 9, 30), r[0].time);
    assert!(r[0].speech.clone().unwrap().contains("会議"));
    let r2 = next(&[], &[event(10)], &on, dt(2026, 9, 30, 9, 30));
    assert_eq!(dt(2026, 9, 30, 10, 0), r2[0].time);
}

#[test]
fn calendar_pre_notice_checkbox_off_and_disabled() {
    let off = CalendarSettings { enabled: true, pre_notice: false, ..Default::default() };
    assert_eq!(dt(2026, 9, 30, 10, 0), next(&[], &[event(10)], &off, dt(2026, 9, 30, 8, 0))[0].time);
    assert!(next(&[], &[event(10)], &CalendarSettings::default(), dt(2026, 9, 30, 8, 0)).is_empty());
}

#[test]
fn simultaneous_occurrences_returned_together() {
    let ev = CalendarEvent { id: "e1".into(), title: "会議".into(), start: dt(2026, 9, 30, 7, 0) };
    let st = CalendarSettings { enabled: true, pre_notice: false, ..Default::default() };
    assert_eq!(2, next(&[plain(AlarmKind::Sound)], &[ev], &st, dt(2026, 9, 30, 0, 0)).len());
}

#[test]
fn event_in_past_ignored() {
    let st = CalendarSettings { enabled: true, ..Default::default() };
    assert!(next(&[], &[event(10)], &st, dt(2026, 9, 30, 10, 0)).is_empty());
}

#[test]
fn codec_round_trip_all_recurrences() {
    let anchor = date(2026, 9, 7);
    let recs = vec![
        Recurrence::Daily,
        Recurrence::Weekdays { skip_holidays: true },
        Recurrence::Weekdays { skip_holidays: false },
        Recurrence::WeekendsAndHolidays,
        Recurrence::days_of_week(DaySet::new(&[Weekday::Mon, Weekday::Fri])).unwrap(),
        Recurrence::nth_weekday_of_month(2, Weekday::Tue).unwrap(),
        Recurrence::nth_weekday_of_month(-1, Weekday::Sun).unwrap(),
        Recurrence::every_n_weeks(3, DaySet::new(&[Weekday::Mon, Weekday::Thu]), anchor).unwrap(),
    ];
    let list: Vec<AlarmEntry> = recs
        .into_iter()
        .enumerate()
        .map(|(i, r)| {
            let sch = Schedule::new(r, time(6, 5)).with_period(Some(date(2026, 1, 1)), Some(date(2027, 1, 1))).with_pre_notice(30).unwrap();
            let mut e = AlarmEntry::new(&format!("id{i}"), "ラベル&=% 日本語\n改行", sch, AlarmKind::Speech);
            e.sound_id = "melody".into();
            e.text = "本文 a=b&c\n2行目".into();
            e.voice = VoiceStyle::DeepMale;
            e.enabled = i % 2 == 0;
            e
        })
        .collect();
    assert_eq!(list, codec::decode_all(&codec::encode_all(&list)));
}

#[test]
fn codec_nullable_fields_and_broken_lines() {
    let e = plain(AlarmKind::Sound);
    let text = codec::encode_all(std::slice::from_ref(&e)) + "\nこわれた行\nid=x&label=y";
    assert_eq!(vec![e], codec::decode_all(&text));
    assert!(codec::decode_all("").is_empty());
}

#[test]
fn unknown_sound_falls_back_to_default() {
    let line = codec::encode(&plain(AlarmKind::Sound)).replace("sound=chime", "sound=nonexistent");
    assert_eq!("chime", codec::decode(&line).unwrap().sound_id);
}

/// Kotlin(Android)版が保存した行を、そのまま読める(互換性の固定)。
#[test]
fn reads_lines_written_by_the_kotlin_version() {
    let line = "id=t1&label=%E8%AA%AD%E3%81%BF%E4%B8%8A%E3%81%92%E4%BD%8E%E9%9F%B3&rec=daily&time=01%3A21&start=&end=&pre=&kind=SPEECH&sound=alarm&text=%E4%BC%9A%E8%AD%B0%E3%81%AE%E6%BA%96%E5%82%99%E3%82%92%E3%81%99%E3%82%8B&voice=DEEP_MALE&enabled=1";
    let e = codec::decode(line).unwrap();
    assert_eq!("読み上げ低音", e.label);
    assert_eq!(time(1, 21), e.schedule.time);
    assert_eq!(AlarmKind::Speech, e.kind);
    assert_eq!(VoiceStyle::DeepMale, e.voice);
    assert_eq!("会議の準備をする", e.text);
    assert!(e.phrases.is_empty() && !e.harmony); // ph/pph/harm無しの旧データ
}

/// こちらが書いた行のURLエンコードは、JavaのURLEncoderと同じ規則(空白は+、~は%7E、.-*_はそのまま)。
#[test]
fn url_encoding_matches_java() {
    assert_eq!("a+b%26c%3Dd", codec::url_encode("a b&c=d"));
    assert_eq!("%7E.-*_", codec::url_encode("~.-*_"));
    assert_eq!("%E6%97%A5", codec::url_encode("日"));
    assert_eq!("a b&c=d~日", codec::url_decode("a+b%26c%3Dd%7E%E6%97%A5").unwrap());
    assert!(codec::url_decode("%zz").is_err());
    let _ = date(2026, 1, 1).year();
    let _ = s(&[]);
}

#[test]
fn speech_alarm_is_voice_only_unless_sound_is_turned_on() {
    let mut e = plain(AlarmKind::Speech);
    let after = dt(2026, 9, 30, 0, 0);
    let voice_only = Planner::next(std::slice::from_ref(&e), &[], &CalendarSettings::default(), after, &NoHolidays).remove(0);
    assert!(voice_only.sound_id.is_none() && voice_only.speech.is_some());
    // 「音も一緒に鳴らす」をオンにすると、声と音が同時
    e.speech_sound = true;
    let both = Planner::next(std::slice::from_ref(&e), &[], &CalendarSettings::default(), after, &NoHolidays).remove(0);
    assert_eq!(Some(e.sound_id.clone()), both.sound_id);
    assert!(both.speech.is_some());
    // 保存形式: オンのときだけ`ss=1`が付く(オフの行は従来と同じ)。往復しても保たれる
    assert!(codec::encode(&e).contains("&ss=1"));
    assert!(!codec::encode(&plain(AlarmKind::Speech)).contains("ss="));
    assert!(codec::decode(&codec::encode(&e)).unwrap().speech_sound);
    assert!(!codec::decode(&codec::encode(&plain(AlarmKind::Speech))).unwrap().speech_sound);
}

#[test]
fn every_language_speaks_its_own_phrases_and_round_trips() {
    let after = dt(2026, 9, 30, 0, 0);
    for (code, _, _) in lang::LANGS {
        let mut e = plain(AlarmKind::Speech);
        e.lang = code.to_string();
        e.text = "pills".into();
        e.phrases = vec!["okaeri".into(), "fight".into()];
        let occ = Planner::next(std::slice::from_ref(&e), &[], &CalendarSettings::default(), after, &NoHolidays);
        let speech = occ[0].speech.clone().unwrap();
        assert_eq!(code, occ[0].lang);
        // 日本語は従来どおり、それ以外は日本語の文字(ひらがな・カタカナ)を含まない(=その言語のセリフ・定型文になっている)
        let has_kana = speech.chars().any(|c| ('\u{3040}'..='\u{30ff}').contains(&c));
        assert_eq!(code == "ja", has_kana, "{code}: {speech}");
        assert!(speech.contains("pills"), "{code}: 入力した文章は翻訳されず、そのまま入る");
        // 予告も同じ言語(日本語の文字を含まない)
        let mut p = e.clone();
        p.schedule.pre_notice_minutes = Some(30);
        let pre = Planner::next(std::slice::from_ref(&p), &[], &CalendarSettings::default(), after, &NoHolidays).remove(0);
        assert_eq!(code == "ja", pre.speech.unwrap().chars().any(|c| ('\u{3040}'..='\u{30ff}').contains(&c)), "{code} 予告");
        // 保存形式: 日本語以外だけ`lang=`が付き、往復で保たれる
        let line = codec::encode(&e);
        assert_eq!(code != "ja", line.contains("&lang="), "{line}");
        assert_eq!(code, codec::decode(&line).unwrap().lang);
    }
    // 未知の言語コードは日本語にする(壊れたデータで起動不能にしない)
    let line = codec::encode(&plain(AlarmKind::Speech)) + "&lang=xx";
    assert_eq!("ja", codec::decode(&line).unwrap().lang);
}

#[test]
fn non_japanese_intonation_splits_on_latin_and_arabic_punctuation() {
    let mut e = plain(AlarmKind::Speech);
    e.lang = "ar".into();
    e.text = "دواء".into();
    let occ = Planner::next(std::slice::from_ref(&e), &[], &CalendarSettings::default(), dt(2026, 9, 30, 0, 0), &NoHolidays).remove(0);
    let out = SpeechText::intonate(occ.segments.clone());
    assert!(out.len() >= occ.segments.len());
    let mut e2 = plain(AlarmKind::Speech);
    e2.lang = "en".into();
    e2.text = "take the medicine".into();
    let occ2 = Planner::next(std::slice::from_ref(&e2), &[], &CalendarSettings::default(), dt(2026, 9, 30, 0, 0), &NoHolidays).remove(0);
    let out2 = SpeechText::intonate(occ2.segments);
    assert!(out2.len() >= 2, "英語の「Master, it's time. …」も文節に割れる: {out2:?}");
}
