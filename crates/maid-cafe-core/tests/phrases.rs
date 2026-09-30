mod common;
use common::{dt, s, time};
use maid_cafe_core::audio::wav::JavaRandom;
use maid_cafe_core::codec;
use maid_cafe_core::*;

fn entry(text: &str, phrases: &[&str], pre_phrases: &[&str], harmony: bool, pre: Option<u32>, kind: AlarmKind, voice: VoiceStyle) -> AlarmEntry {
    let mut sch = Schedule::new(Recurrence::Daily, time(7, 0));
    if let Some(p) = pre {
        sch = sch.with_pre_notice(p).unwrap();
    }
    let mut e = AlarmEntry::new("a", "起床", sch, kind);
    e.text = text.into();
    e.voice = voice;
    e.phrases = s(phrases);
    e.pre_phrases = s(pre_phrases);
    e.harmony = harmony;
    e
}

fn simple(text: &str, phrases: &[&str]) -> AlarmEntry {
    entry(text, phrases, &[], false, None, AlarmKind::Speech, VoiceStyle::Maid)
}

fn first(e: &AlarmEntry, after: chrono::NaiveDateTime) -> Occurrence {
    Planner::next(&[e.clone()], &[], &CalendarSettings::default(), after, &NoHolidays).remove(0)
}

fn t0() -> chrono::NaiveDateTime {
    dt(2026, 9, 30, 0, 0)
}

#[test]
fn catalog_has_the_requested_phrases() {
    let display: Vec<&str> = PHRASES.iter().map(|p| p.display).collect();
    assert!(display.contains(&"おかえりなさいませご主人様！"));
    assert!(display.contains(&"おいしくな～れ萌え萌えキュ～ン"));
    assert!(display.iter().any(|d| d.starts_with("メッ！ダメなんだぞこら！") && d.ends_with("頑張って行きましょう！")));
    assert!(display.contains(&"ファイト！ファイト！"));
    assert!(display.contains(&"エクセレント！"));
    assert!(display.contains(&"パーフェクト！"));
    assert!(display.contains(&"ご主人さま～、お～き～て～。今日も頑張って～"));
    let ids: std::collections::HashSet<&str> = PHRASES.iter().map(|p| p.id).collect();
    assert_eq!(7, ids.len());
}

#[test]
fn single_phrase_appended_after_message() {
    let sp = first(&simple("薬を飲む", &["okaeri"]), t0()).speech.unwrap();
    assert!(sp.contains("薬を飲む"));
    assert!(sp.ends_with("おかえりなさいませ、ご主人様！"));
}

#[test]
fn multiple_phrases_combined_in_selection_order() {
    // カタログ順ではなく、選んだ順(perfect→okaeri→fight)で連結される
    let sp = first(&simple("薬を飲む", &["perfect", "okaeri", "fight"]), t0()).speech.unwrap();
    let (i1, i2, i3) = (sp.find("パーフェクト！").unwrap(), sp.find("おかえりなさいませ").unwrap(), sp.find("ファイト！ファイト！").unwrap());
    assert!(i1 < i2 && i2 < i3, "{sp}");
}

#[test]
fn okaeri_then_wake_up_and_wake_up_then_okaeri_both_work() {
    let a = first(&simple("", &["okaeri", "okite"]), t0()).speech.unwrap();
    assert!(a.starts_with("おかえりなさいませ、ご主人様！"), "{a}");
    assert!(a.find("おーきーてー").unwrap() > a.find("おかえりなさいませ").unwrap());
    let b = first(&simple("", &["okite", "okaeri"]), t0()).speech.unwrap();
    assert!(b.starts_with("ご主人さまー、おーきーてー。今日も、がんばってー"), "{b}");
    assert!(b.find("おかえりなさいませ").unwrap() > b.find("おーきーてー").unwrap());
}

#[test]
fn wake_up_phrase_alone_is_slow() {
    let seg = first(&simple("", &["okite"]), t0()).segments;
    assert_eq!(1, seg.len());
    assert_eq!(0.8f32, seg[0].rate); // ゆっくり間延びさせて読む
}

