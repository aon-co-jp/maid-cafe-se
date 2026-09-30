package tokyo.runo.maidcafese.desktop

import java.io.File
import java.nio.file.Files
import java.util.concurrent.TimeUnit
import kotlin.math.ln
import kotlin.math.roundToInt
import tokyo.runo.maidcafese.core.audio.SourceGender

/** Windowsにインストールされている音声(SAPI5)。 */
data class SapiVoice(val name: String, val culture: String, val gender: SourceGender) {
    val isJapanese: Boolean get() = culture.equals("ja-JP", ignoreCase = true)
}

/** [Sapi.synthesize]の結果: 区切りごとのWAVと、実際に使われた声。 */
class SapiResult(val files: List<File>, val voice: SapiVoice)

/**
 * Windows標準の音声合成(System.Speech = SAPI5)をPowerShell経由で使う。追加インストール不要。
 * 日本語の声(Microsoft Haruka等)はWindowsの「言語」設定で日本語音声を入れると使える。
 */
object Sapi {
    private const val PS_HEADER = "[Console]::OutputEncoding = [Text.Encoding]::UTF8\nAdd-Type -AssemblyName System.Speech\n"

    private const val LIST_SCRIPT = PS_HEADER + """
${'$'}s = New-Object System.Speech.Synthesis.SpeechSynthesizer
${'$'}s.GetInstalledVoices() | Where-Object { ${'$'}_.Enabled } | ForEach-Object {
    ${'$'}_.VoiceInfo.Name + "`t" + ${'$'}_.VoiceInfo.Culture.Name + "`t" + ${'$'}_.VoiceInfo.Gender
}
"""

    private const val SYNTH_SCRIPT = """param([string]${'$'}InFile, [string]${'$'}OutDir, [string]${'$'}Voice)
""" + PS_HEADER + """
${'$'}s = New-Object System.Speech.Synthesis.SpeechSynthesizer
if (${'$'}Voice) {
    ${'$'}s.SelectVoice(${'$'}Voice)
} else {
    ${'$'}ja = ${'$'}s.GetInstalledVoices() | Where-Object { ${'$'}_.Enabled -and ${'$'}_.VoiceInfo.Culture.Name -eq 'ja-JP' } | Select-Object -First 1
    if (-not ${'$'}ja) { [Console]::Error.WriteLine('no ja-JP voice'); exit 2 }
    ${'$'}s.SelectVoice(${'$'}ja.VoiceInfo.Name)
}
Write-Output ("VOICE`t" + ${'$'}s.Voice.Name + "`t" + ${'$'}s.Voice.Culture.Name + "`t" + ${'$'}s.Voice.Gender)
${'$'}lines = [IO.File]::ReadAllLines(${'$'}InFile, [Text.Encoding]::UTF8)
${'$'}i = 0
foreach (${'$'}l in ${'$'}lines) {
    ${'$'}p = ${'$'}l.Split("`t", 2)
    ${'$'}s.Rate = [int]${'$'}p[0]
    ${'$'}f = Join-Path ${'$'}OutDir ("seg_" + ${'$'}i + ".wav")
    ${'$'}s.SetOutputToWaveFile(${'$'}f)
    ${'$'}s.Speak(${'$'}p[1])
    ${'$'}s.SetOutputToNull()
    ${'$'}i++
}
"""

    private fun gender(s: String) = when (s.trim().lowercase()) {
        "female" -> SourceGender.FEMALE
        "male" -> SourceGender.MALE
        else -> SourceGender.UNKNOWN
    }

    /** 話速の倍率(1.0=標準)をSAPIのRate(-10..10)へ。おおむね-10が約1/3倍、+10が約3倍。 */
    fun rateStep(multiplier: Double): Int = (10.0 * ln(multiplier) / ln(3.0)).roundToInt().coerceIn(-10, 10)

    private fun writeScript(dir: File, name: String, body: String): File {
        val f = File(dir, name)
        // Windows PowerShell 5.1は、BOMなしUTF-8のスクリプトを日本語コードページで読んでしまうのでBOMを付ける
        f.writeBytes(byteArrayOf(0xEF.toByte(), 0xBB.toByte(), 0xBF.toByte()) + body.toByteArray(Charsets.UTF_8))
        return f
    }

    private fun run(args: List<String>, timeoutSec: Long): Pair<Int, List<String>> {
        val p = ProcessBuilder(listOf("powershell.exe", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass") + args)
            .redirectErrorStream(false).start()
        p.outputStream.close()
        val err = Thread { p.errorStream.readBytes() }.also { it.isDaemon = true; it.start() }
        val out = p.inputStream.readBytes().toString(Charsets.UTF_8).lines().map { it.trimEnd('\r') }.filter { it.isNotEmpty() }
        if (!p.waitFor(timeoutSec, TimeUnit.SECONDS)) { p.destroyForcibly(); return -1 to out }
        err.join(1000)
        return p.exitValue() to out
    }

    /** インストール済みの音声一覧。取得できなければ空。 */
    fun listVoices(): List<SapiVoice> {
        val dir = Files.createTempDirectory("mcs-sapi").toFile()
        try {
            val script = writeScript(dir, "list.ps1", LIST_SCRIPT)
            val (code, out) = run(listOf("-File", script.absolutePath), 30)
            if (code != 0) return emptyList()
            return out.mapNotNull { l ->
                val p = l.split("\t")
                if (p.size >= 3) SapiVoice(p[0], p[1], gender(p[2])) else null
            }
        } finally { dir.deleteRecursively() }
    }

    /**
     * [items]((SAPIのRate, 文章)の並び)を1回のPowerShell起動でWAVへ合成する。声は[voiceName]、未指定/失敗時は最初の日本語音声。
     * 失敗したらnull(呼び出し側は代替の音に切り替える)。
     */
    fun synthesize(items: List<Pair<Int, String>>, voiceName: String?, outDir: File): SapiResult? {
        if (items.isEmpty()) return null
        outDir.mkdirs()
        val script = writeScript(outDir, "synth.ps1", SYNTH_SCRIPT)
        val input = File(outDir, "in.txt")
        input.writeBytes(items.joinToString("\r\n") { (r, t) -> "$r\t${t.replace('\t', ' ').replace('\r', ' ').replace('\n', ' ')}" }.toByteArray(Charsets.UTF_8))
        val (code, out) = run(
            listOf("-File", script.absolutePath, "-InFile", input.absolutePath, "-OutDir", outDir.absolutePath, "-Voice", voiceName ?: ""),
            120,
        )
        if (code != 0) return null
        val v = out.firstOrNull { it.startsWith("VOICE\t") }?.split("\t") ?: return null
        val voice = SapiVoice(v[1], v.getOrElse(2) { "" }, gender(v.getOrElse(3) { "" }))
        val files = items.indices.map { File(outDir, "seg_$it.wav") }
        if (files.any { !it.isFile || it.length() < 100 }) return null
        return SapiResult(files, voice)
    }
}
