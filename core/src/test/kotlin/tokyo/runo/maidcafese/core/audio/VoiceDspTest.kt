package tokyo.runo.maidcafese.core.audio

import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.cos
import kotlin.math.sin
import kotlin.math.sqrt
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue
import tokyo.runo.maidcafese.core.VoiceStyle

class VoiceDspTest {
    private val sr = 22050

    /** 基本周波数[f0]の倍音を持つ疑似音声(振幅ゆらぎ付き)。 */
    private fun voiced(f0: Double, seconds: Double = 1.5): FloatArray = FloatArray((sr * seconds).toInt()) { i ->
        val t = i.toDouble() / sr
        val env = 0.6 + 0.4 * sin(2 * PI * 3 * t)
        var v = 0.0
        for (h in 1..5) v += sin(2 * PI * f0 * h * t) / h
        (env * v * 0.3).toFloat()
    }

    /** Goertzel法で周波数[f]の成分強度。 */
    private fun power(x: FloatArray, f: Double): Double {
        val w = 2 * PI * f / sr
        val c = 2 * cos(w)
        var s1 = 0.0; var s2 = 0.0
        for (v in x) { val s = v + c * s1 - s2; s2 = s1; s1 = s }
        return (s1 * s1 + s2 * s2 - c * s1 * s2) / (x.size.toDouble() * x.size)
    }

    private fun peakOf(x: FloatArray) = x.maxOf { abs(it) }

    @Test fun pitchShiftUpMovesEnergyAndKeepsLength() {
        val x = FloatArray(sr * 2) { sin(2 * PI * 200 * it / sr).toFloat() * 0.5f }
        val y = VoiceDsp.pitchShift(x, sr, 1.5)
        assertEquals(x.size, y.size)
        val mid = y.copyOfRange(sr / 2, sr * 3 / 2)
        assertTrue(power(mid, 300.0) > 20 * power(mid, 200.0), "300Hzが主成分になる")
    }

    @Test fun pitchShiftDownMovesEnergy() {
        val x = FloatArray(sr * 2) { sin(2 * PI * 300 * it / sr).toFloat() * 0.5f }
        val y = VoiceDsp.pitchShift(x, sr, 0.8)
        assertEquals(x.size, y.size)
        val mid = y.copyOfRange(sr / 2, sr * 3 / 2)
        assertTrue(power(mid, 240.0) > 20 * power(mid, 300.0), "240Hzが主成分になる")
    }

    @Test fun pitchShiftKeepsLoudnessRoughly() {
        val x = voiced(180.0)
        val y = VoiceDsp.pitchShift(x, sr, 1.2)
        fun rms(a: FloatArray) = sqrt(a.sumOf { (it * it).toDouble() } / a.size)
        val ratio = rms(y) / rms(x)
        assertTrue(ratio in 0.7..1.3, "rms比=$ratio")
    }

    @Test fun ratioNearOneIsIdentity() {
        val x = voiced(200.0)
        assertTrue(VoiceDsp.pitchShift(x, sr, 1.001).contentEquals(x))
    }

    @Test fun shortInputDoesNotCrash() {
        val y = VoiceDsp.pitchShift(FloatArray(100) { 0.1f }, sr, 1.3)
        assertEquals(100, y.size)
        assertEquals(0, VoiceDsp.pitchShift(FloatArray(0), sr, 1.3).size)
    }

    @Test fun deepMaleFromFemaleSourceLowersPitch() {
        val x = FloatArray(sr * 2) { sin(2 * PI * 250 * it / sr).toFloat() * 0.5f }
        val out = VoiceDsp.render(Pcm(x, sr), VoiceStyle.DEEP_MALE, SourceGender.FEMALE, harmony = false)
        val mid = out.samples.copyOfRange(sr / 2, sr * 3 / 2)
        assertTrue(power(mid, 250 * 0.72) > 10 * power(mid, 250.0))
    }