fn nums(p: &[(&str, i32)]) -> Numbers {
    p.iter().map(|(a, b)| (a.to_string(), *b)).collect()
}

fn sorted(n: &Numbers) -> Vec<(String, i32)> {
    let mut v = n.clone();
    v.sort();
    v
}

#[test]
fn ranks_and_ordered() {
    assert_eq!(nums(&[("okite", 1), ("okaeri", 2), ("fight", 3)]), MaidPhrases::ranks(&s(&["okite", "okaeri", "fight"])));
    assert_eq!(
        s(&["fight", "okite", "okaeri"]),
        MaidPhrases::ordered(&nums(&[("okite", 2), ("okaeri", 5), ("fight", 1)]))
    );
}

#[test]
fn add_goes_to_the_end_and_remove_leaves_a_gap() {
    let mut n: Numbers = Vec::new();
    n = MaidPhrases::add(&n, "okaeri");
    assert_eq!(nums(&[("okaeri", 1)]), n);
    n = MaidPhrases::add(&n, "okite");
    n = MaidPhrases::add(&n, "fight");
    assert_eq!(nums(&[("okaeri", 1), ("okite", 2), ("fight", 3)]), n);
    n = MaidPhrases::remove(&n, "okite");
    assert_eq!(nums(&[("okaeri", 1), ("fight", 3)]), n); // 欠番を詰めない
    n = MaidPhrases::add(&n, "perfect");
    assert_eq!(Some(4), n.iter().find(|(i, _)| i == "perfect").map(|(_, v)| *v)); // 最大番号の次
    assert_eq!(n, MaidPhrases::add(&n, "perfect")); // 二重に加えない
    assert_eq!(n, MaidPhrases::remove(&n, "excellent"));
}

#[test]
fn duplicate_number_displaced_holder_gets_smallest_free_number() {
    // 2件: 2番目に1を入れる → 元の1番(おかえり)は空いている2番になる
    let two = MaidPhrases::assign(&nums(&[("okaeri", 1), ("okite", 2)]), "okite", 1);
    assert_eq!(sorted(&nums(&[("okite", 1), ("okaeri", 2)])), sorted(&two));
    assert_eq!(s(&["okite", "okaeri"]), MaidPhrases::ordered(&two));
    // 3件: 3番目に1を入れる → 元の1番は、空いた3番へ
    let three = MaidPhrases::assign(&nums(&[("okaeri", 1), ("okite", 2), ("fight", 3)]), "fight", 1);
    assert_eq!(sorted(&nums(&[("fight", 1), ("okite", 2), ("okaeri", 3)])), sorted(&three));
    // 3件: 3番目に2を入れる → 元の2番は、空いた3番へ(1は使われている)
    let mid = MaidPhrases::assign(&nums(&[("okaeri", 1), ("okite", 2), ("fight", 3)]), "fight", 2);
    assert_eq!(sorted(&nums(&[("okaeri", 1), ("okite", 3), ("fight", 2)])), sorted(&mid));
    // 空いている最小の番号(欠番の1)へ回る
    let gap = MaidPhrases::assign(&nums(&[("okite", 2), ("fight", 3)]), "fight", 2);
    assert_eq!(sorted(&nums(&[("okite", 1), ("fight", 2)])), sorted(&gap));
}

#[test]
fn assigning_a_free_number_just_moves() {
    assert_eq!(
        sorted(&nums(&[("okaeri", 1), ("okite", 5)])),
        sorted(&MaidPhrases::assign(&nums(&[("okaeri", 1), ("okite", 2)]), "okite", 5))
    );
    // 自分の番号と同じ・0や負・未選択idは変化なし
    let base = nums(&[("okaeri", 1), ("okite", 2)]);
    assert_eq!(base, MaidPhrases::assign(&base, "okite", 2));
    assert_eq!(base, MaidPhrases::assign(&base, "okite", 0));
    assert_eq!(base, MaidPhrases::assign(&base, "perfect", 1));
}

