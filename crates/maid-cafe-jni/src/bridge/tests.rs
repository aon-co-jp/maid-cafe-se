//! ブリッジの検証。Kotlin版が保存した形式の入力を渡し、coreと同じ結果が返ることを確かめる。

use super::*;
use maid_cafe_core::audio::wav_bytes;

fn secs(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> i64 {
    chrono::NaiveDate::from_ymd_opt(y, mo, d).unwrap().and_hms_opt(h, mi, 0).unwrap().and_utc().timestamp()
}

/// Kotlin版`Codec.encode`が出す形式の、毎日7:00の読み上げアラーム(メイド風、セリフ2つ、ハモり、30分前予告)。
const ENTRY: &str = "id=a1&label=%E8%B5%B7%E5%BA%8A&rec=daily&time=07%3A00&start=&end=&pre=30&kind=SPEECH&sound=chime&text=%E8%96%AC&voice=MAID&enabled=1&ph=okite%2Cfight&pph=fight&harm=1";

#[test]
fn plan_next_returns_the_pre_notice_first_and_encodes_everything() {
    let out = plan_next(ENTRY, "", "", secs(2026, 9, 30, 6, 0));
    let line = out.lines().next().unwrap();
    let m = fields(line);
    // 予告(30分前)が先
    assert_eq!(secs(2026, 9, 30, 6, 30).to_string(), m["t"]);
    assert_eq!("pre:a1", m["key"], "{line}");
    assert_eq!("MAID", m["voice"]);
    assert_eq!("1", m["harm"]);
    assert_eq!("", m["sound"]);
    assert!(m["speech"].contains("30分"));
    // 区切りの形: `<文章>,<音程>,<話速>,<間>` が`;`区切り。最後の間は0
    let segs: Vec<&str> = m["seg"].split(';').collect();
    assert_eq!(2, segs.len(), "基本メッセージ+セリフ1つ(fight)");
    assert!(segs[1].ends_with(",0"), "{:?}", segs);
    assert!(url_decode(segs[1].split(',').next().unwrap()).unwrap().contains("ファイト"));

    // 予告の後は本番
    let after_pre = plan_next(ENTRY, "", "", secs(2026, 9, 30, 6, 30));
    let m = fields(after_pre.lines().next().unwrap());
    assert_eq!(secs(2026, 9, 30, 7, 0).to_string(), m["t"]);
    assert_eq!("alarm:a1", m["key"]);
    assert!(url_decode(m["seg"].split(';').next().unwrap().split(',').next().unwrap()).unwrap().contains("お時間ですよ"));
}

#[test]
fn plan_next_handles_calendar_events_and_settings() {
    let events = format!("id=e1&title={}&start={}\nbroken line\nid=e2&title=x&start=notanumber", url_encode("会議"), secs(2026, 9, 30, 10, 0));
    assert_eq!(1, decode_events(&events).len());
    let settings = "enabled=1&pre=1&premin=15&voice=DEEP_MALE&ph=fight&pph=&harm=0";
    let out = plan_next("", &events, settings, secs(2026, 9, 30, 9, 0));
    let m = fields(out.lines().next().unwrap());
    assert_eq!(secs(2026, 9, 30, 9, 45).to_string(), m["t"], "15分前の予告");
    assert_eq!("DEEP_MALE", m["voice"]);
    assert!(m["speech"].contains("会議") && m["speech"].contains("15分"));
}

#[test]
fn settings_fall_back_to_defaults_for_unreadable_values() {
    let d = CalendarSettings::default();
    assert_eq!(d, decode_settings(""));
    let s = decode_settings("premin=0&voice=ROBOT&ph=bogus%2Cfight");
    assert_eq!(30, s.pre_notice_minutes);
    assert_eq!(VoiceStyle::Maid, s.voice);
    assert_eq!(vec!["fight".to_string()], s.phrases);
}

#[test]
fn plan_next_is_empty_for_no_alarms_bad_time_or_broken_entries() {
    assert_eq!("", plan_next("", "", "", secs(2026, 9, 30, 6, 0)));
    assert_eq!("", plan_next("こわれた行", "", "", secs(2026, 9, 30, 6, 0)));
    assert_eq!("", plan_next(ENTRY, "", "", i64::MAX));
}

#[test]
fn alarm_speech_matches_core() {
    assert_eq!(
        SpeechText::alarm_speech(AlarmKind::Speech, "薬", "起床", VoiceStyle::Maid, &["fight".to_string()]),
        alarm_speech("SPEECH", "薬", "起床", "MAID", "fight")
    );
    assert_eq!(None, alarm_speech("SOUND", "薬", "起床", "MAID", ""));
    assert_eq!(None, alarm_speech("NOISE", "薬", "起床", "MAID", ""));
}

#[test]
fn phrase_numbers_follow_the_core_rules_and_round_trip() {
    let n = phrase_edit("", "add", "okite", 0).unwrap();
    let n = phrase_edit(&n, "add", "okaeri", 0).unwrap();
    assert_eq!("okite:1,okaeri:2", n);
    // 後から1を入れたほうが取り、元の持ち主は空いている最小番号へ
    let n = phrase_edit(&n, "assign", "okaeri", 1).unwrap();
    assert_eq!("okite:2,okaeri:1", n);
    assert_eq!("okaeri,okite", phrase_order(&n));
    assert_eq!("okite:1,okaeri:2", phrase_ranks("okite,okaeri"));
    assert_eq!("okite:2", phrase_edit(&n, "remove", "okaeri", 0).unwrap());
    assert!(phrase_edit(&n, "add", "bogus", 0).is_err());
    assert!(phrase_edit(&n, "shuffle", "okite", 0).is_err());
    assert_eq!("", phrase_order(""));
}

fn tone(sr: u32, secs: f32) -> Vec<f32> {
    (0..(sr as f32 * secs) as usize).map(|i| 0.3 * (2.0 * std::f32::consts::PI * 220.0 * i as f32 / sr as f32).sin()).collect()
}

#[test]
fn render_segment_processes_wav_bytes_and_rejects_bad_input() {
    let wav = wav_bytes(&Pcm::new(tone(22_050, 0.6), 22_050));
    assert_eq!(22_050, wav_sample_rate(&wav));
    assert_eq!(0, wav_sample_rate(b"not a wav"));
    let out = render_segment(&wav, "MAID", "FEMALE", false, 1.0).expect("音声が返る");
    assert!(out.len() > 22_050 / 4 && out.iter().all(|x| x.is_finite() && x.abs() <= 1.0));
    let harmony = render_segment(&wav, "MAID", "FEMALE", true, 1.0).unwrap();
    assert_ne!(out, harmony);
    assert!(render_segment(&wav, "ROBOT", "FEMALE", false, 1.0).is_none());
    assert!(render_segment(b"junk", "MAID", "FEMALE", false, 1.0).is_none());
    // 0.05秒未満は空扱い
    let tiny = wav_bytes(&Pcm::new(tone(22_050, 0.02), 22_050));
    assert!(render_segment(&tiny, "MAID", "FEMALE", false, 1.0).is_none());
}

#[test]
fn join_inserts_gaps_and_dithers_to_pcm16() {
    let a = tone(8_000, 0.1);
    let pcm = join_pcm16(&[a.clone(), a.clone()], &[500, 0], 8_000);
    let gap = 8_000 / 2;
    assert!(pcm.len() >= 2 * a.len() + gap - 4 && pcm.len() <= 2 * a.len() + gap + 4, "{}", pcm.len());
    // 間の中ほどは(ディザの量子化ノイズを除き)ほぼ無音
    let mid = &pcm[a.len() + gap / 2 - 50..a.len() + gap / 2 + 50];
    assert!(mid.iter().all(|s| s.abs() <= 2), "{mid:?}");
    assert!(pcm.iter().any(|s| s.abs() > 3000), "音そのものは残る");
}
