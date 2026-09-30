package tokyo.runo.maidcafese.core

import java.time.DayOfWeek
import java.time.LocalDate
import java.time.LocalDateTime
import java.time.LocalTime
import java.time.temporal.ChronoUnit
import java.time.temporal.TemporalAdjusters

/** 「どの日に鳴らすか」の定型ルール。 */
sealed interface Recurrence {
    /** 毎日。 */
    data object Daily : Recurrence

    /** 平日(月〜金)。[skipHolidays]=true なら祝日を除く。 */
    data class Weekdays(val skipHolidays: Boolean = true) : Recurrence

    /** 土日祝日(平日ルールの補集合、祝日を休みとして扱う)。 */
    data object WeekendsAndHolidays : Recurrence

    /** 曜日指定(複数可)。 */
    data class DaysOfWeek(val days: Set<DayOfWeek>) : Recurrence {
        init { require(days.isNotEmpty()) { "曜日を1つ以上指定してください" } }
    }

    /** 毎月の第[nth]週の[day]曜日。[nth]は1..5、最終週は-1。存在しない月(第5など)は鳴らさない。 */
    data class NthWeekdayOfMonth(val nth: Int, val day: DayOfWeek) : Recurrence {
        init { require(nth in 1..5 || nth == -1) { "nthは1..5または-1(最終): $nth" } }
    }

    /** [interval]週間ごとの[days]曜日。基準週は[anchor]を含む週(月曜始まり)。 */
    data class EveryNWeeks(val interval: Int, val days: Set<DayOfWeek>, val anchor: LocalDate) : Recurrence {
        init {
            require(interval >= 1) { "intervalは1以上: $interval" }
            require(days.isNotEmpty()) { "曜日を1つ以上指定してください" }
        }
    }

    fun matches(date: LocalDate, holidays: HolidayCalendar): Boolean = when (this) {
        Daily -> true
        is Weekdays -> date.dayOfWeek !in WEEKEND && !(skipHolidays && holidays.isHoliday(date))
        WeekendsAndHolidays -> date.dayOfWeek in WEEKEND || holidays.isHoliday(date)
        is DaysOfWeek -> date.dayOfWeek in days
        is NthWeekdayOfMonth -> date.dayOfWeek == day && when (nth) {
            -1 -> date == date.with(TemporalAdjusters.lastInMonth(day))
            else -> (date.dayOfMonth - 1) / 7 + 1 == nth
        }
        is EveryNWeeks -> {
            val weeks = ChronoUnit.WEEKS.between(anchor.monday(), date.monday())
            date.dayOfWeek in days && weeks >= 0 && weeks % interval == 0L
        }
    }

    private companion object {
        val WEEKEND = setOf(DayOfWeek.SATURDAY, DayOfWeek.SUNDAY)
        fun LocalDate.monday(): LocalDate = with(TemporalAdjusters.previousOrSame(DayOfWeek.MONDAY))
    }
}

/** 何時に鳴らすか+有効期間。[preNoticeMinutes]が非nullなら、その分前に予告も鳴らす。 */
data class Schedule(
    val recurrence: Recurrence,
    val time: LocalTime,
    val startDate: LocalDate? = null,
    val endDate: LocalDate? = null,
    val preNoticeMinutes: Int? = null,
) {
    init { require(preNoticeMinutes == null || preNoticeMinutes > 0) { "予告は1分以上前" } }

    /** [after]より後の最初の発火時刻。探索は最大 [SEARCH_DAYS] 日先まで、無ければnull。 */
    fun nextTrigger(after: LocalDateTime, holidays: HolidayCalendar = JapaneseHolidays): LocalDateTime? {
        var date = after.toLocalDate()
        if (startDate != null && date.isBefore(startDate)) date = startDate
        repeat(SEARCH_DAYS) {
            if (endDate != null && date.isAfter(endDate)) return null
            if (recurrence.matches(date, holidays)) {
                val dt = date.atTime(time)
                if (dt.isAfter(after)) return dt
            }
            date = date.plusDays(1)
        }
        return null
    }

    /** [after]より後の最初の予告時刻(予告なし設定ならnull)。予告時刻が過去でも本番時刻が未来なら次回分を探す。 */
    fun nextPreNotice(after: LocalDateTime, holidays: HolidayCalendar = JapaneseHolidays): LocalDateTime? {
        val m = preNoticeMinutes ?: return null
        // 予告は本番の m 分前。after+m分より後の本番を探せば、その予告は after より後になる。
        val trigger = nextTrigger(after.plusMinutes(m.toLong()), holidays) ?: return null
        return trigger.minusMinutes(m.toLong())
    }

    companion object {
        const val SEARCH_DAYS = 366 * 5
    }
}
