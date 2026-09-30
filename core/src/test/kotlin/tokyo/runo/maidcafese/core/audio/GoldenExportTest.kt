package tokyo.runo.maidcafese.core.audio

import java.io.File
import java.nio.ByteBuffer
import java.nio.ByteOrder
import kotlin.math.PI
import kotlin.math.sin
import kotlin.test.Test
import tokyo.runo.maidcafese.core.VoiceStyle

/**
 * Rust移植版との照合用に、Kotlin版の音声処理の出力(float32リトルエンディアン)を書き出す。
 * 環境変数 MCS_GOLDEN_DIR が設定されているときだけ動く(通常のテスト実行では何もしない)。
 * 書き出し先: crates/maid-cafe-core/tests/golden/
 */
class GoldenExportTest {
    private val sr = 22050

    private fun voiced(f0: Double, seconds: Double): FloatArray = FloatArray((sr * seconds).toInt()) { i ->
        val t = i.toDouble() / sr
        val env = 0.6 + 0.4 * sin(2 * PI * 3 * t)
        var v = 0.0
        for (h in 1..5) v += sin(2 * PI * f0 * h * t) / h
        (env * v * 0.3).toFloat()
    }

    private fun write(dir: File, name: String, x: FloatArray) {
        val b = ByteBuffer.allocate(x.size * 4).order(ByteOrder.LITTLE_ENDIAN)
        x.forEach { b.putFloat(it) }
        File(dir, "$name.f32").writeBytes(b.array())
    }

    @Test fun exportGolden() {
        val dir = System.getenv("MCS_GOLDEN_DIR")?.let(::File) ?: return
        dir.mkdirs()
        val input = Pcm(voiced(200.0, 0.6), sr)
        write(dir, "maid_female", VoiceDsp.render(input, VoiceStyle.MAID, SourceGender.FEMALE, false).samples)
        write(dir, "deep_female_harmony", VoiceDsp.render(input, VoiceStyle.DEEP_MALE, SourceGender.FEMALE, true).samples)
        write(dir, "maid_male_pitchmul", VoiceDsp.render(input, VoiceStyle.MAID, SourceGender.MALE, false, 1.06).samples)
        write(dir, "deep_male", VoiceDsp.render(input, VoiceStyle.DEEP_MALE, SourceGender.MALE, false).samples)
    }
}
