mod common;
use chrono::{Datelike, Weekday};
use common::{date, dt, time};
use maid_cafe_core::{DaySet, JapaneseHolidays, NoHolidays, Recurrence, Schedule};

fn days(d: &[Weekday]) -> DaySet {
    DaySet::new(d)
}

#[test]
fn daily() {
    assert!(Recurrence::Daily.matches(date(2026, 1, 1), &NoHolidays));
}

#[test]
fn weekdays_skip_holidays() {
    let r = Recurrence::weekdays();
    assert!(!r.matches(date(2026, 5, 6), &JapaneseHolidays)); // 振替休日(水)
    assert!(r.matches(date(2026, 5, 7), &JapaneseHolidays));
    assert!(!r.matches(date(2026, 5, 9), &JapaneseHolidays)); // 土
    assert!(Recurrence::Weekdays { skip_holidays: false }.matches(date(2026, 5, 6), &JapaneseHolidays));
}

#[test]
fn weekends_and_holidays_is_complement_of_weekdays() {
    let mut d = date(2026, 1, 1);
    while d.year() == 2026 {
        assert_eq!(
            !Recurrence::weekdays().matches(d, &JapaneseHolidays),
            Recurrence::WeekendsAndHolidays.matches(d, &JapaneseHolidays),
            "{d}"
        );
        d += chrono::Duration::days(1);
    }
}

#[test]
fn days_of_week() {
    let r = Recurrence::days_of_week(days(&[Weekday::Mon, Weekday::Wed])).unwrap();
    assert!(r.matches(date(2026, 9, 28), &NoHolidays)); // 月
    assert!(!r.matches(date(2026, 9, 29), &NoHolidays)); // 火
    assert!(Recurrence::days_of_week(DaySet::default()).is_err());
}

#[test]
fn nth_weekday() {
    let second = Recurrence::nth_weekday_of_month(2, Weekday::Tue).unwrap();
    assert!(second.matches(date(2026, 9, 8), &NoHolidays)); // 9/1,8 が火 → 第2
    assert!(!second.matches(date(2026, 9, 1), &NoHolidays));
    assert!(!second.matches(date(2026, 9, 15), &NoHolidays));
    // 2026-09 は火曜が 1,8,15,22,29 → 第5・最終は29
    assert!(Recurrence::nth_weekday_of_month(5, Weekday::Tue).unwrap().matches(date(2026, 9, 29), &NoHolidays));
    let last = Recurrence::nth_weekday_of_month(-1, Weekday::Tue).unwrap();
    assert!(last.matches(date(2026, 9, 29), &NoHolidays));
    assert!(!last.matches(date(2026, 9, 22), &NoHolidays));
    // 2026-10 は火曜が 6,13,20,27 → 第5は存在せず、次に第5火曜がある月(12/29)まで飛ぶ
    let s = Schedule::new(Recurrence::nth_weekday_of_month(5, Weekday::Tue).unwrap(), time(7, 0));
    assert_eq!(Some(dt(2026, 12, 29, 7, 0)), s.next_trigger(dt(2026, 9, 30, 0, 0), &NoHolidays));
    assert!(Recurrence::nth_weekday_of_month(6, Weekday::Tue).is_err());
}

#[test]
fn every_n_weeks() {
    let anchor = date(2026, 9, 7); // 月曜
    let r = Recurrence::every_n_weeks(2, days(&[Weekday::Mon, Weekday::Fri]), anchor).unwrap();
    assert!(r.matches(date(2026, 9, 7), &NoHolidays));
    assert!(r.matches(date(2026, 9, 11), &NoHolidays)); // 同じ週の金
    assert!(!r.matches(date(2026, 9, 14), &NoHolidays)); // 翌週
    assert!(r.matches(date(2026, 9, 21), &NoHolidays)); // 2週後
    assert!(!r.matches(date(2026, 8, 24), &NoHolidays)); // 基準より前は鳴らさない
    // 基準日が週の途中(水)でも、その週の月曜からを1週目とする
    let mid = Recurrence::every_n_weeks(2, days(&[Weekday::Mon]), date(2026, 9, 9)).unwrap();
    assert!(mid.matches(date(2026, 9, 7), &NoHolidays));
    assert!(mid.matches(date(2026, 9, 21), &NoHolidays));
    assert!(Recurrence::every_n_weeks(0, days(&[Weekday::Mon]), anchor).is_err());
}

#[test]
fn next_trigger_basic_and_strictly_after() {
    let s = Schedule::new(Recurrence::Daily, time(7, 0));
    assert_eq!(Some(dt(2026, 9, 30, 7, 0)), s.next_trigger(dt(2026, 9, 30, 6, 59), &NoHolidays));
    assert_eq!(Some(dt(2026, 10, 1, 7, 0)), s.next_trigger(dt(2026, 9, 30, 7, 0), &NoHolidays)); // ちょうどは含めない
}

#[test]
fn next_trigger_weekdays_skips_holiday_weekend() {
    let s = Schedule::new(Recurrence::weekdays(), time(7, 0));
    // 2026-05-02(土)の後: 5/3日 5/4月 5/5火 5/6水 は休み → 5/7木
    assert_eq!(Some(dt(2026, 5, 7, 7, 0)), s.next_trigger(dt(2026, 5, 2, 12, 0), &JapaneseHolidays));
}

#[test]
fn next_trigger_start_end_date() {
    let s = Schedule::new(Recurrence::Daily, time(7, 0)).with_period(Some(date(2026, 10, 5)), Some(date(2026, 10, 6)));
    assert_eq!(Some(dt(2026, 10, 5, 7, 0)), s.next_trigger(dt(2026, 9, 30, 0, 0), &NoHolidays));
    assert_eq!(Some(dt(2026, 10, 6, 7, 0)), s.next_trigger(dt(2026, 10, 5, 8, 0), &NoHolidays));
    assert_eq!(None, s.next_trigger(dt(2026, 10, 6, 8, 0), &NoHolidays));
}

#[test]
fn pre_notice() {
    let s = Schedule::new(Recurrence::Daily, time(9, 0)).with_pre_notice(30).unwrap();
    assert_eq!(Some(dt(2026, 9, 30, 8, 30)), s.next_pre_notice(dt(2026, 9, 30, 8, 0), &NoHolidays));
    // 予告時刻を過ぎて本番前 → 予告は翌日分
    assert_eq!(Some(dt(2026, 10, 1, 8, 30)), s.next_pre_notice(dt(2026, 9, 30, 8, 45), &NoHolidays));
    assert_eq!(None, Schedule::new(Recurrence::Daily, time(7, 0)).next_pre_notice(dt(2026, 9, 30, 0, 0), &NoHolidays));
    // 0:10発火・30分前は前日23:40
    let early = Schedule::new(Recurrence::Daily, time(0, 10)).with_pre_notice(30).unwrap();
    assert_eq!(Some(dt(2026, 9, 30, 23, 40)), early.next_pre_notice(dt(2026, 9, 30, 12, 0), &NoHolidays));
    assert!(Schedule::new(Recurrence::Daily, time(7, 0)).with_pre_notice(0).is_err());
}

#[test]
fn weekend() {
    let r = Recurrence::days_of_week(days(&[Weekday::Sat, Weekday::Sun])).unwrap();
    assert!(r.matches(date(2026, 10, 3), &NoHolidays));
}
