package tokyo.runo.maidcafese.core.audio

import java.nio.ByteBuffer
import java.nio.ByteOrder
import kotlin.math.roundToInt

/** モノラルのPCM(-1..1)。 */
class Pcm(val samples: FloatArray, val sampleRate: Int) {
    val seconds: Double get() = samples.size.toDouble() / sampleRate
}

/** 16bit PCM WAVの読み書き(TTSエンジンの`synthesizeToFile`出力を読むための最小実装)。 */
object Wav {
    /** 16bit PCM(モノラル/ステレオ)を読む。ステレオはモノラルに平均。非対応形式・破損はnull。 */
    fun parse(bytes: ByteArray): Pcm? {
        if (bytes.size < 44) return null
        val b = ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN)
        if (String(bytes, 0, 4, Charsets.US_ASCII) != "RIFF" || String(bytes, 8, 4, Charsets.US_ASCII) != "WAVE") return null
        var pos = 12
        var channels = 0
        var rate = 0
        var bits = 0
        var format = 0
        while (pos + 8 <= bytes.size) {
            val id = String(bytes, pos, 4, Charsets.US_ASCII)
            var size = b.getInt(pos + 4)
            val body = pos + 8
            if (id == "fmt ") {
                if (size < 16 || body + 16 > bytes.size) return null
                format = b.getShort(body).toInt() and 0xFFFF
                channels = b.getShort(body + 2).toInt()
                rate = b.getInt(body + 4)
                bits = b.getShort(body + 14).toInt()
            } else if (id == "data") {
                if (format != 1 && format != 0xFFFE || bits != 16 || channels !in 1..2 || rate <= 0) return null
                // ストリーミング出力でサイズが0や過大になっている場合はファイル末尾までとみなす
                if (size <= 0 || body + size > bytes.size) size = bytes.size - body
                val frames = size / (2 * channels)
                val out = FloatArray(frames)
                for (i in 0 until frames) {
                    var sum = 0f
                    for (c in 0 until channels) sum += b.getShort(body + (i * channels + c) * 2) / 32768f
                    out[i] = sum / channels
                }
                return Pcm(out, rate)
            }
            pos = body + size + (size and 1)
            if (size < 0) return null
        }
        return null
    }

    /**
     * 16bit化。量子化歪みを聴こえにくくするため三角分布(TPDF)ディザを加える
     * (make-diskの`triangular_hp`ディザと同じ考え方)。乱数は固定シードで再現可能。
     */
    fun toPcm16(x: FloatArray, seed: Long = 1234L): ShortArray {
        val rnd = java.util.Random(seed)
        return ShortArray(x.size) { i ->
            val d = (rnd.nextFloat() - rnd.nextFloat()) / 32768f
            ((x[i] + d).coerceIn(-1f, 1f) * 32767f).roundToInt().toShort()
        }
    }

    fun toBytes(p: Pcm): ByteArray {
        val n = p.samples.size
        val b = ByteBuffer.allocate(44 + n * 2).order(ByteOrder.LITTLE_ENDIAN)
        b.put("RIFF".toByteArray()).putInt(36 + n * 2).put("WAVE".toByteArray())
        b.put("fmt ".toByteArray()).putInt(16).putShort(1).putShort(1).putInt(p.sampleRate)
            .putInt(p.sampleRate * 2).putShort(2).putShort(16)
        b.put("data".toByteArray()).putInt(n * 2)
        for (s in toPcm16(p.samples)) b.putShort(s)
        return b.array()
    }
}
