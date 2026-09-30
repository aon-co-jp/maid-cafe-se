mod common;
use chrono::NaiveDate;
use common::date;
use maid_cafe_core::{HolidayCalendar, JapaneseHolidays};
use std::collections::HashSet;

fn set(year: i32, md: &[(u32, u32)]) -> HashSet<NaiveDate> {
    md.iter().map(|(m, d)| date(year, *m, *d)).collect()
}

#[test]
fn holidays_2024() {
    let expected = set(
        2024,
        &[
            (1, 1), (1, 8), (2, 11), (2, 12), (2, 23), (3, 20), (4, 29), (5, 3), (5, 4), (5, 5), (5, 6),
            (7, 15), (8, 11), (8, 12), (9, 16), (9, 22), (9, 23), (10, 14), (11, 3), (11, 4), (11, 23),
        ],
    );
    assert_eq!(expected, JapaneseHolidays::holidays_of(2024).unwrap());
}

#[test]
fn holidays_2026_citizens_holiday_and_substitute() {
    let h = JapaneseHolidays;
    assert!(h.is_holiday(date(2026, 5, 6))); // 5/3(日)の振替
    assert!(h.is_holiday(date(2026, 9, 22))); // 国民の休日
    assert!(h.is_holiday(date(2026, 9, 23))); // 秋分の日
    assert!(h.is_holiday(date(2026, 3, 20))); // 春分の日
    assert!(!h.is_holiday(date(2026, 5, 7)));
}

#[test]
fn holidays_2019_enthronement() {
    let h = JapaneseHolidays::holidays_of(2019).unwrap();
    for d in 29..=30 {
        assert!(h.contains(&date(2019, 4, d)));
    }
    for d in 1..=6 {
        assert!(h.contains(&date(2019, 5, d)));
    }
    assert!(h.contains(&date(2019, 10, 22)));
    assert!(!h.contains(&date(2019, 12, 23)));
    assert!(!h.contains(&date(2019, 2, 23)));
}

#[test]
fn olympic_years() {
    let h20 = JapaneseHolidays::holidays_of(2020).unwrap();
    assert!(h20.contains(&date(2020, 7, 23)));
    assert!(h20.contains(&date(2020, 7, 24)));
    assert!(h20.contains(&date(2020, 8, 10)));
    assert!(!h20.contains(&date(2020, 10, 12)));
    assert!(JapaneseHolidays::holidays_of(2021).unwrap().contains(&date(2021, 8, 9))); // 8/8(日)の振替
}

#[test]
fn heisei_emperor_birthday() {
    let h18 = JapaneseHolidays::holidays_of(2018).unwrap();
    assert!(h18.contains(&date(2018, 12, 23)));
    assert!(h18.contains(&date(2018, 12, 24))); // 日曜の振替
    assert!(JapaneseHolidays::holidays_of(2010).unwrap().contains(&date(2010, 12, 23)));
}

#[test]
fn out_of_range() {
    assert!(JapaneseHolidays::holidays_of(2006).is_err());
    assert!(JapaneseHolidays::holidays_of(2100).is_err());
    // 範囲外の年でも、スケジューラが落ちないよう「祝日ではない」を返す
    assert!(!JapaneseHolidays.is_holiday(date(2100, 1, 1)));
}
