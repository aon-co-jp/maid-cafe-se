//! 次に鳴らすアラームを監視するタスク。
//!
//! 30秒ごと(またはアラームが編集されたとき)に[`Planner`]で再計算し、時刻になったら鳴らす。
//! スリープ復帰などで90秒以上遅れた分は、鳴らさずに飛ばす(何時間も前のアラームが一斉に鳴るのを避ける)。

use crate::state::AppState;
use chrono::{Duration as CDuration, Local, NaiveDateTime};
use maid_cafe_core::{CalendarSettings, JapaneseHolidays, Occurrence, Planner};
use std::sync::Arc;
use std::time::Duration;

/// 何秒遅れたところまでなら、鳴らすか。
pub const MAX_LATE_SECS: i64 = 90;

/// `cursor`より後の、次の発火(同時刻が複数あれば複数)と、それが「いま鳴らすべきか/待つか/古すぎて飛ばすか」。
#[derive(Debug, PartialEq)]
pub enum Step {
    /// 予定なし
    Idle,
    /// まだ先: この時間だけ待つ
    Wait(Duration),
    /// 時刻になった(遅れが許容内): 鳴らす
    Fire(Vec<Occurrence>),
    /// 古すぎる(スリープ復帰など): 鳴らさず、カーソルだけ進める
    Skip(NaiveDateTime),
}

/// 1回分の判断(純粋関数、テスト用に切り出している)。最大の待ち時間は30秒。
pub fn step(entries: &[maid_cafe_core::AlarmEntry], cursor: NaiveDateTime, now: NaiveDateTime) -> Step {
    let next = Planner::next(entries, &[], &CalendarSettings::default(), cursor, &JapaneseHolidays);
    let Some(first) = next.first() else { return Step::Idle };
    let at = first.time;
    if at > now {
        let ms = (at - now).num_milliseconds().clamp(50, 30_000);
        return Step::Wait(Duration::from_millis(ms as u64));
    }
    if now - at <= CDuration::seconds(MAX_LATE_SECS) {
        Step::Fire(next)
    } else {
        Step::Skip(at)
    }
}

/// スケジューラ本体(アプリの間、ずっと動く)。
pub async fn run(state: Arc<AppState>) {
    let mut cursor = Local::now().naive_local();
    loop {
        let now = Local::now().naive_local();
        match step(&state.entries(), cursor, now) {
            Step::Idle => {
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(30)) => {}
                    _ = state.changed.notified() => {}
                }
            }
            Step::Wait(d) => {
                tokio::select! {
                    _ = tokio::time::sleep(d) => {}
                    _ = state.changed.notified() => {}
                }
            }
            Step::Fire(list) => {
                cursor = list[0].time;
                state.store.log(&format!("fire: {} at {}", list.iter().map(|o| o.title.as_str()).collect::<Vec<_>>().join(" / "), cursor));
                let st = state.clone();
                // 鳴らしている間もスケジューラは止めない(次の予定の監視を続ける)
                tokio::task::spawn_blocking(move || {
                    let s = st.clone();
                    st.player.play(&list, &move |style| s.voice_pref(style));
                });
            }
            Step::Skip(at) => {
                state.store.log(&format!("skipped late alarm at {at}"));
                cursor = at;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveTime;
    use maid_cafe_core::{AlarmEntry, AlarmKind, Recurrence, Schedule};

    fn dt(h: u32, m: u32, s: u32) -> NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(2026, 9, 30).unwrap().and_hms_opt(h, m, s).unwrap()
    }

    fn daily(id: &str, h: u32, m: u32) -> AlarmEntry {
        AlarmEntry::new(id, id, Schedule::new(Recurrence::Daily, NaiveTime::from_hms_opt(h, m, 0).unwrap()), AlarmKind::Sound)
    }

    #[test]
    fn idle_without_alarms() {
        assert_eq!(Step::Idle, step(&[], dt(7, 0, 0), dt(7, 0, 0)));
    }

    #[test]
    fn waits_until_the_alarm_but_at_most_30_seconds() {
        assert_eq!(Step::Wait(Duration::from_secs(10)), step(&[daily("a", 7, 0)], dt(6, 0, 0), dt(6, 59, 50)));
        assert_eq!(Step::Wait(Duration::from_secs(30)), step(&[daily("a", 7, 0)], dt(6, 0, 0), dt(6, 0, 0)));
    }

    #[test]
    fn fires_when_due_and_only_within_the_lateness_window() {
        let e = [daily("a", 7, 0)];
        match step(&e, dt(6, 59, 0), dt(7, 0, 0)) {
            Step::Fire(list) => assert_eq!("alarm:a", list[0].key),
            other => panic!("{other:?}"),
        }
        assert!(matches!(step(&e, dt(6, 59, 0), dt(7, 1, 30)), Step::Fire(_))); // ちょうど90秒遅れまで
        assert_eq!(Step::Skip(dt(7, 0, 0)), step(&e, dt(6, 59, 0), dt(7, 1, 31))); // 91秒遅れは飛ばす
    }

    #[test]
    fn firing_advances_the_cursor_so_that_the_same_alarm_is_not_repeated() {
        let e = [daily("a", 7, 0)];
        // カーソルを鳴らした時刻へ進めると、次は翌日の同時刻を待つ
        match step(&e, dt(7, 0, 0), dt(7, 0, 1)) {
            Step::Wait(d) => assert_eq!(Duration::from_secs(30), d),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn simultaneous_alarms_fire_together() {
        let e = [daily("a", 7, 0), daily("b", 7, 0)];
        match step(&e, dt(6, 0, 0), dt(7, 0, 0)) {
            Step::Fire(list) => assert_eq!(2, list.len()),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn disabled_alarms_are_ignored() {
        let mut a = daily("a", 7, 0);
        a.enabled = false;
        assert_eq!(Step::Idle, step(&[a], dt(6, 0, 0), dt(7, 0, 0)));
    }
}
