//! 曜日の集合と、曜日の名前(保存形式・画面表示)。

use chrono::Weekday;

/// 月〜日(月曜始まり)。
pub const ALL_DAYS: [Weekday; 7] = [
    Weekday::Mon,
    Weekday::Tue,
    Weekday::Wed,
    Weekday::Thu,
    Weekday::Fri,
    Weekday::Sat,
    Weekday::Sun,
];

/// 曜日の集合(ビット集合)。反復は月曜→日曜の順。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct DaySet(u8);

impl DaySet {
    pub fn new(days: &[Weekday]) -> Self {
        let mut s = DaySet(0);
        for d in days {
            s.insert(*d);
        }
        s
    }

    fn bit(d: Weekday) -> u8 {
        1 << d.num_days_from_monday()
    }

    pub fn insert(&mut self, d: Weekday) {
        self.0 |= Self::bit(d);
    }

    pub fn remove(&mut self, d: Weekday) {
        self.0 &= !Self::bit(d);
    }

    pub fn contains(&self, d: Weekday) -> bool {
        self.0 & Self::bit(d) != 0
    }

    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }

    /// 月曜→日曜の順。
    pub fn iter(&self) -> impl Iterator<Item = Weekday> + '_ {
        ALL_DAYS.into_iter().filter(|d| self.contains(*d))
    }
}

impl std::fmt::Debug for DaySet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.iter().map(weekday_name)).finish()
    }
}

/// 保存形式での曜日名(Java/Kotlinの`DayOfWeek.toString()`と同じ大文字英名)。
pub fn weekday_name(d: Weekday) -> &'static str {
    match d {
        Weekday::Mon => "MONDAY",
        Weekday::Tue => "TUESDAY",
        Weekday::Wed => "WEDNESDAY",
        Weekday::Thu => "THURSDAY",
        Weekday::Fri => "FRIDAY",
        Weekday::Sat => "SATURDAY",
        Weekday::Sun => "SUNDAY",
    }
}

pub fn parse_weekday(s: &str) -> Option<Weekday> {
    ALL_DAYS.into_iter().find(|d| weekday_name(*d) == s)
}

/// 画面表示用の一文字(月〜日)。
pub fn weekday_ja(d: Weekday) -> &'static str {
    match d {
        Weekday::Mon => "月",
        Weekday::Tue => "火",
        Weekday::Wed => "水",
        Weekday::Thu => "木",
        Weekday::Fri => "金",
        Weekday::Sat => "土",
        Weekday::Sun => "日",
    }
}
