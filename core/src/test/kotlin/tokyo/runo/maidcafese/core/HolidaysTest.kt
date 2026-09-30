package tokyo.runo.maidcafese.core

import java.time.LocalDate
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class HolidaysTest {
    private fun set(year: Int, vararg md: Pair<Int, Int>) = md.map { LocalDate.of(year, it.first, it.second) }.toSet()

    @Test fun holidays2024() {
        val expected = set(
            2024, 1 to 1, 1 to 8, 2 to 11, 2 to 12, 2 to 23, 3 to 20, 4 to 29, 5 to 3, 5 to 4, 5 to 5, 5 to 6,
            7 to 15, 8 to 11, 8 to 12, 9 to 16, 9 to 22, 9 to 23, 10 to 14, 11 to 3, 11 to 4, 11 to 23,
        )
        assertEquals(expected, JapaneseHolidays.holidaysOf(2024))
    }

    @Test fun holidays2026_citizensHolidayAndSubstitute() {
        val h = JapaneseHolidays
        assertTrue(h.isHoliday(LocalDate.of(2026, 5, 6))) // 5/3(日)の振替
        assertTrue(h.isHoliday(LocalDate.of(2026, 9, 22))) // 国民の休日
        assertTrue(h.isHoliday(LocalDate.of(2026, 9, 23))) // 秋分の日
        assertTrue(h.isHoliday(LocalDate.of(2026, 3, 20))) // 春分の日
        assertFalse(h.isHoliday(LocalDate.of(2026, 5, 7)))
    }

    @Test fun holidays2019_enthronement() {
        val h = JapaneseHolidays.holidaysOf(2019)
        for (d in 29..30) assertTrue(LocalDate.of(2019, 4, d) in h)
        for (d in 1..6) assertTrue(LocalDate.of(2019, 5, d) in h)
        assertTrue(LocalDate.of(2019, 10, 22) in h)
        assertFalse(LocalDate.of(2019, 12, 23) in h)
        assertFalse(LocalDate.of(2019, 2, 23) in h)
    }

    @Test fun olympicYears() {
        assertTrue(LocalDate.of(2020, 7, 23) in JapaneseHolidays.holidaysOf(2020))
        assertTrue(LocalDate.of(2020, 7, 24) in JapaneseHolidays.holidaysOf(2020))
        assertTrue(LocalDate.of(2020, 8, 10) in JapaneseHolidays.holidaysOf(2020))
        assertFalse(LocalDate.of(2020, 10, 12) in JapaneseHolidays.holidaysOf(2020))
        assertTrue(LocalDate.of(2021, 8, 9) in JapaneseHolidays.holidaysOf(2021)) // 8/8(日)の振替
    }

    @Test fun heisei_emperorBirthday() {
        assertTrue(LocalDate.of(2018, 12, 23) in JapaneseHolidays.holidaysOf(2018))
        assertTrue(LocalDate.of(2018, 12, 24) in JapaneseHolidays.holidaysOf(2018)) // 日曜の振替
        assertTrue(LocalDate.of(2010, 12, 23) in JapaneseHolidays.holidaysOf(2010))
    }

    @Test fun outOfRange() {
        assertFailsWith<IllegalArgumentException> { JapaneseHolidays.holidaysOf(2006) }
        assertFailsWith<IllegalArgumentException> { JapaneseHolidays.holidaysOf(2100) }
    }
}
