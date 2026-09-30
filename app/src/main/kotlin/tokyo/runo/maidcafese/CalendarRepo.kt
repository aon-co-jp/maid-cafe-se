package tokyo.runo.maidcafese

import android.Manifest
import android.content.ContentUris
import android.content.Context
import android.content.pm.PackageManager
import android.provider.CalendarContract
import androidx.core.content.ContextCompat
import java.time.Instant
import java.time.LocalDateTime
import java.time.ZoneId
import tokyo.runo.maidcafese.core.CalendarEvent

/** 端末に同期済みのGoogleカレンダー(等)を CalendarContract 経由で読む。OAuth・外部送信なし。 */
object CalendarRepo {
    fun hasPermission(ctx: Context) =
        ContextCompat.checkSelfPermission(ctx, Manifest.permission.READ_CALENDAR) == PackageManager.PERMISSION_GRANTED

    fun upcoming(ctx: Context, from: LocalDateTime, to: LocalDateTime): List<CalendarEvent> {
        if (!hasPermission(ctx)) return emptyList()
        val zone = ZoneId.systemDefault()
        val builder = CalendarContract.Instances.CONTENT_URI.buildUpon()
        ContentUris.appendId(builder, from.atZone(zone).toInstant().toEpochMilli())
        ContentUris.appendId(builder, to.atZone(zone).toInstant().toEpochMilli())
        val out = ArrayList<CalendarEvent>()
        try {
            ctx.contentResolver.query(
                builder.build(),
                arrayOf(
                    CalendarContract.Instances.EVENT_ID,
                    CalendarContract.Instances.TITLE,
                    CalendarContract.Instances.BEGIN,
                    CalendarContract.Instances.ALL_DAY,
                ),
                null, null, CalendarContract.Instances.BEGIN + " ASC",
            )?.use { c ->
                while (c.moveToNext()) {
                    if (c.getInt(3) == 1) continue // 終日予定は時刻が無いので読み上げ対象外
                    val start = LocalDateTime.ofInstant(Instant.ofEpochMilli(c.getLong(2)), zone)
                    out += CalendarEvent(c.getLong(0).toString(), c.getString(1)?.ifBlank { null } ?: "予定", start)
                }
            }
        } catch (_: SecurityException) {
            return emptyList()
        }
        return out
    }
}
