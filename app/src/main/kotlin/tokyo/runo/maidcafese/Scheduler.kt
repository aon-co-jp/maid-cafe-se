package tokyo.runo.maidcafese

import android.app.AlarmManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import java.time.LocalDateTime
import java.time.ZoneId
import tokyo.runo.maidcafese.core.Planner

/**
 * 次に鳴らす1件だけを AlarmManager.setAlarmClock で登録する(Doze中も確実に発火、正確アラーム権限不要)。
 * 発火後・起動時・再起動後・設定変更時に毎回計算し直す。カレンダーは先7日分を取り込み、
 * 毎日4時にも再計算して新しい予定を拾う。
 */
object Scheduler {
    const val EXTRA_TIME = "time"
    const val ACTION_REFRESH = "tokyo.runo.maidcafese.REFRESH"
    private const val REQ_ALARM = 1
    private const val REQ_REFRESH = 2
    private const val REQ_SHOW = 3
    private const val FLAGS = PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE

    private fun zone() = ZoneId.systemDefault()

    fun reschedule(ctx: Context, after: LocalDateTime = LocalDateTime.now()) {
        val app = ctx.applicationContext
        val am = app.getSystemService(Context.ALARM_SERVICE) as AlarmManager
        val settings = Store.calendar(app)
        val events = if (settings.enabled) CalendarRepo.upcoming(app, after, after.plusDays(7)) else emptyList()
        val next = Planner.next(Store.entries(app), events, settings, after)

        fun firePi(ms: Long) = PendingIntent.getBroadcast(
            app, REQ_ALARM, Intent(app, AlarmReceiver::class.java).putExtra(EXTRA_TIME, ms), FLAGS,
        )
        if (next.isEmpty()) {
            am.cancel(firePi(0L))
        } else {
            val ms = next[0].time.atZone(zone()).toInstant().toEpochMilli()
            val showPi = PendingIntent.getActivity(app, REQ_SHOW, Intent(app, MainActivity::class.java), FLAGS)
            try {
                am.setAlarmClock(AlarmManager.AlarmClockInfo(ms, showPi), firePi(ms))
            } catch (_: SecurityException) {
                // 正確アラーム権限が無効化された場合でも起動不能にせず、精度が落ちる通常アラームで代替する。
                am.setAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, ms, firePi(ms))
            }
        }

        // 毎日4:00の再計算(カレンダーの新規予定取り込み用)。
        var refresh = after.toLocalDate().atTime(4, 0)
        if (!refresh.isAfter(after)) refresh = refresh.plusDays(1)
        val refreshPi = PendingIntent.getBroadcast(
            app, REQ_REFRESH, Intent(app, AlarmReceiver::class.java).setAction(ACTION_REFRESH), FLAGS,
        )
        am.setAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, refresh.atZone(zone()).toInstant().toEpochMilli(), refreshPi)
    }

    /** 直近の予定(画面表示用)。 */
    fun nextPreview(ctx: Context): String? {
        val app = ctx.applicationContext
        val settings = Store.calendar(app)
        val now = LocalDateTime.now()
        val events = if (settings.enabled) CalendarRepo.upcoming(app, now, now.plusDays(7)) else emptyList()
        val n = Planner.next(Store.entries(app), events, settings, now)
        return n.firstOrNull()?.let { "${it.time.toLocalDate()} ${it.time.toLocalTime()}  ${it.title}" }
    }
}
