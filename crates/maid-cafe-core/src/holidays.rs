//! 日本の国民の祝日(祝日法)を、ルールから計算する。外部データ・ネットワーク不要。
//!
//! 対象は2007年以降(昭和の日・みどりの日の現行体系)、春分/秋分の日の近似式が有効な2099年まで。
//! 2019年の即位関連、2020/2021年の東京五輪に伴う祝日移動、振替休日、国民の休日に対応。

use chrono::{Datelike, Duration, NaiveDate, Weekday};
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

/// 祝日判定の抽象。テストや将来の外部データ差し替えのため trait にしている。
pub trait HolidayCalendar {
    fn is_holiday(&self, date: NaiveDate) -> bool;
}

/// 祝日なし(土日のみ休みとして扱いたい場合・テスト用)。
pub struct NoHolidays;

impl HolidayCalendar for NoHolidays {
    fn is_holiday(&self, _date: NaiveDate) -> bool {
        false
    }
}

pub const MIN_YEAR: i32 = 2007;
pub const MAX_YEAR: i32 = 2099;

/// 日本の祝日。対応年(2007〜2099)の外では「祝日ではない」を返す(クラッシュさせない)。
pub struct JapaneseHolidays;

fn cache() -> &'static Mutex<HashMap<i32, HashSet<NaiveDate>>> {
    static CACHE: OnceLock<Mutex<HashMap<i32, HashSet<NaiveDate>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

impl HolidayCalendar for JapaneseHolidays {
    fn is_holiday(&self, date: NaiveDate) -> bool {
        match JapaneseHolidays::holidays_of(date.year()) {
            Ok(set) => set.contains(&date),
            Err(_) => false,
        }
    }
}

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).expect("固定の祝日の日付は常に有効")
}

/// その月の第[nth]月曜日。
fn nth_monday(year: i32, month: u32, nth: u32) -> NaiveDate {
    let first = d(year, month, 1);
    let offset = (7 + Weekday::Mon.num_days_from_monday() as i64 - first.weekday().num_days_from_monday() as i64) % 7;
    first + Duration::days(offset + 7 * (nth as i64 - 1))
}

/// 春分/秋分の日(1980〜2099年の近似式)。
fn equinox_day(year: i32, base: f64) -> u32 {
    let dy = year - 1980;
    (base + 0.242194 * dy as f64 - (dy / 4) as f64) as u32
}

impl JapaneseHolidays {
    /// その年の祝日の集合。対応年の外は`Err`。
    pub fn holidays_of(year: i32) -> Result<HashSet<NaiveDate>, String> {
        if !(MIN_YEAR..=MAX_YEAR).contains(&year) {
            return Err(format!("対応年は{MIN_YEAR}..{MAX_YEAR}年です: {year}"));
        }
        let mut c = cache().lock().unwrap_or_else(|e| e.into_inner());
        Ok(c.entry(year).or_insert_with(|| compute(year)).clone())
    }
}

fn compute(year: i32) -> HashSet<NaiveDate> {
    let mut base: HashSet<NaiveDate> = HashSet::new();
    base.insert(d(year, 1, 1)); // 元日
    base.insert(nth_monday(year, 1, 2)); // 成人の日
    base.insert(d(year, 2, 11)); // 建国記念の日
    if year >= 2020 {
        base.insert(d(year, 2, 23)); // 天皇誕生日(令和)
    }
    if year <= 2018 {
        base.insert(d(year, 12, 23)); // 天皇誕生日(平成、2019年は該当なし)
    }
    base.insert(d(year, 3, equinox_day(year, 20.8431))); // 春分の日
    base.insert(d(year, 4, 29)); // 昭和の日
    base.insert(d(year, 5, 3)); // 憲法記念日
    base.insert(d(year, 5, 4)); // みどりの日
    base.insert(d(year, 5, 5)); // こどもの日
    match year {
        // 海の日・スポーツの日・山の日(2020/2021は五輪で移動)
        2020 => {
            base.insert(d(year, 7, 23));
            base.insert(d(year, 7, 24));
            base.insert(d(year, 8, 10));
        }
        2021 => {
            base.insert(d(year, 7, 22));
            base.insert(d(year, 7, 23));
            base.insert(d(year, 8, 8));
        }
        _ => {
            base.insert(nth_monday(year, 7, 3));
            base.insert(d(year, 8, 11));
            base.insert(nth_monday(year, 10, 2));
        }
    }
    base.insert(nth_monday(year, 9, 3)); // 敬老の日
    base.insert(d(year, 9, equinox_day(year, 23.2488))); // 秋分の日
    base.insert(d(year, 11, 3)); // 文化の日
    base.insert(d(year, 11, 23)); // 勤労感謝の日
    if year == 2019 {
        base.insert(d(year, 5, 1)); // 即位の日
        base.insert(d(year, 10, 22)); // 即位礼正殿の儀
    }

    let mut all = base.clone();
    // 国民の休日: 前後を祝日に挟まれた平日(日曜・祝日を除く)
    let mut day = d(year, 1, 2);
    let end = d(year, 12, 30);
    while day <= end {
        if !base.contains(&day)
            && day.weekday() != Weekday::Sun
            && base.contains(&(day - Duration::days(1)))
            && base.contains(&(day + Duration::days(1)))
        {
            all.insert(day);
        }
        day += Duration::days(1);
    }
    // 振替休日: 日曜の祝日の次の「祝日でない日」(順序を決めるため日付順に処理)
    let mut sorted: Vec<NaiveDate> = base.iter().copied().collect();
    sorted.sort();
    for h in sorted {
        if h.weekday() == Weekday::Sun {
            let mut s = h + Duration::days(1);
            while all.contains(&s) {
                s += Duration::days(1);
            }
            all.insert(s);
        }
    }
    all
}
