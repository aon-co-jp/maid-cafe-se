package tokyo.runo.maidcafese.core.audio

import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.cos
import kotlin.math.sin
import kotlin.math.sqrt
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import tokyo.runo.maidcafese.core.AlarmEntry
import tokyo.runo.maidcafese.core.AlarmKind
import tokyo.runo.maidcafese.core.CalendarSettings
import tokyo.runo.maidcafese.core.MaidPhrases
import tokyo.runo.maidcafese.core.NoHolidays
import tokyo.runo.maidcafese.core.Planner
import tokyo.runo.maidcafese.core.Recurrence
import tokyo.runo.maidcafese.core.Schedule
import tokyo.runo.maidcafese.core.SpeechText
import tokyo.runo.maidcafese.core.VoiceStyle
import java.time.LocalDateTime
import java.time.LocalTime

class PolishTest {
    private val sr = 22050
    private fun tone(f: Double, sec: Double, amp: Float = 0.5f) =
        FloatArray((sr * sec).toInt()) { sin(2 * PI * f * it / sr).toFloat() * amp }

    private fun rms(a: FloatArray) = sqrt(a.sumOf { (it * it).toDouble() } / a.size)

    private fun power(x: FloatArray, f: Double): Double {
        val w = 2 * PI * f / sr
        val c = 2 * cos(w)
        var s1 = 0.0; var s2 = 0.0
        for (v in x) { val s = v + c * s1 - s2; s2 = s1; s1 = s }
        return (s1 * s1 + s2 * s2 - c * s1 * s2) / (x.size.toDouble() * x.size)
    }

    @Test fun trimSilenceRemovesLeadingAndTrailingSilenceOnly() {
        val body = tone(300.0, 0.5)
        val x = FloatArray(sr / 2) + body + FloatArray(sr / 2)
        val y = VoiceDsp.trimSilence(x, sr)
        val expected = body.size + 2 * (sr * 40 / 1000)
        assertTrue(abs(y.size - expected) < sr / 50, "size=${y.size} expected≈$expected")
        assertTrue(y.size < x.size / 2)
    }

    @Test fun trimSilenceKeepsInnerPauseAndAllSilence() {
        val x = tone(300.0, 0.2) + FloatArray(sr / 2) + tone(300.0, 0.2)
        assertEquals(x.size, VoiceDsp.trimSilence(x, sr).size)
        val silent = FloatArray(1000)
        assertEquals(1000, VoiceDsp.trimSilence(silent, sr).size)
    }

    @Test fun loudnessNormalizationEqualizesShortAndLongAndLimits() {
        val quiet = tone(250.0, 0.4, 0.05f)
        val loud = tone(250.0, 1.5, 0.9f)
        val a = VoiceDsp.normalizeLoudness(quiet)
        val b = VoiceDsp.normalizeLoudness(loud)
        assertTrue(abs(rms(a) - rms(b)) < 0.02, "rms ${rms(a)} vs ${rms(b)}")
        // スパイク混じりでも0.9を超えない(ソフトリミッター)
        val spiky = tone(250.0, 1.0, 0.05f).also { it[100] = 1f; it[5000] = -1f }
        assertTrue(VoiceDsp.normalizeLoudness(spiky).maxOf { abs(it) } <= 0.9f)
        assertEquals(0f, VoiceDsp.normalizeLoudness(FloatArray(100)).maxOf { abs(it) })
    }

    @Test fun joinInsertsGaps() {
        val p1 = FloatArray(sr) { 0.5f }
        val p2 = FloatArray(sr / 2) { 0.25f }
        val out = VoiceDsp.join(listOf(p1, p2), listOf(500, 0), sr)
        assertEquals(sr + sr / 2 + sr / 2, out.size)
        assertEquals(0.5f, out[sr - 1])
        assertEquals(0f, out[sr + 10]) // 間は無音
        assertEquals(0.25f, out[sr + sr / 2 + 5])
        assertEquals(0, VoiceDsp.join(emptyList(), emptyList(), sr).size)
    }

    @Test fun pitchMulShiftsTheSegment() {
        val x = tone(250.0, 2.0)
        val base = VoiceDsp.render(Pcm(x, sr), VoiceStyle.MAID, SourceGender.FEMALE, false)
        val up = VoiceDsp.render(Pcm(x, sr), VoiceStyle.MAID, SourceGender.FEMALE, false, pitchMul = 1.06)
        val mid = { a: Pcm -> a.samples.copyOfRange(sr / 2, sr * 3 / 2) }
        val f = 250 * 1.12
        assertTrue(power(mid(base), f) > 10 * power(mid(base), f * 1.06))
        assertTrue(power(mid(up), f * 1.06) > 10 * power(mid(up), f))
    }

    @Test fun segmentsCarryPerPhraseProsodyAndGaps() {
        val segs = SpeechText.segments("ご主人様、お時間です", linkedSetOf("okaeri", "fight"))
        // 本文 → おかえり → ファイト(選んだ順)
        assertEquals(3, segs.size)
        assertEquals("ご主人様、お時間です", segs[0].text)
        assertEquals(300, segs[0].gapAfterMs)
        val okaeri = MaidPhrases.all.first { it.id == "okaeri" }
        assertEquals(okaeri.spoken, segs[1].text)
        assertEquals(okaeri.gapMs, segs[1].gapAfterMs)
        assertEquals(okaeri.rate, segs[1].rate)
        val fight = MaidPhrases.all.first { it.id == "fight" }
        assertEquals(fight.pitch, segs[2].pitch)
        assertEquals(0, segs[2].gapAfterMs) // 最後の後ろには間を置かない
    }

    @Test fun segmentsWithoutPhrasesOrBase() {
        assertEquals(1, SpeechText.segments("本文のみ", emptySet()).size)
        assertEquals(0, SpeechText.segments("本文のみ", emptySet()).first().gapAfterMs)
        assertTrue(SpeechText.segments(null, emptySet()).isEmpty())
        assertEquals(1, SpeechText.segments(null, setOf("perfect")).size)
    }

    @Test fun occurrenceSegmentsJoinToSpeech() {
        val e = AlarmEntry(
            "a", "起床", Schedule(Recurrence.Daily, LocalTime.of(7, 0)), AlarmKind.SPEECH,
            text = "薬", phrases = setOf("okaeri", "perfect"),
        )
        val o = Planner.next(listOf(e), emptyList(), CalendarSettings(), LocalDateTime.of(2026, 9, 30, 0, 0), NoHolidays).first()
        assertEquals(o.speech, o.segments.joinToString(" ") { it.text })
        assertEquals(3, o.segments.size)
    }

    @Test fun soundOnlyOccurrenceHasNoSegments() {
        val e = AlarmEntry("a", "起床", Schedule(Recurrence.Daily, LocalTime.of(7, 0)), AlarmKind.SOUND)
        val o = Planner.next(listOf(e), emptyList(), CalendarSettings(), LocalDateTime.of(2026, 9, 30, 0, 0), NoHolidays).first()
        assertTrue(o.segments.isEmpty())
    }
}
