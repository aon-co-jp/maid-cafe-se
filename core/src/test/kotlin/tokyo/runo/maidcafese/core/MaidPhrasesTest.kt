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
        Planner.next(listOf(e), emptyList(), CalendarSettings(), after).first()

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

    @Test fun multiplePhrasesCombinedInSelectionOrder() {
        // カタログ順(おかえり→…→パーフェクト)ではなく、選んだ順(perfect→okaeri→fight)で連結される
        val speech = first(entry(phrases = linkedSetOf("perfect", "okaeri", "fight"))).speech!!
        val i1 = speech.indexOf("パーフェクト！")
        val i2 = speech.indexOf("おかえりなさいませ")
        val i3 = speech.indexOf("ファイト！ファイト！")
        assertTrue(i1 in 0 until i2 && i2 < i3, speech)
    }

    @Test fun okaeriThenWakeUpAndWakeUpThenOkaeriBothWork() {
        val a = first(entry(text = "", phrases = linkedSetOf("okaeri", "okite"))).speech!!
        assertTrue(a.startsWith("おかえりなさいませ、ご主人様！"), a)
        assertTrue(a.indexOf("おーきーてー") > a.indexOf("おかえりなさいませ"))
        val b = first(entry(text = "", phrases = linkedSetOf("okite", "okaeri"))).speech!!
        assertTrue(b.startsWith("ご主人さまー、おーきーてー。今日も、がんばってー"), b)
        assertTrue(b.indexOf("おかえりなさいませ") > b.indexOf("おーきーてー"))
    }

    @Test fun wakeUpPhraseAloneIsSlow() {
        val seg = first(entry(text = "", phrases = setOf("okite"))).segments
        assertTrue(seg.size >= 2, "メイドの声は文節ごとに抑揚を付けるため、割れる")
        assertTrue(seg.all { it.rate <= 0.8f }) // どの文節もゆっくり間延びさせて読む
        assertTrue(seg.map { it.pitch }.toSet().size > 1, "音程が文節ごとに変わる")
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
        val pre = Planner.next(emptyList(), listOf(ev), s, dt(8)).first()
        assertEquals(dt(9, 30), pre.time)
        assertTrue(pre.speech!!.endsWith("エクセレント！") && pre.harmony)
        val main = Planner.next(emptyList(), listOf(ev), s, dt(9, 30)).first()
        assertTrue(main.speech!!.contains("会議") && main.speech!!.endsWith("パーフェクト！"))
    }

    private fun nums(vararg p: Pair<String, Int>) = linkedMapOf(*p)

    @Test fun ranksAndOrdered() {
        val ids = linkedSetOf("okite", "okaeri", "fight")
        assertEquals(mapOf("okite" to 1, "okaeri" to 2, "fight" to 3), MaidPhrases.ranks(ids))
        assertEquals(
            listOf("fight", "okite", "okaeri"),
            MaidPhrases.ordered(nums("okite" to 2, "okaeri" to 5, "fight" to 1)).toList(),
        )
    }

    @Test fun addGoesToTheEndAndRemoveLeavesAGap() {
        var n: Map<String, Int> = emptyMap()
        n = MaidPhrases.add(n, "okaeri")
        assertEquals(mapOf("okaeri" to 1), n)
        n = MaidPhrases.add(n, "okite")
        n = MaidPhrases.add(n, "fight")
        assertEquals(mapOf("okaeri" to 1, "okite" to 2, "fight" to 3), n)
        n = MaidPhrases.remove(n, "okite")
        assertEquals(mapOf("okaeri" to 1, "fight" to 3), n) // 欠番を詰めない
        n = MaidPhrases.add(n, "perfect")
        assertEquals(4, n["perfect"]) // 最大番号の次
        assertEquals(n, MaidPhrases.add(n, "perfect")) // 二重に加えない
        assertEquals(n, MaidPhrases.remove(n, "excellent"))
    }

    @Test fun duplicateNumberDisplacedHolderGetsSmallestFreeNumber() {
        // 2件: 2番目に1を入れる → 元の1番(おかえり)は空いている2番になる
        val two = MaidPhrases.assign(nums("okaeri" to 1, "okite" to 2), "okite", 1)
        assertEquals(mapOf("okite" to 1, "okaeri" to 2), two)
        assertEquals(listOf("okite", "okaeri"), MaidPhrases.ordered(two).toList())
        // 3件: 3番目に1を入れる → 元の1番は、空いた3番へ
        val three = MaidPhrases.assign(nums("okaeri" to 1, "okite" to 2, "fight" to 3), "fight", 1)
        assertEquals(mapOf("fight" to 1, "okite" to 2, "okaeri" to 3), three)
        // 3件: 3番目に2を入れる → 元の2番は、空いた3番へ(1は使われている)
        val mid = MaidPhrases.assign(nums("okaeri" to 1, "okite" to 2, "fight" to 3), "fight", 2)
        assertEquals(mapOf("okaeri" to 1, "okite" to 3, "fight" to 2), mid)
        // 空いている最小の番号(欠番の1)へ回る
        val gap = MaidPhrases.assign(nums("okite" to 2, "fight" to 3), "fight", 2)
        assertEquals(mapOf("okite" to 1, "fight" to 2), gap)
    }

    @Test fun assigningAFreeNumberJustMoves() {
        assertEquals(mapOf("okaeri" to 1, "okite" to 5), MaidPhrases.assign(nums("okaeri" to 1, "okite" to 2), "okite", 5))
        // 自分の番号と同じ・0や負・未選択idは変化なし
        val base = nums("okaeri" to 1, "okite" to 2)
        assertEquals(base, MaidPhrases.assign(base, "okite", 2))
        assertEquals(base, MaidPhrases.assign(base, "okite", 0))
        assertEquals(base, MaidPhrases.assign(base, "perfect", 1))
    }

    @Test fun neverProducesDuplicateNumbers() {
        val ids = listOf("okite", "okaeri", "oishiku", "meh", "fight", "excellent", "perfect")
        var n: Map<String, Int> = emptyMap()
        for (id in ids) n = MaidPhrases.add(n, id)
        val rnd = java.util.Random(7)
        repeat(500) {
            n = MaidPhrases.assign(n, ids[rnd.nextInt(ids.size)], 1 + rnd.nextInt(9))
            assertEquals(n.size, n.values.toSet().size, "重複あり: $n")
            assertEquals(ids.size, MaidPhrases.ordered(n).size)
        }
    }

    @Test fun editedNumbersDecideTheSpokenOrder() {
        val n = MaidPhrases.assign(nums("okaeri" to 1, "okite" to 2), "okite", 1)
        val speech = first(entry(text = "", phrases = MaidPhrases.ordered(n))).speech!!
        assertTrue(speech.startsWith("ご主人さまー、おーきーてー"), speech)
        assertTrue(speech.indexOf("おかえりなさいませ") > speech.indexOf("おーきーてー"))
    }

    @Test fun selectionOrderSurvivesCodec() {
        val e = entry(phrases = linkedSetOf("perfect", "okite", "okaeri"), prePhrases = linkedSetOf("fight", "excellent"))
        val back = Codec.decodeAll(Codec.encodeAll(listOf(e))).single()
        assertEquals(listOf("perfect", "okite", "okaeri"), back.phrases.toList())
        assertEquals(listOf("fight", "excellent"), back.prePhrases.toList())
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
