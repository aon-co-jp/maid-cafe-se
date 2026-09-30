package tokyo.runo.maidcafese.core.audio

import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.sin
import kotlin.math.sqrt
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class ResamplerTest {
    private fun tone(f: Double, sr: Int, n: Int) = FloatArray(n) { sin(2 * PI * f * it / sr).toFloat() * 0.5f }
    private fun rms(a: FloatArray, from: Int, to: Int) = sqrt((from until to).sumOf { (a[it] * a[it]).toDouble() } / (to - from))

    @Test fun lengthIsCeilOfRatio() {
        assertEquals(1000, Resampler.resamplePoly(FloatArray(500), 2, 1).size)
        assertEquals(334, Resampler.resamplePoly(FloatArray(1000), 1, 3).size)
        assertEquals(10, Resampler.resamplePoly(FloatArray(10), 5, 5).size)
    }

    @Test fun toneIsPreservedWithHighAccuracyWhenUpsampling() {
        // 1kHz@16k を 2倍(32k)に: 内部で元と同じ正弦になるはず(端の過渡部分は除く)
        val x = tone(1000.0, 16000, 4000)
        val y = Resampler.resamplePoly(x, 2, 1)
        val ref = tone(1000.0, 32000, y.size)
        var maxErr = 0f
        for (i in 800 until y.size - 800) maxErr = maxOf(maxErr, abs(y[i] - ref[i]))
        assertTrue(maxErr < 2e-3f, "maxErr=$maxErr")
    }

    @Test fun aboveNyquistContentIsRejectedNotAliased() {
        // 22.05kHz系の10kHzを1.5倍速にすると15kHz(>ナイキスト11.025k)。線形補間なら折り返して残るが、ここでは消える。
        // (フィルタの遮断は入力側で約7.35kHz、遷移帯約2kHz幅なので、10kHzは完全に阻止帯)
        val sr = 22050
        val y = Resampler.speedUp(tone(10000.0, sr, sr), 1.5)
        assertTrue(rms(y, y.size / 4, y.size * 3 / 4) < 0.02, "折り返し雑音が残っている")
        // 通過帯域(2kHz→3kHz)は保たれる
        val z = Resampler.speedUp(tone(2000.0, sr, sr), 1.5)
        assertTrue(rms(z, z.size / 4, z.size * 3 / 4) > 0.3)
    }

    @Test fun speedUpChangesPitchAndLength() {
        val sr = 22050
        val x = tone(200.0, sr, sr * 2)
        val y = Resampler.speedUp(x, 1.25)
        assertEquals(x.size / 1.25, y.size.toDouble(), 3.0)
    }

    @Test fun ditherIsSmallAndDeterministic() {
        val x = FloatArray(1000) { 0.25f }
        val a = Wav.toPcm16(x)
        val b = Wav.toPcm16(x)
        assertTrue(a.contentEquals(b))
        val expect = (0.25f * 32767f).toInt()
        assertTrue(a.all { abs(it - expect) <= 2 })
        assertTrue(a.toSet().size > 1, "ディザで量子化値がばらつく")
    }
}
