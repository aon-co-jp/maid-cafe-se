package tokyo.runo.maidcafese.core.audio

import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.cos
import kotlin.math.max
import kotlin.math.min
import kotlin.math.pow
import kotlin.math.sin
import kotlin.math.sqrt
import tokyo.runo.maidcafese.core.VoiceStyle

/** TTSエンジンが出した声の性別(端末の音声名からの推定)。不明は女性扱い(多くの端末の既定が女性声のため)。 */
enum class SourceGender { FEMALE, MALE, UNKNOWN }

/**
 * TTS出力の後処理。エンジンの`setPitch`に頼らず、ピッチと声の太さ(フォルマント)を自前で変える。
 *  - 高品質リサンプラ([Resampler]、make-disk移植)で音程とフォルマントを同時に動かし(=声道の大きさが変わった声になる)、
 *    WSOLAで元の長さに戻す。
 *  - 「ハモり」は同じ音声から音程違い(長3度上)の声を作って重ねるため、2人のタイミングが完全に揃う。
 */
object VoiceDsp {
    /** 声ごとの変換レシピ。[pitchRatio]は音程(と声の太さ)の倍率。 */
    data class Recipe(val pitchRatio: Double, val lowShelfDb: Double, val highShelfDb: Double)

    fun recipe(style: VoiceStyle, source: SourceGender): Recipe = when (style) {
        VoiceStyle.MAID ->
            if (source == SourceGender.MALE) Recipe(1.45, 0.0, 3.0) else Recipe(1.12, 0.0, 3.0)
        VoiceStyle.DEEP_MALE ->
            if (source == SourceGender.MALE) Recipe(0.86, 4.0, -1.0) else Recipe(0.72, 5.0, -2.0)
    }

    /** 平均律の長3度(4半音)。 */
    val MAJOR_THIRD = 2.0.pow(4.0 / 12.0)

    /** [pitchMul]はセリフごとの抑揚(音程への追加倍率)。 */
    fun render(pcm: Pcm, style: VoiceStyle, source: SourceGender, harmony: Boolean, pitchMul: Double = 1.0): Pcm {
        val r = recipe(style, source).let { it.copy(pitchRatio = it.pitchRatio * pitchMul) }
        val sr = pcm.sampleRate
        var x = highPass(pcm.samples, sr, 70.0)
        fun voice(ratio: Double): FloatArray {
            var v = pitchShift(x, sr, ratio)
            if (r.lowShelfDb != 0.0) v = lowShelf(v, sr, 180.0, r.lowShelfDb)
            if (r.highShelfDb != 0.0) v = highShelf(v, sr, 4000.0, r.highShelfDb)
            return v
        }
        val a = voice(r.pitchRatio)
        val out = if (harmony) {
            val b = voice(r.pitchRatio * MAJOR_THIRD)
            val delay = (sr * 0.018).toInt() // 18msずらして「別の2人」の厚みを出す
            val mixed = FloatArray(max(a.size, b.size + delay))
            for (i in a.indices) mixed[i] += a[i] * 0.75f
            for (i in b.indices) mixed[i + delay] += b[i] * 0.6f
            mixed
        } else a
        return Pcm(fade(normalizeLoudness(trimSilence(out, sr)), sr), sr)
    }

    /** 長さを保ったまま音程(とフォルマント)を[ratio]倍にする。 */
    fun pitchShift(x: FloatArray, sr: Int, ratio: Double): FloatArray {
        if (abs(ratio - 1.0) < 0.005 || x.isEmpty()) return x.copyOf()
        val y = Resampler.speedUp(x, ratio)
        return wsola(y, (x.size.toDouble() / y.size), sr, targetLength = x.size)
    }