    @Test fun maidRaisesPitch() {
        val x = FloatArray(sr * 2) { sin(2 * PI * 250 * it / sr).toFloat() * 0.5f }
        val out = VoiceDsp.render(Pcm(x, sr), VoiceStyle.MAID, SourceGender.FEMALE, harmony = false)
        val mid = out.samples.copyOfRange(sr / 2, sr * 3 / 2)
        assertTrue(power(mid, 250 * 1.12) > 10 * power(mid, 250.0))
    }

    @Test fun harmonyContainsBothVoicesAMajorThirdApart() {
        val x = FloatArray(sr * 2) { sin(2 * PI * 250 * it / sr).toFloat() * 0.5f }
        val single = VoiceDsp.render(Pcm(x, sr), VoiceStyle.MAID, SourceGender.FEMALE, harmony = false)
        val duo = VoiceDsp.render(Pcm(x, sr), VoiceStyle.MAID, SourceGender.FEMALE, harmony = true)
        val fa = 250 * 1.12
        val fb = fa * VoiceDsp.MAJOR_THIRD
        val s = single.samples.copyOfRange(sr / 2, sr * 3 / 2)
        val d = duo.samples.copyOfRange(sr / 2, sr * 3 / 2)
        assertTrue(power(s, fb) < 0.01 * power(s, fa), "単独には3度上の成分が無い")
        assertTrue(power(d, fa) > 10 * power(d, 700.0) && power(d, fb) > 10 * power(d, 700.0), "ハモりは両方の音を含む")
        assertTrue(power(d, fb) > 0.2 * power(d, fa))
        assertTrue(duo.samples.size >= single.samples.size)
    }

    @Test fun outputIsNormalizedWithoutClipping() {
        val loud = FloatArray(sr) { sin(2 * PI * 200 * it / sr).toFloat() }
        for (h in listOf(false, true)) for (style in VoiceStyle.entries) {
            val o = VoiceDsp.render(Pcm(loud, sr), style, SourceGender.UNKNOWN, h)
            assertTrue(peakOf(o.samples) <= 0.91f, "$style harmony=$h peak=${peakOf(o.samples)}")
            assertTrue(peakOf(o.samples) > 0.1f)
        }
    }

    @Test fun silenceStaysSilent() {
        val o = VoiceDsp.render(Pcm(FloatArray(sr), sr), VoiceStyle.MAID, SourceGender.FEMALE, true)
        assertEquals(0f, peakOf(o.samples))
    }

    @Test fun wavRoundTrip() {
        val x = voiced(200.0, 0.3)
        val back = Wav.parse(Wav.toBytes(Pcm(x, sr)))
        assertNotNull(back)
        assertEquals(sr, back.sampleRate)
        assertEquals(x.size, back.samples.size)
        for (i in x.indices step 97) assertEquals(x[i], back.samples[i], 1e-3f)
    }

    @Test fun wavRejectsGarbageAndUnsupported() {
        assertNull(Wav.parse(ByteArray(10)))
        assertNull(Wav.parse(ByteArray(100)))
        val b = Wav.toBytes(Pcm(FloatArray(100), sr))
        b[34] = 8 // 8bitは非対応
        assertNull(Wav.parse(b))
    }

    @Test fun wavStereoIsAveragedAndStreamingSizeTolerated() {
        // 手組みのステレオ16bit(L=+0.5, R=-0.5 → 平均0)、dataサイズ0(ストリーミング出力想定)
        val frames = 50
        val bb = java.nio.ByteBuffer.allocate(44 + frames * 4).order(java.nio.ByteOrder.LITTLE_ENDIAN)
        bb.put("RIFF".toByteArray()).putInt(0).put("WAVE".toByteArray()).put("fmt ".toByteArray()).putInt(16)
            .putShort(1).putShort(2).putInt(sr).putInt(sr * 4).putShort(4).putShort(16)
            .put("data".toByteArray()).putInt(0)
        repeat(frames) { bb.putShort(16384).putShort(-16384) }
        val p = Wav.parse(bb.array())
        assertNotNull(p)
        assertEquals(frames, p.samples.size)
        assertTrue(p.samples.all { abs(it) < 1e-4f })
    }
}
