package tokyo.runo.maidcafese.desktop

import java.io.File
import java.time.LocalDateTime
import java.time.LocalTime
import java.util.concurrent.atomic.AtomicBoolean
import kotlin.math.abs
import kotlin.math.sqrt
import tokyo.runo.maidcafese.core.AlarmEntry
import tokyo.runo.maidcafese.core.AlarmKind
import tokyo.runo.maidcafese.core.CalendarSettings
import tokyo.runo.maidcafese.core.NoHolidays
import tokyo.runo.maidcafese.core.Planner
import tokyo.runo.maidcafese.core.Recurrence
import tokyo.runo.maidcafese.core.Schedule
import tokyo.runo.maidcafese.core.VoiceStyle
import tokyo.runo.maidcafese.core.audio.Pcm
import tokyo.runo.maidcafese.core.audio.Wav

/**
 * `maid-cafe-se.exe --selftest [出力フォルダ] [--play]`: 画面を出さずに、Windows音声→加工→WAV書き出しまでを実行し、
 * `report.txt`に結果を書く(インストール後の動作確認・不具合調査用)。`--play`で実際にスピーカーからも鳴らす。
 * 終了コード: 0=すべて成功 / 1=失敗あり。
 */
object SelfTest {
    fun run(outArg: String?, play: Boolean): Int {
        val out = File(outArg ?: File(System.getProperty("java.io.tmpdir"), "maid-cafe-se-selftest").path).also { it.mkdirs() }
        val report = StringBuilder()
        var failed = false
        fun line(s: String) { report.appendLine(s) }
        fun fail(s: String) { failed = true; line("FAIL: $s") }

        line("maid-cafe-se selftest ${LocalDateTime.now()}")
        val voices = Sapi.listVoices()
        line("voices: " + voices.joinToString("; ") { "${it.name}(${it.culture},${it.gender})" })
        if (voices.none { it.isJapanese }) fail("日本語のWindows音声がありません")

        fun occ(voice: VoiceStyle, harmony: Boolean, phrases: Set<String>) = Planner.next(
            listOf(
                AlarmEntry(
                    "t", "テスト", Schedule(Recurrence.Daily, LocalTime.of(7, 0)), AlarmKind.SPEECH,
                    text = "", voice = voice, phrases = phrases, harmony = harmony,
                ),
            ),
            emptyList(), CalendarSettings(), LocalDateTime.of(2026, 1, 1, 0, 0), NoHolidays,
        ).first()

        val cases = listOf(
            Triple("maid", occ(VoiceStyle.MAID, false, linkedSetOf("okite", "okaeri", "fight")), "メイド風(起きて→おかえり→ファイト)"),
            Triple("maid-harmony", occ(VoiceStyle.MAID, true, linkedSetOf("okaeri", "okite")), "メイド風ハモり(おかえり→起きて)"),
            Triple("deep", occ(VoiceStyle.DEEP_MALE, false, linkedSetOf("perfect", "excellent")), "低い男性(パーフェクト→エクセレント)"),
        )
        val runner = AlarmRunner()
        val rendered = ArrayList<Pair<String, Pcm>>()
        for ((name, o, desc) in cases) {
            val t0 = System.nanoTime()
            val pcm = runner.render(o)
            val ms = (System.nanoTime() - t0) / 1_000_000
            if (pcm == null) { fail("$desc: 合成/加工に失敗"); continue }
            val rms = sqrt(pcm.samples.sumOf { (it * it).toDouble() } / pcm.samples.size)
            val peak = pcm.samples.maxOf { abs(it) }
            File(out, "$name.wav").writeBytes(Wav.toBytes(pcm))
            line("OK: $desc ${"%.2f".format(pcm.seconds)}s sr=${pcm.sampleRate} rms=${"%.3f".format(rms)} peak=${"%.2f".format(peak)} (${ms}ms) -> $name.wav")
            if (peak > 0.91f || rms < 0.02) fail("$desc: 音量が想定外 (rms=$rms peak=$peak)")
            rendered += name to pcm
        }
        if (play && rendered.isNotEmpty()) {
            for ((name, pcm) in rendered.take(1)) {
                val t0 = System.nanoTime()
                try {
                    AudioOut.playPcm(pcm, AtomicBoolean(false))
                    val sec = (System.nanoTime() - t0) / 1e9
                    line("play $name: ${"%.2f".format(sec)}s (音声の長さ ${"%.2f".format(pcm.seconds)}s)")
                    if (sec < pcm.seconds * 0.8) fail("再生が音声の長さより短い(オーディオ出力が無効?)")
                } catch (e: Exception) { fail("再生できません: $e") }
            }
        }
        line(if (failed) "RESULT: FAIL" else "RESULT: OK")
        File(out, "report.txt").writeText(report.toString(), Charsets.UTF_8)
        return if (failed) 1 else 0
    }
}