    /** WSOLAによるタイムストレッチ(出力長 = 入力長 x [alpha])。音程は変えない。 */
    fun wsola(x: FloatArray, alpha: Double, sr: Int, targetLength: Int = (x.size * alpha).toInt()): FloatArray {
        val n = ((sr * 0.030).toInt() / 2 * 2).coerceAtLeast(128)
        val hs = n / 2
        val tol = (sr * 0.010).toInt()
        val outLen = targetLength
        val out = FloatArray(outLen)
        if (x.size < n * 2 || outLen <= 0) {
            System.arraycopy(x, 0, out, 0, min(x.size, outLen))
            return out
        }
        val win = FloatArray(n) { (0.5 - 0.5 * cos(2.0 * PI * it / n)).toFloat() }
        val acc = FloatArray(outLen + n)
        val norm = FloatArray(outLen + n)
        fun add(pos: Int, outPos: Int) {
            for (i in 0 until n) {
                acc[outPos + i] += x[pos + i] * win[i]
                norm[outPos + i] += win[i]
            }
        }
        var prev = 0
        add(0, 0)
        var k = 1
        while (k * hs < outLen) {
            val nominal = (k * hs / alpha).toInt()
            val lo = max(0, nominal - tol)
            val hi = min(x.size - n, nominal + tol)
            if (lo > hi) break
            val tmpl = prev + hs
            var best = min(max(nominal, lo), hi)
            if (tmpl + n <= x.size) {
                var bestScore = Double.NEGATIVE_INFINITY
                for (c in lo..hi) {
                    var dot = 0.0
                    var e = 1e-9
                    var i = 0
                    while (i < n) { // 4サンプル間引きで相関を計算(速度優先、位相合わせには十分)
                        val v = x[c + i].toDouble()
                        dot += v * x[tmpl + i]
                        e += v * v
                        i += 4
                    }
                    val score = dot / sqrt(e)
                    if (score > bestScore) { bestScore = score; best = c }
                }
            }
            add(best, k * hs)
            prev = best
            k++
        }
        for (i in 0 until outLen) out[i] = if (norm[i] > 1e-3f) acc[i] / norm[i] else 0f
        return out
    }

    private class Biquad(b0: Double, b1: Double, b2: Double, a0: Double, a1: Double, a2: Double) {
        private val nb0 = (b0 / a0).toFloat(); private val nb1 = (b1 / a0).toFloat(); private val nb2 = (b2 / a0).toFloat()
        private val na1 = (a1 / a0).toFloat(); private val na2 = (a2 / a0).toFloat()
        fun process(x: FloatArray): FloatArray {
            val y = FloatArray(x.size)
            var x1 = 0f; var x2 = 0f; var y1 = 0f; var y2 = 0f
            for (i in x.indices) {
                val v = nb0 * x[i] + nb1 * x1 + nb2 * x2 - na1 * y1 - na2 * y2
                x2 = x1; x1 = x[i]; y2 = y1; y1 = v
                y[i] = v
            }
            return y
        }
    }

    fun highPass(x: FloatArray, sr: Int, freq: Double): FloatArray {
        val w = 2 * PI * freq / sr
        val alpha = sin(w) / (2 * 0.7071)
        val c = cos(w)
        return Biquad((1 + c) / 2, -(1 + c), (1 + c) / 2, 1 + alpha, -2 * c, 1 - alpha).process(x)
    }

    fun lowShelf(x: FloatArray, sr: Int, freq: Double, gainDb: Double): FloatArray {
        val a = 10.0.pow(gainDb / 40); val w = 2 * PI * freq / sr; val c = cos(w)
        val al = sin(w) / 2 * sqrt(2.0); val sa = 2 * sqrt(a) * al
        return Biquad(
            a * ((a + 1) - (a - 1) * c + sa), 2 * a * ((a - 1) - (a + 1) * c), a * ((a + 1) - (a - 1) * c - sa),
            (a + 1) + (a - 1) * c + sa, -2 * ((a - 1) + (a + 1) * c), (a + 1) + (a - 1) * c - sa,
        ).process(x)
    }

