package tokyo.runo.maidcafese.core

import java.net.URLDecoder
import java.net.URLEncoder
import java.time.DayOfWeek
import java.time.LocalDate
import java.time.LocalTime

/**
 * 保存形式。1行=1アラーム、`key=value&key=value`(値はURLエンコード)。JSONライブラリ非依存で
 * JVMのみでテスト可能。壊れた行・未知の値は[decodeAll]が読み飛ばす(アプリ更新での起動不能を避ける)。
 */
object Codec {
    fun encodeRecurrence(r: Recurrence): String = when (r) {
        Recurrence.Daily -> "daily"
        is Recurrence.Weekdays -> "weekdays:${if (r.skipHolidays) 1 else 0}"
        Recurrence.WeekendsAndHolidays -> "weekends"
        is Recurrence.DaysOfWeek -> "dow:" + r.days.sorted().joinToString(",")
        is Recurrence.NthWeekdayOfMonth -> "nth:${r.nth}:${r.day}"
        is Recurrence.EveryNWeeks -> "every:${r.interval}:${r.days.sorted().joinToString(",")}:${r.anchor}"
    }

    fun decodeRecurrence(s: String): Recurrence {
        val p = s.split(":")
        fun days(x: String) = x.split(",").map { DayOfWeek.valueOf(it) }.toSet()
        return when (p[0]) {
            "daily" -> Recurrence.Daily
            "weekdays" -> Recurrence.Weekdays(p.getOrNull(1) != "0")
            "weekends" -> Recurrence.WeekendsAndHolidays
            "dow" -> Recurrence.DaysOfWeek(days(p[1]))
            "nth" -> Recurrence.NthWeekdayOfMonth(p[1].toInt(), DayOfWeek.valueOf(p[2]))
            "every" -> Recurrence.EveryNWeeks(p[1].toInt(), days(p[2]), LocalDate.parse(p[3]))
            else -> throw IllegalArgumentException("unknown recurrence: $s")
        }
    }

    private fun enc(s: String) = URLEncoder.encode(s, "UTF-8")
    private fun dec(s: String) = URLDecoder.decode(s, "UTF-8")

    fun encode(e: AlarmEntry): String = listOf(
        "id" to e.id,
        "label" to e.label,
        "rec" to encodeRecurrence(e.schedule.recurrence),
        "time" to e.schedule.time.toString(),
        "start" to (e.schedule.startDate?.toString() ?: ""),
        "end" to (e.schedule.endDate?.toString() ?: ""),
        "pre" to (e.schedule.preNoticeMinutes?.toString() ?: ""),
        "kind" to e.kind.name,
        "sound" to e.soundId,
        "text" to e.text,
        "voice" to e.voice.name,
        "enabled" to if (e.enabled) "1" else "0",
        "ph" to e.phrases.joinToString(","),
        "pph" to e.prePhrases.joinToString(","),
        "harm" to if (e.harmony) "1" else "0",
    ).plus(if (e.speechSound) listOf("ss" to "1") else emptyList()) // オンのときだけ書く(旧版と同じ行を保つ)
        .plus(if (e.lang != Langs.DEFAULT) listOf("lang" to e.lang) else emptyList())
        .joinToString("&") { (k, v) -> "$k=${enc(v)}" }

    fun decode(line: String): AlarmEntry {
        val m = line.split("&").associate {
            val i = it.indexOf('=')
            it.substring(0, i) to dec(it.substring(i + 1))
        }
        fun opt(k: String) = m[k]?.takeIf { it.isNotEmpty() }
        return AlarmEntry(
            id = m.getValue("id"),
            label = m.getValue("label"),
            schedule = Schedule(
                recurrence = decodeRecurrence(m.getValue("rec")),
                time = LocalTime.parse(m.getValue("time")),
                startDate = opt("start")?.let(LocalDate::parse),
                endDate = opt("end")?.let(LocalDate::parse),
                preNoticeMinutes = opt("pre")?.toInt(),
            ),
            kind = AlarmKind.valueOf(m.getValue("kind")),
            soundId = m["sound"]?.takeIf(SoundCatalog::isKnown) ?: SoundCatalog.DEFAULT_ID,
            text = m["text"] ?: "",
            voice = VoiceStyle.valueOf(m.getValue("voice")),
            enabled = m["enabled"] != "0",
            phrases = decodeIds(m["ph"]),
            prePhrases = decodeIds(m["pph"]),
            harmony = m["harm"] == "1",
            speechSound = m["ss"] == "1",
            lang = m["lang"]?.takeIf(Langs::isKnown) ?: Langs.DEFAULT,
        )
    }

    /** 保存されたセリフidを復元(未知のidは捨てる。旧バージョンのデータには欠けているので空扱い)。 */
    fun decodeIds(s: String?): Set<String> =
        MaidPhrases.known(s.orEmpty().split(",").filter { it.isNotEmpty() }.toSet())

    fun encodeAll(list: List<AlarmEntry>): String = list.joinToString("\n") { encode(it) }

    fun decodeAll(s: String): List<AlarmEntry> =
        s.lineSequence().filter { it.isNotBlank() }.mapNotNull { runCatching { decode(it) }.getOrNull() }.toList()
}
