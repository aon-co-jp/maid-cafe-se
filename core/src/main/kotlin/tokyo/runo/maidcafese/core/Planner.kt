package tokyo.runo.maidcafese.core

import java.time.LocalDateTime

/** 読み上げ文の生成。TTSが読み間違えやすい記号(♪等)は入れない。 */
object SpeechText {
    /** [base]の後ろに選択されたセリフを(カタログ順で)続ける。何も無ければnull。 */
    fun withPhrases(base: String?, ids: Set<String>): String? {
        val spoken = MaidPhrases.all.filter { it.id in ids }.map { it.spoken }
        val parts = listOfNotNull(base) + spoken
        return if (parts.isEmpty()) null else parts.joinToString(" ")
    }

    /** 指定時刻の読み上げ文。読み上げ文が空でセリフだけ選ばれている場合はセリフのみ。 */
    fun alarmSpeech(kind: AlarmKind, text: String, label: String, voice: VoiceStyle, ids: Set<String>): String? {
        val base = if (kind == AlarmKind.SPEECH && !(text.isBlank() && ids.isNotEmpty())) alarm(text.ifBlank { label }, voice) else null
        return withPhrases(base, ids)
    }

    fun alarm(text: String, voice: VoiceStyle): String = when (voice) {
        VoiceStyle.MAID -> "ご主人様、お時間ですよ。${text.trim()}。忘れずにお願いしますね"
        VoiceStyle.DEEP_MALE -> "時間だ。${text.trim()}"
    }

    fun preNotice(title: String, minutes: Int, voice: VoiceStyle): String = when (voice) {
        VoiceStyle.MAID -> "ご主人様、あと${minutes}分で、${title}のお時間ですわ"
        VoiceStyle.DEEP_MALE -> "あと${minutes}分で、${title}の時間だ"
    }

    fun calendar(title: String, voice: VoiceStyle): String = when (voice) {
        VoiceStyle.MAID -> "ご主人様、${title}のお時間です"
        VoiceStyle.DEEP_MALE -> "${title}の時間だ"
    }
}

/** 登録アラームとカレンダー予定から「次に鳴らすもの」を決める純粋関数。 */
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
        holidays: HolidayCalendar = JapaneseHolidays,
    ): List<Occurrence> {
        val candidates = ArrayList<Occurrence>()
        for (e in entries) {
            if (!e.enabled) continue
            e.schedule.nextTrigger(after, holidays)?.let { t ->
                candidates += Occurrence(
                    time = t, key = "alarm:${e.id}", title = e.label,
                    soundId = if (e.kind == AlarmKind.SOUND) e.soundId else null,
                    speech = SpeechText.alarmSpeech(e.kind, e.text, e.label, e.voice, e.phrases),
                    voice = e.voice, harmony = e.harmony,
                )
            }
            val m = e.schedule.preNoticeMinutes
            e.schedule.nextPreNotice(after, holidays)?.let { t ->
                candidates += Occurrence(
                    time = t, key = "pre:${e.id}", title = e.label, soundId = null,
                    speech = SpeechText.withPhrases(SpeechText.preNotice(e.label, m!!, e.voice), e.prePhrases),
                    voice = e.voice, harmony = e.harmony,
                )
            }
        }
        if (settings.enabled) {
            for (ev in events) {
                if (ev.start.isAfter(after)) {
                    candidates += Occurrence(
                        time = ev.start, key = "cal:${ev.id}@${ev.start}", title = ev.title, soundId = null,
                        speech = SpeechText.withPhrases(SpeechText.calendar(ev.title, settings.voice), settings.phrases),
                        voice = settings.voice, harmony = settings.harmony,
                    )
                }
                val pre = ev.start.minusMinutes(settings.preNoticeMinutes.toLong())
                if (settings.preNotice && pre.isAfter(after)) {
                    candidates += Occurrence(
                        time = pre, key = "calpre:${ev.id}@${ev.start}", title = ev.title, soundId = null,
                        speech = SpeechText.withPhrases(
                            SpeechText.preNotice(ev.title, settings.preNoticeMinutes, settings.voice), settings.prePhrases,
                        ),
                        voice = settings.voice, harmony = settings.harmony,
                    )
                }
            }
        }
        val first = candidates.minOfOrNull { it.time } ?: return emptyList()
        return candidates.filter { it.time == first }
    }
}