    fun highShelf(x: FloatArray, sr: Int, freq: Double, gainDb: Double): FloatArray {
        val a = 10.0.pow(gainDb / 40); val w = 2 * PI * freq / sr; val c = cos(w)
        val al = sin(w) / 2 * sqrt(2.0); val sa = 2 * sqrt(a) * al
        return Biquad(
            a * ((a + 1) + (a - 1) * c + sa), -2 * a * ((a - 1) + (a + 1) * c), a * ((a + 1) + (a - 1) * c - sa),
            (a + 1) - (a - 1) * c + sa, 2 * ((a - 1) - (a + 1) * c), (a + 1) - (a - 1) * c - sa,
        ).process(x)
    }

    /**
     * 前後の無音を削る(TTSエンジンは前後に数百msの無音を付けることがあり、間の長さを制御できなくなるため)。
     * 10ms窓のRMSが[thresholdDb]を超えた最初/最後の位置から[keepMs]だけ残す。全体が無音ならそのまま返す。
     */
    fun trimSilence(x: FloatArray, sr: Int, thresholdDb: Double = -50.0, keepMs: Int = 40): FloatArray {
        val win = max(1, sr / 100)
        val thr = 10.0.pow(thresholdDb / 20)
        fun loud(from: Int): Boolean {
            var e = 0.0
            val to = min(x.size, from + win)
            for (i in from until to) e += x[i].toDouble() * x[i]
            return sqrt(e / (to - from)) > thr
        }
        var first = -1
        var i = 0
        while (i < x.size) { if (loud(i)) { first = i; break }; i += win }
        if (first < 0) return x
        var last = first
        i = (x.size - 1) / win * win
        while (i >= first) { if (loud(i)) { last = min(x.size, i + win); break }; i -= win }
        val keep = sr * keepMs / 1000
        return x.copyOfRange(max(0, first - keep), min(x.size, last + keep))
    }

    /**
     * 声の大きさを揃える: RMSを[targetRms]に合わせ、[KNEE]を超える部分はtanhでなだらかに丸めて[CEILING]以内に収める
     * (ピーク基準だと短い言葉と長い文で聴こえの大きさがばらつくため)。無音はそのまま。
     */
    fun normalizeLoudness(x: FloatArray, targetRms: Float = 0.18f): FloatArray {
        var e = 0.0
        for (v in x) e += v.toDouble() * v
        val rms = sqrt(e / max(1, x.size)).toFloat()
        if (rms < 1e-6f) return x
        val g = targetRms / rms
        return FloatArray(x.size) { softLimit(x[it] * g) }
    }

    private const val KNEE = 0.7f
    private const val CEILING = 0.9f

    private fun softLimit(v: Float): Float {
        val a = abs(v)
        if (a <= KNEE) return v
        val over = kotlin.math.tanh(((a - KNEE) / (CEILING - KNEE)).toDouble()).toFloat()
        val y = KNEE + (CEILING - KNEE) * over
        return if (v < 0) -y else y
    }

    /** 区切りを、それぞれの後ろの間([gapsMs]ミリ秒の無音)を挟んで連結する。 */
    fun join(parts: List<FloatArray>, gapsMs: List<Int>, sr: Int): FloatArray {
        var total = 0
        for (i in parts.indices) total += parts[i].size + (gapsMs.getOrElse(i) { 0 } * sr / 1000).coerceAtLeast(0)
        val out = FloatArray(total)
        var pos = 0
        for (i in parts.indices) {
            System.arraycopy(parts[i], 0, out, pos, parts[i].size)
            pos += parts[i].size + (gapsMs.getOrElse(i) { 0 } * sr / 1000).coerceAtLeast(0)
        }
        return out
    }

    /** ピークを[peak]に揃える(無音はそのまま)。 */
    fun normalize(x: FloatArray, peak: Float): FloatArray {
        var m = 0f
        for (v in x) m = max(m, abs(v))
        if (m < 1e-6f) return x
        val g = peak / m
        return FloatArray(x.size) { x[it] * g }
    }

    /** 先頭/末尾5msのフェード(クリックノイズ防止)。 */
    fun fade(x: FloatArray, sr: Int): FloatArray {
        val n = min((sr * 0.005).toInt(), x.size / 2)
        for (i in 0 until n) {
            val g = i.toFloat() / n
            x[i] *= g
            x[x.size - 1 - i] *= g
        }
        return x
    }
}
