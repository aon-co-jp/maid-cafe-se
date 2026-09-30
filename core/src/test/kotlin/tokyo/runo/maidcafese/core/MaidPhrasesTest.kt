package tokyo.runo.maidcafese.core

import java.time.LocalDateTime
import java.time.LocalTime
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

class MaidPhrasesTest {
    private fun dt(h: Int, m: Int = 0) = LocalDateTime.of(2026, 9, 30, h, m)

    private fun entry(
        kind: AlarmKind = AlarmKind.SPEECH,
        text: String = "薬を飲む",
        phrases: Set<String> = emptySet(),
        prePhrases: Set<String> = emptySet(),
        harmony: Boolean = false,
        pre: Int? = null,
        voice: VoiceStyle = VoiceStyle.MAID,
    ) = AlarmEntry(
        "a", "起床", Schedule(Recurrence.Daily, LocalTime.of(7, 0), preNoticeMinutes = pre), kind,
        text = text, voice = voice, phrases = phrases, prePhrases = prePhrases, harmony = harmony,
    )

    private fun first(e: AlarmEntry, after: LocalDateTime = dt(0)) =
        Planner.next(listOf(e), emptyList(), CalendarSettings(), after, NoHolidays).first()

    @Test fun catalogHasTheRequestedPhrases() {
        val display = MaidPhrases.all.map { it.display }
        assertTrue("おかえりなさいませご主人様！" in display)
        assertTrue("おいしくな～れ萌え萌えキュ～ン" in display)
        assertTrue(display.any { it.startsWith("メッ！ダメなんだぞこら！") && it.endsWith("頑張って行きましょう！") })
        assertTrue("ファイト！ファイト！" in display)
        assertTrue("エクセレント！" in display)
        assertTrue("パーフェクト！" in display)
        assertTrue("ご主人さま～、お～き～て～。今日も頑張って～" in display)
        assertEquals(7, MaidPhrases.all.map { it.id }.toSet().size)
    }

    @Test fun singlePhraseAppendedAfterMessage() {
        val speech = first(entry(phrases = setOf("okaeri"))).speech!!
        assertTrue(speech.contains("薬を飲む"))
        assertTrue(speech.endsWith("おかえりなさいませ、ご主人様！"))
    }

    @Test fun multiplePhrasesCombinedInCatalogOrder() {
        // 選択順に関わらずカタログ順(おかえり→…→パーフェクト)で連結される
        val speech = first(entry(phrases = setOf("perfect", "okaeri", "fight"))).speech!!
        val i1 = speech.indexOf("おかえりなさいませ")
        val i2 = speech.indexOf("ファイト！ファイト！")
        val i3 = speech.indexOf("パーフェクト！")
        assertTrue(i1 in 0 until i2 && i2 < i3, speech)
    }

    @Test fun wakeUpPhraseComesFirstWhenCombined() {
        val speech = first(entry(text = "", phrases = setOf("okaeri", "okite"))).speech!!
        assertTrue(speech.startsWith("ご主人さまー、おーきーてー。今日も、がんばってー"), speech)
        assertTrue(speech.indexOf("おかえりなさいませ") > speech.indexOf("おーきーてー"))
        val seg = first(entry(text = "", phrases = setOf("okite"))).segments
        assertEquals(1, seg.size)
        assertEquals(0.8f, seg[0].rate) // ゆっくり間延びさせて読む
    }

    @Test fun phrasesOnlyWhenTextBlank() {
        val speech = first(entry(text = "", phrases = setOf("excellent"))).speech
        assertEquals("エクセレント！", speech)
    }

    @Test fun blankTextAndNoPhrasesFallsBackToLabel() {
        assertTrue(first(entry(text = "")).speech!!.contains("起床"))
    }

    @Test fun soundKindWithPhrasesPlaysBoth() {
        val o = first(entry(kind = AlarmKind.SOUND, phrases = setOf("meh")))
        assertEquals("chime", o.soundId)
        assertTrue(o.speech!!.startsWith("めっ！だめなんだぞ"))
    }

    @Test fun soundKindWithoutPhrasesHasNoSpeech() {
        assertNull(first(entry(kind = AlarmKind.SOUND)).speech)
    }

    @Test fun preNoticeUsesPrePhrasesNotMainPhrases() {
        val e = entry(phrases = setOf("okaeri"), prePhrases = setOf("fight"), pre = 30)
        val pre = first(e)
        assertTrue(pre.key.startsWith("pre:"))
        assertTrue(pre.speech!!.contains("30分"))
        assertTrue(pre.speech!!.endsWith("ファイト！ファイト！"))
        assertFalse(pre.speech!!.contains("おかえり"))
        val main = first(e, dt(6, 45))
        assertTrue(main.key.startsWith("alarm:"))
        assertTrue(main.speech!!.endsWith("おかえりなさいませ、ご主人様！"))
        assertFalse(main.speech!!.contains("ファイト"))
    }

    @Test fun harmonyFlagPropagates() {
        assertTrue(first(entry(harmony = true, phrases = setOf("okaeri"))).harmony)
        assertFalse(first(entry(phrases = setOf("okaeri"))).harmony)
    }

    @Test fun calendarPhrasesAndHarmony() {
        val ev = CalendarEvent("e", "会議", dt(10))
        val s = CalendarSettings(
            enabled = true, phrases = setOf("perfect"), prePhrases = setOf("excellent"), harmony = true,
        )
        val pre = Planner.next(emptyList(), listOf(ev), s, dt(8), NoHolidays).first()
        assertEquals(dt(9, 30), pre.time)
        assertTrue(pre.speech!!.endsWith("エクセレント！") && pre.harmony)
        val main = Planner.next(emptyList(), listOf(ev), s, dt(9, 30), NoHolidays).first()
        assertTrue(main.speech!!.contains("会議") && main.speech!!.endsWith("パーフェクト！"))
    }

    @Test fun codecRoundTripAndBackwardCompat() {
        val e = entry(phrases = setOf("okaeri", "fight"), prePhrases = setOf("oishiku"), harmony = true, pre = 30)
        assertEquals(listOf(e), Codec.decodeAll(Codec.encodeAll(listOf(e))))
        // 旧バージョンの保存行(ph/pph/harm無し)も読める
        val legacy = Codec.encode(entry()).split("&").filterNot { it.startsWith("ph=") || it.startsWith("pph=") || it.startsWith("harm=") }.joinToString("&")
        val d = Codec.decode(legacy)
        assertTrue(d.phrases.isEmpty() && d.prePhrases.isEmpty() && !d.harmony)
        // 未知のidは捨てる
        val bad = Codec.encode(e).replace("ph=okaeri%2Cfight", "ph=okaeri%2Cbogus")
        assertEquals(setOf("okaeri"), Codec.decode(bad).phrases)
    }
}
