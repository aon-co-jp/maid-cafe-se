package tokyo.runo.maidcafese.core

import java.time.DayOfWeek.FRIDAY
import java.time.DayOfWeek.MONDAY
import java.time.DayOfWeek.SATURDAY
import java.time.DayOfWeek.SUNDAY
import java.time.DayOfWeek.TUESDAY
import java.time.DayOfWeek.WEDNESDAY
import java.time.LocalDate
import java.time.LocalDateTime
import java.time.LocalTime
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

class RecurrenceTest {
    private fun date(y: Int, m: Int, d: Int) = LocalDate.of(y, m, d)
    private fun dt(y: Int, m: Int, d: Int, h: Int, min: Int = 0) = LocalDateTime.of(y, m, d, h, min)
    private val seven = LocalTime.of(7, 0)

    @Test fun daily() {
        assertTrue(Recurrence.Daily.matches(date(2026, 1, 1), NoHolidays))
    }

    @Test fun weekdays_skipHolidays() {
        val r = Recurrence.Weekdays()
        assertFalse(r.matches(date(2026, 5, 6), JapaneseHolidays)) // 振替休日(水)
        assertTrue(r.matches(date(2026, 5, 7), JapaneseHolidays))
        assertFalse(r.matches(date(2026, 5, 9), JapaneseHolidays)) // 土
        assertTrue(Recurrence.Weekdays(skipHolidays = false).matches(date(2026, 5, 6), JapaneseHolidays))
    }

    @Test fun weekendsAndHolidays_isComplementOfWeekdays() {
        var d = date(2026, 1, 1)
        while (d.year == 2026) {
            assertEquals(
                !Recurrence.Weekdays().matches(d, JapaneseHolidays),
                Recurrence.WeekendsAndHolidays.matches(d, JapaneseHolidays),
                d.toString(),
            )
            d = d.plusDays(1)
        }
    }

    @Test fun daysOfWeek() {
        val r = Recurrence.DaysOfWeek(setOf(MONDAY, WEDNESDAY))
        assertTrue(r.matches(date(2026, 9, 28), NoHolidays)) // 月
        assertFalse(r.matches(date(2026, 9, 29), NoHolidays)) // 火
        assertFailsWith<IllegalArgumentException> { Recurrence.DaysOfWeek(emptySet()) }
    }

    @Test fun nthWeekday() {
        val second = Recurrence.NthWeekdayOfMonth(2, TUESDAY)
        assertTrue(second.matches(date(2026, 9, 8), NoHolidays)) // 9/1,8 が火 → 第2
        assertFalse(second.matches(date(2026, 9, 1), NoHolidays))
        assertFalse(second.matches(date(2026, 9, 15), NoHolidays))
        // 2026-09 は火曜が 1,8,15,22,29 → 第5・最終は29
        assertTrue(Recurrence.NthWeekdayOfMonth(5, TUESDAY).matches(date(2026, 9, 29), NoHolidays))
        assertTrue(Recurrence.NthWeekdayOfMonth(-1, TUESDAY).matches(date(2026, 9, 29), NoHolidays))
        assertFalse(Recurrence.NthWeekdayOfMonth(-1, TUESDAY).matches(date(2026, 9, 22), NoHolidays))
        // 2026-10 は火曜が 6,13,20,27 → 第5は存在せず、次に第5火曜がある月(12/29)まで飛ぶ
        val s = Schedule(Recurrence.NthWeekdayOfMonth(5, TUESDAY), seven)
        assertEquals(dt(2026, 12, 29, 7), s.nextTrigger(dt(2026, 9, 30, 0), NoHolidays))
        assertFailsWith<IllegalArgumentException> { Recurrence.NthWeekdayOfMonth(6, TUESDAY) }
    }

    @Test fun everyNWeeks() {
        val anchor = date(2026, 9, 7) // 月曜
        val r = Recurrence.EveryNWeeks(2, setOf(MONDAY, FRIDAY), anchor)
        assertTrue(r.matches(date(2026, 9, 7), NoHolidays))
        assertTrue(r.matches(date(2026, 9, 11), NoHolidays)) // 同じ週の金
        assertFalse(r.matches(date(2026, 9, 14), NoHolidays)) // 翌週
        assertTrue(r.matches(date(2026, 9, 21), NoHolidays)) // 2週後
        assertFalse(r.matches(date(2026, 8, 24), NoHolidays)) // 基準より前は鳴らさない
        // 基準日が週の途中(水)でも、その週の月曜からを1週目とする
        val mid = Recurrence.EveryNWeeks(2, setOf(MONDAY), date(2026, 9, 9))
        assertTrue(mid.matches(date(2026, 9, 7), NoHolidays))
        assertTrue(mid.matches(date(2026, 9, 21), NoHolidays))
        assertFailsWith<IllegalArgumentException> { Recurrence.EveryNWeeks(0, setOf(MONDAY), anchor) }
    }

    @Test fun nextTrigger_basicAndStrictlyAfter() {
        val s = Schedule(Recurrence.Daily, seven)
        assertEquals(dt(2026, 9, 30, 7), s.nextTrigger(dt(2026, 9, 30, 6, 59), NoHolidays))
        assertEquals(dt(2026, 10, 1, 7), s.nextTrigger(dt(2026, 9, 30, 7, 0), NoHolidays)) // ちょうどは含めない
    }

    @Test fun nextTrigger_weekdaysSkipsHolidayWeekend() {
        val s = Schedule(Recurrence.Weekdays(), seven)
        // 2026-05-02(土)の後: 5/3日 5/4月 5/5火 5/6水 は休み → 5/7木
        assertEquals(dt(2026, 5, 7, 7), s.nextTrigger(dt(2026, 5, 2, 12), JapaneseHolidays))
    }

    @Test fun nextTrigger_startEndDate() {
        val s = Schedule(Recurrence.Daily, seven, startDate = date(2026, 10, 5), endDate = date(2026, 10, 6))
        assertEquals(dt(2026, 10, 5, 7), s.nextTrigger(dt(2026, 9, 30, 0), NoHolidays))
        assertEquals(dt(2026, 10, 6, 7), s.nextTrigger(dt(2026, 10, 5, 8), NoHolidays))
        assertNull(s.nextTrigger(dt(2026, 10, 6, 8), NoHolidays))
    }

    @Test fun preNotice() {
        val s = Schedule(Recurrence.Daily, LocalTime.of(9, 0), preNoticeMinutes = 30)
        assertEquals(dt(2026, 9, 30, 8, 30), s.nextPreNotice(dt(2026, 9, 30, 8, 0), NoHolidays))
        // 予告時刻を過ぎて本番前 → 予告は翌日分
        assertEquals(dt(2026, 10, 1, 8, 30), s.nextPreNotice(dt(2026, 9, 30, 8, 45), NoHolidays))
        assertNull(Schedule(Recurrence.Daily, seven).nextPreNotice(dt(2026, 9, 30, 0), NoHolidays))
        // 0:10発火・30分前は前日23:40
        val early = Schedule(Recurrence.Daily, LocalTime.of(0, 10), preNoticeMinutes = 30)
        assertEquals(dt(2026, 9, 30, 23, 40), early.nextPreNotice(dt(2026, 9, 30, 12), NoHolidays))
    }

    @Test fun weekend() {
        val r = Recurrence.DaysOfWeek(setOf(SATURDAY, SUNDAY))
        assertTrue(r.matches(date(2026, 10, 3), NoHolidays))
    }
}
