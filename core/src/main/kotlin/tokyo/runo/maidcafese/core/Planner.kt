package tokyo.runo.maidcafese.core

import java.net.URLDecoder
import java.net.URLEncoder
import java.time.LocalDateTime
import java.time.ZoneOffset

/** 読み上げ文。生成はRust側。 */
object SpeechText {
    /** 指定時刻の読み上げ文(全体)。読み上げない設定ならnull。 */
    fun alarmSpeech(kind: AlarmKind, text: String, label: String, voice: VoiceStyle, ids: Set<String>): String? =
        Native.alarmSpeech(kind.name, text, label, voice.name, ids.joinToString(","))
}

/** 登録アラームとカレンダー予定から「次に鳴らすもの」を決める。計算はRust側(`crates/maid-cafe-jni`→`maid-cafe-core`)。 */
object Planner {
    /**
     * [after]より後で最も早い発火時刻の[Occurrence]を全件返す(同時刻が複数あれば複数)。無ければ空。
     * カレンダー予定は開始時刻に読み上げ、[CalendarSettings.preNotice]なら[CalendarSettings.preNoticeMinutes]分前にも予告。
     */
    fun next(
        entries: List<AlarmEntry>,
        events: List<CalendarEvent>,
        settings: CalendarSettings,
        after: LocalDateTime,
    ): List<Occurrence> {
        val out = Native.planNext(Codec.encodeAll(entries), encodeEvents(events), encodeSettings(settings), secs(after))
        return out.lines().filter { it.isNotBlank() }.mapNotNull { runCatching { decodeOccurrence(it) }.getOrNull() }
    }

    private fun secs(t: LocalDateTime) = t.toEpochSecond(ZoneOffset.UTC)

    private fun enc(s: String) = URLEncoder.encode(s, "UTF-8")
    private fun dec(s: String) = URLDecoder.decode(s, "UTF-8")

    private fun encodeEvents(events: List<CalendarEvent>) =
        events.joinToString("\n") { "id=${enc(it.id)}&title=${enc(it.title)}&start=${secs(it.start)}" }

    private fun encodeSettings(s: CalendarSettings) = listOf(
        "enabled" to if (s.enabled) "1" else "0",
        "pre" to if (s.preNotice) "1" else "0",
        "premin" to s.preNoticeMinutes.toString(),
        "voice" to s.voice.name,
        "ph" to s.phrases.joinToString(","),
        "pph" to s.prePhrases.joinToString(","),
        "harm" to if (s.harmony) "1" else "0",
    ).joinToString("&") { (k, v) -> "$k=${enc(v)}" }

    private fun decodeOccurrence(line: String): Occurrence {
        val m = line.split("&").associate {
            val i = it.indexOf('=')
            it.substring(0, i) to it.substring(i + 1)
        }
        fun text(k: String) = dec(m.getValue(k))
        val segments = m["seg"].orEmpty().split(";").filter { it.isNotEmpty() }.map {
            val f = it.split(",")
            Segment(dec(f[0]), f[1].toDouble(), f[2].toFloat(), f[3].toInt())
        }
        return Occurrence(
            time = LocalDateTime.ofEpochSecond(m.getValue("t").toLong(), 0, ZoneOffset.UTC),
            key = text("key"),
            title = text("title"),
            soundId = text("sound").ifEmpty { null },
            speech = text("speech").ifEmpty { null },
            voice = VoiceStyle.valueOf(m.getValue("voice")),
            harmony = m["harm"] == "1",
            lang = m["lang"] ?: Langs.DEFAULT,
            segments = segments,
        )
    }
}
