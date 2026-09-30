package tokyo.runo.maidcafese

import android.content.Context
import tokyo.runo.maidcafese.core.AlarmEntry
import tokyo.runo.maidcafese.core.CalendarSettings
import tokyo.runo.maidcafese.core.Codec
import tokyo.runo.maidcafese.core.VoiceStyle

/** SharedPreferences による永続化(形式は core の [Codec])。 */
object Store {
    private fun prefs(ctx: Context) = ctx.applicationContext.getSharedPreferences("maidcafese", Context.MODE_PRIVATE)

    fun entries(ctx: Context): List<AlarmEntry> = Codec.decodeAll(prefs(ctx).getString("entries", "") ?: "")

    fun saveEntries(ctx: Context, list: List<AlarmEntry>) {
        prefs(ctx).edit().putString("entries", Codec.encodeAll(list)).apply()
    }

    fun calendar(ctx: Context): CalendarSettings {
        val p = prefs(ctx)
        return CalendarSettings(
            enabled = p.getBoolean("cal_enabled", false),
            preNotice = p.getBoolean("cal_pre", true),
            preNoticeMinutes = p.getInt("cal_pre_min", 30).coerceAtLeast(1),
            voice = runCatching { VoiceStyle.valueOf(p.getString("cal_voice", "MAID")!!) }.getOrDefault(VoiceStyle.MAID),
        )
    }

    fun saveCalendar(ctx: Context, s: CalendarSettings) {
        prefs(ctx).edit()
            .putBoolean("cal_enabled", s.enabled)
            .putBoolean("cal_pre", s.preNotice)
            .putInt("cal_pre_min", s.preNoticeMinutes)
            .putString("cal_voice", s.voice.name)
            .apply()
    }
}