#[test]
fn never_produces_duplicate_numbers() {
    let ids = ["okite", "okaeri", "oishiku", "meh", "fight", "excellent", "perfect"];
    let mut n: Numbers = Vec::new();
    for id in ids {
        n = MaidPhrases::add(&n, id);
    }
    let mut rnd = JavaRandom::new(7);
    for _ in 0..500 {
        let id = ids[(rnd.next_float() * ids.len() as f32) as usize % ids.len()];
        let v = 1 + (rnd.next_float() * 9.0) as i32;
        n = MaidPhrases::assign(&n, id, v);
        let uniq: std::collections::HashSet<i32> = n.iter().map(|(_, v)| *v).collect();
        assert_eq!(n.len(), uniq.len(), "重複あり: {n:?}");
        assert_eq!(ids.len(), MaidPhrases::ordered(&n).len());
    }
}

#[test]
fn edited_numbers_decide_the_spoken_order() {
    let n = MaidPhrases::assign(&nums(&[("okaeri", 1), ("okite", 2)]), "okite", 1);
    let phrases = MaidPhrases::ordered(&n);
    let e = simple("", &phrases.iter().map(|x| x.as_str()).collect::<Vec<_>>());
    let sp = first(&e, t0()).speech.unwrap();
    assert!(sp.starts_with("ご主人さまー、おーきーてー"), "{sp}");
    assert!(sp.find("おかえりなさいませ").unwrap() > sp.find("おーきーてー").unwrap());
}

#[test]
fn phrases_only_when_text_blank() {
    assert_eq!(Some("エクセレント！".to_string()), first(&simple("", &["excellent"]), t0()).speech);
}

#[test]
fn blank_text_and_no_phrases_falls_back_to_label() {
    assert!(first(&simple("", &[]), t0()).speech.unwrap().contains("起床"));
}

#[test]
fn sound_kind_with_phrases_plays_both() {
    let e = entry("薬を飲む", &["meh"], &[], false, None, AlarmKind::Sound, VoiceStyle::Maid);
    let o = first(&e, t0());
    assert_eq!(Some("chime".to_string()), o.sound_id);
    assert!(o.speech.unwrap().starts_with("めっ！だめなんだぞ"));
}

#[test]
fn sound_kind_without_phrases_has_no_speech() {
    let e = entry("", &[], &[], false, None, AlarmKind::Sound, VoiceStyle::Maid);
    assert_eq!(None, first(&e, t0()).speech);
}

#[test]
fn pre_notice_uses_pre_phrases_not_main_phrases() {
    let e = entry("薬を飲む", &["okaeri"], &["fight"], false, Some(30), AlarmKind::Speech, VoiceStyle::Maid);
    let pre = first(&e, t0());
    assert!(pre.key.starts_with("pre:"));
    let sp = pre.speech.unwrap();
    assert!(sp.contains("30分") && sp.ends_with("ファイト！ファイト！") && !sp.contains("おかえり"));
    let main = first(&e, dt(2026, 9, 30, 6, 45));
    assert!(main.key.starts_with("alarm:"));
    let sp = main.speech.unwrap();
    assert!(sp.ends_with("おかえりなさいませ、ご主人様！") && !sp.contains("ファイト"));
}

#[test]
fn harmony_flag_propagates() {
    assert!(first(&entry("", &["okaeri"], &[], true, None, AlarmKind::Speech, VoiceStyle::Maid), t0()).harmony);
    assert!(!first(&simple("", &["okaeri"]), t0()).harmony);
}

