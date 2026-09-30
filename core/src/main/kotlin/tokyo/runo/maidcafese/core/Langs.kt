package tokyo.runo.maidcafese.core

import java.util.Locale

/**
 * 読み上げの言語(画面用)。固定のセリフと定型文の各言語の文は、Rust側(`maid-cafe-core`の`lang.rs`)が持つ。
 * **翻訳はAI(Claude)が下訳したもので、ネイティブの確認はしていない。** 入力した文章は翻訳しない。
 */
object Langs {
    data class Lang(val code: String, val name: String, val english: String, val locale: Locale)

    val all = listOf(
        Lang("ja", "日本語", "Japanese", Locale.JAPAN),
        Lang("en", "English", "English", Locale.US),
        Lang("zh", "中文(简体)", "Chinese", Locale.SIMPLIFIED_CHINESE),
        Lang("ko", "한국어", "Korean", Locale.KOREA),
        Lang("it", "Italiano", "Italian", Locale.ITALY),
        Lang("fr", "Français", "French", Locale.FRANCE),
        Lang("de", "Deutsch", "German", Locale.GERMANY),
        Lang("ru", "Русский", "Russian", Locale("ru", "RU")),
        Lang("fa", "فارسی", "Persian (Iran)", Locale("fa", "IR")),
        Lang("ar", "العربية", "Arabic", Locale("ar")),
    )
    const val DEFAULT = "ja"

    fun byCode(code: String): Lang = all.firstOrNull { it.code == code } ?: all.first()
    fun isKnown(code: String) = all.any { it.code == code }
}
