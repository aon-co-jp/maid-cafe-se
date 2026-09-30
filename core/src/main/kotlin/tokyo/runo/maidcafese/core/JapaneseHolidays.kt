package tokyo.runo.maidcafese.core

import java.time.DayOfWeek
import java.time.LocalDate
import java.time.temporal.TemporalAdjusters

/** 祝日判定の抽象。テストや将来の外部データ差し替えのため interface にしている。 */
fun interface HolidayCalendar {
    fun isHoliday(date: LocalDate): Boolean
}

/** 祝日なし(土日のみ休みとして扱いたい場合・テスト用)。 */
object NoHolidays : HolidayCalendar {
    override fun isHoliday(date: LocalDate) = false
}

/**
 * 日本の国民の祝日(祝日法)をルールから計算する。外部データ・ネットワーク不要。
 *
 * 対象は2007年以降(昭和の日・みどりの日の現行体系)、春分/秋分の日の近似式が
 * 有効な2099年まで。範囲外の年は [IllegalArgumentException]。
 * 2019年の即位関連、2020/2021年の東京五輪に伴う祝日移動、振替休日、国民の休日に対応。
 */
object JapaneseHolidays : HolidayCalendar {
    const val MIN_YEAR = 2007
    const val MAX_YEAR = 2099

    private val cache = HashMap<Int, Set<LocalDate>>()

    override fun isHoliday(date: LocalDate): Boolean = holidaysOf(date.year).contains(date)

    @Synchronized
    fun holidaysOf(year: Int): Set<LocalDate> {
        require(year in MIN_YEAR..MAX_YEAR) { "対応年は$MIN_YEAR..${MAX_YEAR}年です: $year" }
        return cache.getOrPut(year) { compute(year) }
    }

    private fun nthMonday(year: Int, month: Int, nth: Int): LocalDate =
        LocalDate.of(year, month, 1).with(TemporalAdjusters.dayOfWeekInMonth(nth, DayOfWeek.MONDAY))

    private fun equinoxDay(year: Int, base: Double): Int {
        val d = year - 1980
        return (base + 0.242194 * d - d / 4).toInt()
    }

    private fun compute(year: Int): Set<LocalDate> {
        fun d(m: Int, day: Int) = LocalDate.of(year, m, day)
        val base = HashSet<LocalDate>()
        base += d(1, 1) // 元日
        base += nthMonday(year, 1, 2) // 成人の日
        base += d(2, 11) // 建国記念の日
        if (year >= 2020) base += d(2, 23) // 天皇誕生日(令和)
        if (year <= 2018) base += d(12, 23) // 天皇誕生日(平成、2019年は該当なし)
        base += d(3, equinoxDay(year, 20.8431)) // 春分の日
        base += d(4, 29) // 昭和の日
        base += d(5, 3) // 憲法記念日
        base += d(5, 4) // みどりの日
        base += d(5, 5) // こどもの日
        when (year) { // 海の日・スポーツの日・山の日(2020/2021は五輪で移動)
            2020 -> { base += d(7, 23); base += d(7, 24); base += d(8, 10) }
            2021 -> { base += d(7, 22); base += d(7, 23); base += d(8, 8) }
            else -> {
                base += nthMonday(year, 7, 3)
                base += d(8, 11)
                base += nthMonday(year, 10, 2)
            }
        }
        base += nthMonday(year, 9, 3) // 敬老の日
        base += d(9, equinoxDay(year, 23.2488)) // 秋分の日
        base += d(11, 3) // 文化の日
        base += d(11, 23) // 勤労感謝の日
        if (year == 2019) { base += d(5, 1); base += d(10, 22) } // 即位の日・即位礼正殿の儀

        val all = HashSet(base)
        // 国民の休日: 前後を祝日に挟まれた平日(日曜・祝日を除く)
        var day = d(1, 2)
        val end = d(12, 30)
        while (!day.isAfter(end)) {
            if (day !in base && day.dayOfWeek != DayOfWeek.SUNDAY &&
                day.minusDays(1) in base && day.plusDays(1) in base
            ) all += day
            day = day.plusDays(1)
        }
        // 振替休日: 日曜の祝日の次の「祝日でない日」
        for (h in base.toList()) {
            if (h.dayOfWeek == DayOfWeek.SUNDAY) {
                var s = h.plusDays(1)
                while (s in all) s = s.plusDays(1)
                all += s
            }
        }
        return all
    }
}
