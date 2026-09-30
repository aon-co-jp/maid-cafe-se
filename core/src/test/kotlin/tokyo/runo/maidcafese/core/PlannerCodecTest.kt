package tokyo.runo.maidcafese.core

import java.time.DayOfWeek
import java.time.LocalDate
import java.time.LocalDateTime
import java.time.LocalTime
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue

class PlannerCodecTest {
    private fun dt(y: Int, m: Int, d: Int, h: Int, min: Int = 0) = LocalDateTime.of(y, m, d, h, min)

    private fun entry(
        id: String = "a", kind: AlarmKind = AlarmKind.SOUND, pre: Int? = null,
        time: LocalTime = LocalTime.of(7, 0), enabled: Boolean = true, voice: VoiceStyle = VoiceStyle.MAID,
    ) = AlarmEntry(
        id, "起床", Schedule(Recurrence.Daily, time, preNoticeMinutes = pre), kind,
        text = "薬を飲む", voice = voice, enabled = enabled,
    )

    @Test fun soundEntry() {
        val r = Planner.next(listOf(entry()), emptyList(), CalendarSettings(), dt(2026, 9, 30, 0))
        assertEquals(1, r.size)
        assertEquals(dt(2026, 9, 30, 7), r[0].time)
        assertEquals("chime", r[0].soundId)
        assertNull(r[0].speech)
    }

    @Test fun speechEntryMaidAndDeep() {
        val maid = Planner.next(listOf(entry(kind = AlarmKind.SPEECH)), emptyList(), CalendarSettings(), dt(2026, 9, 30, 0))
        assertNull(maid[0].soundId)
        assertTrue(maid[0].speech!!.contains("ご主人様") && maid[0].speech!!.contains("薬を飲む"))
        val deep = Planner.next(listOf(entry(kind = AlarmKind.SPEECH, voice = VoiceStyle.DEEP_MALE)), emptyList(), CalendarSettings(), dt(2026, 9, 30, 0))
        assertEquals("時間だ。薬を飲む", deep[0].speech)
        assertEquals(VoiceStyle.DEEP_MALE, deep[0].voice)
    }

    @Test fun preNoticeComesFirst() {
        val r = Planner.next(listOf(entry(pre = 30)), emptyList(), CalendarSettings(), dt(2026, 9, 30, 0))
        assertEquals(dt(2026, 9, 30, 6, 30), r[0].time)
        assertTrue(r[0].key.startsWith("pre:"))
        assertTrue(r[0].speech!!.contains("30分"))
    }

    @Test fun disabledIgnored() {
        assertTrue(Planner.next(listOf(entry(enabled = false)), emptyList(), CalendarSettings(), dt(2026, 9, 30, 0)).isEmpty())
    }

    @Test fun calendarEventsWithPreNotice() {
        val ev = CalendarEvent("e1", "会議", dt(2026, 9, 30, 10))
        val on = CalendarSettings(enabled = true, preNotice = true)
        val r = Planner.next(emptyList(), listOf(ev), on, dt(2026, 9, 30, 8))
        assertEquals(dt(2026, 9, 30, 9, 30), r[0].time)
        assertTrue(r[0].speech!!.contains("会議"))
        val r2 = Planner.next(emptyList(), listOf(ev), on, dt(2026, 9, 30, 9, 30))
        assertEquals(dt(2026, 9, 30, 10), r2[0].time)
    }

    @Test fun calendarPreNoticeCheckboxOffAndDisabled() {
        val ev = CalendarEvent("e1", "会議", dt(2026, 9, 30, 10))
        val off = Planner.next(emptyList(), listOf(ev), CalendarSettings(enabled = true, preNotice = false), dt(2026, 9, 30, 8))
        assertEquals(dt(2026, 9, 30, 10), off[0].time)
        assertTrue(Planner.next(emptyList(), listOf(ev), CalendarSettings(enabled = false), dt(2026, 9, 30, 8)).isEmpty())
    }

    @Test fun simultaneousOccurrencesReturnedTogether() {
        val ev = CalendarEvent("e1", "会議", dt(2026, 9, 30, 7))
        val r = Planner.next(listOf(entry()), listOf(ev), CalendarSettings(enabled = true, preNotice = false), dt(2026, 9, 30, 0))
        assertEquals(2, r.size)
    }

    @Test fun eventInPastIgnored() {
        val ev = CalendarEvent("e1", "会議", dt(2026, 9, 30, 10))
        assertTrue(Planner.next(emptyList(), listOf(ev), CalendarSettings(enabled = true), dt(2026, 9, 30, 10)).isEmpty())
    }

    @Test fun codecRoundTripAllRecurrences() {
        val anchor = LocalDate.of(2026, 9, 7)
        val recs = listOf(
            Recurrence.Daily, Recurrence.Weekdays(true), Recurrence.Weekdays(false), Recurrence.WeekendsAndHolidays,
            Recurrence.DaysOfWeek(setOf(DayOfWeek.MONDAY, DayOfWeek.FRIDAY)),
            Recurrence.NthWeekdayOfMonth(2, DayOfWeek.TUESDAY), Recurrence.NthWeekdayOfMonth(-1, DayOfWeek.SUNDAY),
            Recurrence.EveryNWeeks(3, setOf(DayOfWeek.MONDAY, DayOfWeek.THURSDAY), anchor),
        )
        val list = recs.mapIndexed { i, r ->
            AlarmEntry(
                "id$i", "ラベル&=% 日本語\n改行", Schedule(r, LocalTime.of(6, 5), LocalDate.of(2026, 1, 1), LocalDate.of(2027, 1, 1), 30),
                AlarmKind.SPEECH, "melody", "本文 a=b&c\n2行目", VoiceStyle.DEEP_MALE, i % 2 == 0,
            )
        }
        assertEquals(list, Codec.decodeAll(Codec.encodeAll(list)))
    }

    @Test fun codecNullableFieldsAndBrokenLines() {
        val e = entry()
        assertEquals(listOf(e), Codec.decodeAll(Codec.encodeAll(listOf(e)) + "\nこわれた行\nid=x&label=y"))
        assertEquals(emptyList(), Codec.decodeAll(""))
    }

    @Test fun unknownSoundFallsBackToDefault() {
        val line = Codec.encode(entry()).replace("sound=chime", "sound=nonexistent")
        assertEquals("chime", Codec.decode(line).soundId)
    }

    @Test fun speechAlarmIsVoiceOnlyUnlessSoundIsOn() {
        val voiceOnly = entry(kind = AlarmKind.SPEECH)
        val r1 = Planner.next(listOf(voiceOnly), emptyList(), CalendarSettings(), dt(2026, 9, 30, 0))
        assertNull(r1[0].soundId)
        assertTrue(r1[0].speech != null)
        val both = voiceOnly.copy(speechSound = true)
        val r2 = Planner.next(listOf(both), emptyList(), CalendarSettings(), dt(2026, 9, 30, 0))
        assertEquals("chime", r2[0].soundId)
        assertTrue(r2[0].speech != null)
        // 保存形式: オンのときだけ ss=1(オフの行は従来と同じ)、往復で保たれる
        assertTrue(Codec.encode(both).contains("&ss=1"))
        assertTrue(!Codec.encode(voiceOnly).contains("ss="))
        assertEquals(both, Codec.decode(Codec.encode(both)))
        assertEquals(voiceOnly, Codec.decode(Codec.encode(voiceOnly)))
    }
}
