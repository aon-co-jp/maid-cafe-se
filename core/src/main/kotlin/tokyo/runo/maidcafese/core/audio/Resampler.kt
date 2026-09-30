package tokyo.runo.maidcafese.core.audio

import kotlin.math.PI
import kotlin.math.max
import kotlin.math.min
import kotlin.math.roundToInt
import kotlin.math.sin
import kotlin.math.sqrt

/**
 * 高品質リサンプラ。make-disk(`engine/audio_sr.rs`の`resample_poly`、scipy互換で精度検証済み)の
 * 設計をKotlinへ移植: Kaiser窓(β=5)のFIR、半長は10×max(up,down)、多相実装。
 * 線形補間と違い、間引き時の折り返し雑音・補間時のイメージングが出ない。
 */
object Resampler {
    private fun gcd(a: Int, b: Int): Int = if (b == 0) a else gcd(b, a % b)

    private fun besselI0(x: Double): Double {
        var sum = 1.0
        var term = 1.0
        var k = 1.0
        while (term > 1e-12 * sum) {
            term *= (x / (2 * k)) * (x / (2 * k))
            sum += term
            k += 1.0
        }
        return sum
    }

    /** 出力長は ceil(n*up/down)。`up==down`ならコピー。 */
    fun resamplePoly(x: FloatArray, upIn: Int, downIn: Int): FloatArray {
        require(upIn > 0 && downIn > 0)
        if (upIn == downIn || x.isEmpty()) return x.copyOf()
        val g = gcd(upIn, downIn)
        val up = upIn / g
        val down = downIn / g
        val half = 10 * max(up, down)
        val len = 2 * half + 1
        val cutoff = 1.0 / max(up, down)
        val beta = 5.0
        val i0b = besselI0(beta)
        val hd = DoubleArray(len) { n ->
            val t = n.toDouble() - half
            val sinc = if (t == 0.0) 1.0 else sin(PI * cutoff * t) / (PI * cutoff * t)
            val r = 2.0 * n / (len - 1) - 1.0
            cutoff * sinc * besselI0(beta * sqrt(max(1.0 - r * r, 0.0))) / i0b
        }
        val sum = hd.sum()
        val h = FloatArray(len) { (hd[it] * up / sum).toFloat() }

        val outLen = ((x.size.toLong() * up + down - 1) / down).toInt()
        val y = FloatArray(outLen)
        for (m in 0 until outLen) {
            val base = m.toLong() * down + half
            val kMin = max(0L, (base - 2 * half + up - 1) / up).toInt()
            val kMax = min(base / up, (x.size - 1).toLong()).toInt()
            var acc = 0f
            for (k in kMin..kMax) acc += x[k] * h[(base - k.toLong() * up).toInt()]
            y[m] = acc
        }
        return y
    }

    /** 再生速度を[ratio]倍にした結果(長さ1/ratio・音程ratio倍)。比率は1/200刻みに丸める(3度の誤差は4セント以内)。 */
    fun speedUp(x: FloatArray, ratio: Double): FloatArray {
        val denom = 200
        val down = (ratio * denom).roundToInt().coerceAtLeast(1)
        return resamplePoly(x, denom, down)
    }
}
