package tokyo.runo.maidcafese.desktop

import java.io.File
import java.nio.file.Files
import java.nio.file.StandardCopyOption
import java.time.LocalDateTime
import java.util.Properties
import tokyo.runo.maidcafese.core.AlarmEntry
import tokyo.runo.maidcafese.core.Codec
import tokyo.runo.maidcafese.core.VoiceStyle

/**
 * 設定の保存先。`%APPDATA%\\maid-cafe-se\\`(システムプロパティ`maidcafese.data`で変更可、テスト用)。
 * アラームはAndroid版と同じ[Codec]の1行1件テキスト、設定はproperties。
 */
object DesktopStore {
    val dir: File = File(
        System.getProperty("maidcafese.data")
            ?: File(System.getenv("APPDATA") ?: System.getProperty("user.home"), "maid-cafe-se").path,
    ).also { it.mkdirs() }

    private val alarmsFile get() = File(dir, "alarms.txt")
    private val settingsFile get() = File(dir, "settings.properties")

    @Synchronized
    fun entries(): List<AlarmEntry> =
        if (alarmsFile.isFile) Codec.decodeAll(alarmsFile.readText(Charsets.UTF_8)) else emptyList()

    @Synchronized
    fun saveEntries(list: List<AlarmEntry>) = writeAtomically(alarmsFile, Codec.encodeAll(list))

    @Synchronized
    fun voiceName(style: VoiceStyle): String? = props().getProperty("voice_${style.name}")?.takeIf { it.isNotBlank() }

    @Synchronized
    fun saveVoiceName(style: VoiceStyle, name: String?) {
        val p = props()
        if (name == null) p.remove("voice_${style.name}") else p.setProperty("voice_${style.name}", name)
        val sw = java.io.StringWriter()
        p.store(sw, null)
        writeAtomically(settingsFile, sw.toString())
    }

    private fun props(): Properties = Properties().also { p ->
        if (settingsFile.isFile) settingsFile.reader(Charsets.UTF_8).use { p.load(it) }
    }

    /** 途中で電源が落ちても壊れないよう、一時ファイルへ書いてから置き換える。 */
    private fun writeAtomically(target: File, text: String) {
        val tmp = File(target.path + ".tmp")
        tmp.writeText(text, Charsets.UTF_8)
        Files.move(tmp.toPath(), target.toPath(), StandardCopyOption.REPLACE_EXISTING)
    }
}

/** `app.log`への簡易ログ(1MBを超えたら作り直す)。 */
object Log {
    private val file get() = File(DesktopStore.dir, "app.log")

    @Synchronized
    fun i(msg: String) {
        val f = file
        if (f.length() > 1_000_000) f.delete()
        f.appendText("${LocalDateTime.now()} $msg\n", Charsets.UTF_8)
    }
}