#[test]
fn calendar_phrases_and_harmony() {
    let ev = CalendarEvent { id: "e".into(), title: "会議".into(), start: dt(2026, 9, 30, 10, 0) };
    let st = CalendarSettings {
        enabled: true,
        phrases: s(&["perfect"]),
        pre_phrases: s(&["excellent"]),
        harmony: true,
        ..Default::default()
    };
    let pre = Planner::next(&[], &[ev.clone()], &st, dt(2026, 9, 30, 8, 0), &NoHolidays).remove(0);
    assert_eq!(dt(2026, 9, 30, 9, 30), pre.time);
    assert!(pre.speech.unwrap().ends_with("エクセレント！") && pre.harmony);
    let main = Planner::next(&[], &[ev], &st, dt(2026, 9, 30, 9, 30), &NoHolidays).remove(0);
    let sp = main.speech.unwrap();
    assert!(sp.contains("会議") && sp.ends_with("パーフェクト！"));
}

#[test]
fn selection_order_survives_codec() {
    let e = entry("", &["perfect", "okite", "okaeri"], &["fight", "excellent"], false, None, AlarmKind::Speech, VoiceStyle::Maid);
    let back = codec::decode_all(&codec::encode_all(&[e])).remove(0);
    assert_eq!(s(&["perfect", "okite", "okaeri"]), back.phrases);
    assert_eq!(s(&["fight", "excellent"]), back.pre_phrases);
}

#[test]
fn codec_round_trip_and_backward_compat() {
    let e = entry("", &["okaeri", "fight"], &["oishiku"], true, Some(30), AlarmKind::Speech, VoiceStyle::Maid);
    assert_eq!(vec![e.clone()], codec::decode_all(&codec::encode_all(&[e.clone()])));
    // 旧バージョンの保存行(ph/pph/harm無し)も読める
    let legacy: String = codec::encode(&e).split('&').filter(|p| !p.starts_with("ph=") && !p.starts_with("pph=") && !p.starts_with("harm=")).collect::<Vec<_>>().join("&");
    let d = codec::decode(&legacy).unwrap();
    assert!(d.phrases.is_empty() && d.pre_phrases.is_empty() && !d.harmony);
    // 未知のidは捨てる
    let bad = codec::encode(&e).replace("ph=okaeri%2Cfight", "ph=okaeri%2Cbogus");
    assert_eq!(s(&["okaeri"]), codec::decode(&bad).unwrap().phrases);
}

// ---- セリフごとの間・抑揚(PolishTest由来) ----

#[test]
fn segments_carry_per_phrase_prosody_and_gaps() {
    let segs = SpeechText::segments(Some("ご主人様、お時間です"), &s(&["okaeri", "fight"]));
    assert_eq!(3, segs.len()); // 本文 → おかえり → ファイト(選んだ順)
    assert_eq!("ご主人様、お時間です", segs[0].text);
    assert_eq!(300, segs[0].gap_after_ms);
    let okaeri = MaidPhrases::by_id("okaeri").unwrap();
    assert_eq!(okaeri.spoken, segs[1].text);
    assert_eq!(okaeri.gap_ms, segs[1].gap_after_ms);
    assert_eq!(okaeri.rate, segs[1].rate);
    let fight = MaidPhrases::by_id("fight").unwrap();
    assert_eq!(fight.pitch, segs[2].pitch);
    assert_eq!(0, segs[2].gap_after_ms); // 最後の後ろには間を置かない
}

#[test]
fn segments_without_phrases_or_base() {
    assert_eq!(1, SpeechText::segments(Some("本文のみ"), &[]).len());
    assert_eq!(0, SpeechText::segments(Some("本文のみ"), &[])[0].gap_after_ms);
    assert!(SpeechText::segments(None, &[]).is_empty());
    assert_eq!(1, SpeechText::segments(None, &s(&["perfect"])).len());
}

#[test]
fn occurrence_segments_join_to_speech() {
    let e = simple("薬", &["okaeri", "perfect"]);
    let o = first(&e, t0());
    assert_eq!(o.speech.clone().unwrap(), o.segments.iter().map(|x| x.text.as_str()).collect::<Vec<_>>().join(" "));
    assert_eq!(3, o.segments.len());
}

#[test]
fn sound_only_occurrence_has_no_segments() {
    assert!(first(&entry("", &[], &[], false, None, AlarmKind::Sound, VoiceStyle::Maid), t0()).segments.is_empty());
}
