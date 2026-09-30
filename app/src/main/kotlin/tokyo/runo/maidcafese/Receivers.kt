package tokyo.runo.maidcafese

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import androidx.core.content.ContextCompat

class AlarmReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action == Scheduler.ACTION_REFRESH) {
            Scheduler.reschedule(context)
            return
        }
        val time = intent.getLongExtra(Scheduler.EXTRA_TIME, 0L)
        if (time == 0L) return
        ContextCompat.startForegroundService(
            context,
            Intent(context, AlarmService::class.java).putExtra(Scheduler.EXTRA_TIME, time),
        )
    }
}

/** 再起動・アプリ更新・時刻/タイムゾーン変更で登録を復元する。 */
class BootReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        Scheduler.reschedule(context)
    }
}
