//! 「どの日に鳴らすか」の定型ルールと、それに時刻・有効期間を足した[`Schedule`]。

use crate::days::DaySet;
use crate::holidays::{HolidayCalendar, JapaneseHolidays};
use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, Weekday};

/// 「どの日に鳴らすか」。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Recurrence {
    /// 毎日。
    Daily,
    /// 平日(月〜金)。`skip_holidays`なら祝日を除く。
    Weekdays { skip_holidays: bool },
    /// 土日祝日(平日ルールの補集合、祝日を休みとして扱う)。
    WeekendsAndHolidays,
    /// 曜日指定(複数可)。
    DaysOfWeek(DaySet),
    /// 毎月の第`nth`週の`day`曜日。`nth`は1..=5、最終週は-1。存在しない月(第5など)は鳴らさない。
    NthWeekdayOfMonth { nth: i32, day: Weekday },
    /// `interval`週間ごとの`days`曜日。基準週は`anchor`を含む週(月曜始まり)。
    EveryNWeeks { interval: u32, days: DaySet, anchor: NaiveDate },
}

impl Recurrence {
    /// 平日(祝日を除く)。
    pub fn weekdays() -> Self {
        Recurrence::Weekdays { skip_holidays: true }
    }

    pub fn days_of_week(days: DaySet) -> Result<Self, String> {
        if days.is_empty() {
            return Err("曜日を1つ以上指定してください".into());
        }
        Ok(Recurrence::DaysOfWeek(days))
    }

    pub fn nth_weekday_of_month(nth: i32, day: Weekday) -> Result<Self, String> {
        if !((1..=5).contains(&nth) || nth == -1) {
            return Err(format!("nthは1..5または-1(最終): {nth}"));
        }
        Ok(Recurrence::NthWeekdayOfMonth { nth, day })
    }

    pub fn every_n_weeks(interval: u32, days: DaySet, anchor: NaiveDate) -> Result<Self, String> {
        if interval < 1 {
            return Err(format!("intervalは1以上: {interval}"));
        }
        if days.is_empty() {
            return Err("曜日を1つ以上指定してください".into());
        }
        Ok(Recurrence::EveryNWeeks { interval, days, anchor })
    }

    pub fn matches(&self, date: NaiveDate, holidays: &dyn HolidayCalendar) -> bool {
        let is_weekend = matches!(date.weekday(), Weekday::Sat | Weekday::Sun);
        match self {
            Recurrence::Daily => true,
            Recurrence::Weekdays { skip_holidays } => !is_weekend && !(*skip_holidays && holidays.is_holiday(date)),
            Recurrence::WeekendsAndHolidays => is_weekend || holidays.is_holiday(date),
            Recurrence::DaysOfWeek(days) => days.contains(date.weekday()),
            Recurrence::NthWeekdayOfMonth { nth, day } => {
                if date.weekday() != *day {
                    return false;
                }
                if *nth == -1 {
                    // 7日後が翌月なら、その月の最終
                    (date + Duration::days(7)).month() != date.month()
                } else {
                    ((date.day() as i32 - 1) / 7 + 1) == *nth
                }
            }
            Recurrence::EveryNWeeks { interval, days, anchor } => {
                let weeks = (monday_of(date) - monday_of(*anchor)).num_weeks();
                days.contains(date.weekday()) && weeks >= 0 && weeks % (*interval as i64) == 0
            }
        }
    }
}

/// その日を含む週の月曜日。
fn monday_of(d: NaiveDate) -> NaiveDate {
    d - Duration::days(d.weekday().num_days_from_monday() as i64)
}

/// 何時に鳴らすか+有効期間。`pre_notice_minutes`が`Some`なら、その分前に予告も鳴らす。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Schedule {
    pub recurrence: Recurrence,
    pub time: NaiveTime,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub pre_notice_minutes: Option<u32>,
}

/// 探索は最大この日数先まで。
pub const SEARCH_DAYS: i64 = 366 * 5;

impl Schedule {
    pub fn new(recurrence: Recurrence, time: NaiveTime) -> Self {
        Schedule { recurrence, time, start_date: None, end_date: None, pre_notice_minutes: None }
    }

    pub fn with_pre_notice(mut self, minutes: u32) -> Result<Self, String> {
        if minutes == 0 {
            return Err("予告は1分以上前".into());
        }
        self.pre_notice_minutes = Some(minutes);
        Ok(self)
    }

    pub fn with_period(mut self, start: Option<NaiveDate>, end: Option<NaiveDate>) -> Self {
        self.start_date = start;
        self.end_date = end;
        self
    }

    /// `after`より後の最初の発火時刻。探索は最大[`SEARCH_DAYS`]日先まで、無ければ`None`。
    pub fn next_trigger(&self, after: NaiveDateTime, holidays: &dyn HolidayCalendar) -> Option<NaiveDateTime> {
        let mut date = after.date();
        if let Some(start) = self.start_date {
            if date < start {
                date = start;
            }
        }
        for _ in 0..SEARCH_DAYS {
            if let Some(end) = self.end_date {
                if date > end {
                    return None;
                }
            }
            if self.recurrence.matches(date, holidays) {
                let dt = date.and_time(self.time);
                if dt > after {
                    return Some(dt);
                }
            }
            date += Duration::days(1);
        }
        None
    }

    /// `after`より後の最初の予告時刻(予告なし設定なら`None`)。
    pub fn next_pre_notice(&self, after: NaiveDateTime, holidays: &dyn HolidayCalendar) -> Option<NaiveDateTime> {
        let m = self.pre_notice_minutes? as i64;
        // 予告は本番の m 分前。after+m分より後の本番を探せば、その予告は after より後になる。
        let trigger = self.next_trigger(after + Duration::minutes(m), holidays)?;
        Some(trigger - Duration::minutes(m))
    }

    /// 既定の日本の祝日で[`next_trigger`](Self::next_trigger)。
    pub fn next_trigger_jp(&self, after: NaiveDateTime) -> Option<NaiveDateTime> {
        self.next_trigger(after, &JapaneseHolidays)
    }
}
