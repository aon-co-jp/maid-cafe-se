package tokyo.runo.maidcafese

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.media.AudioAttributes
import android.media.MediaPlayer
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.os.PowerManager
import android.speech.tts.TextToSpeech
import android.speech.tts.UtteranceProgressListener
import android.util.Log
import java.time.Duration
import java.time.Instant
import java.time.LocalDateTime
import java.time.ZoneId
import java.util.Locale
import tokyo.runo.maidcafese.core.Occurrence
import tokyo.runo.maidcafese.core.Planner
import tokyo.runo.maidcafese.core.VoiceStyle

/** 発火時に音を鳴らし・文章を読み上げるフォアグラウンドサービス。全て終わるか「停止」で自分を止める。 */
class AlarmService : Service() {
    companion object {
        private const val TAG = "MaidCafeSe"
        private const val CHANNEL = "alarm"
        private const val NOTIF_ID = 1
        private const val SOUND_MAX_MS = 30_000L
        const val ACTION_STOP = "tokyo.runo.maidcafese.STOP"
        const val ACTION_TEST = "tokyo.runo.maidcafese.TEST"
        const val EXTRA_SOUND = "sound"
        const val EXTRA_SPEECH = "speech"
        const val EXTRA_VOICE = "voice"
    }

    private val main = Handler(Looper.getMainLooper())
    private var player: MediaPlayer? = null
    private var tts: TextToSpeech? = null
    private var ttsReady = false
    private var ttsFailed = false
    private var pendingSpeech: List<Occurrence> = emptyList()
    private var speechRemaining = 0
    private var soundActive = false
    private var wakeLock: PowerManager.WakeLock? = null
    private var started = false

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACTION_STOP) {
            finish()
            return START_NOT_STICKY
        }
        if (started) return START_NOT_STICKY // 再生中の重複起動は無視(次回分は再計算で登録済み)
        started = true
        startForegroundCompat("アラーム")
        wakeLock = (getSystemService(Context.POWER_SERVICE) as PowerManager)
            .newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "maidcafese:alarm").apply { acquire(90_000L) }

        if (intent?.action == ACTION_TEST) {
            val voice = runCatching { VoiceStyle.valueOf(intent.getStringExtra(EXTRA_VOICE) ?: "") }.getOrDefault(VoiceStyle.MAID)
            play(listOf(Occurrence(LocalDateTime.now(), "test", "テスト", intent.getStringExtra(EXTRA_SOUND), intent.getStringExtra(EXTRA_SPEECH), voice)))
            return START_NOT_STICKY
        }
        val timeMs = intent?.getLongExtra(Scheduler.EXTRA_TIME, 0L) ?: 0L
        if (timeMs == 0L) {
            finish()
            return START_NOT_STICKY
        }
        Thread {
            val t = LocalDateTime.ofInstant(Instant.ofEpochMilli(timeMs), ZoneId.systemDefault())
            val due = dueAt(t)
            Scheduler.reschedule(this, t) // 再生の成否に関わらず先に次回分を登録する
            main.post { if (due.isEmpty()) finish() else play(due) }
        }.start()
        return START_NOT_STICKY
    }

    private fun dueAt(t: LocalDateTime): List<Occurrence> {
        val settings = Store.calendar(this)
        val after = t.minusSeconds(1)
        val events = if (settings.enabled) CalendarRepo.upcoming(this, after, after.plusDays(1)) else emptyList()
        return Planner.next(Store.entries(this), events, settings, after)
            .filter { Duration.between(t, it.time).abs() <= Duration.ofMinutes(1) }
    }

    private fun play(list: List<Occurrence>) {
        startForegroundCompat(list.joinToString(" / ") { it.title })
        val sound = list.firstNotNullOfOrNull { it.soundId }
        pendingSpeech = list.filter { it.speech != null }
        speechRemaining = pendingSpeech.size
        if (sound != null) startSound(sound)
        if (pendingSpeech.isNotEmpty()) startTts()
        if (sound == null && pendingSpeech.isEmpty()) finish()
    }

    private fun alarmAttrs() = AudioAttributes.Builder()
        .setUsage(AudioAttributes.USAGE_ALARM)
        .setContentType(AudioAttributes.CONTENT_TYPE_SONIFICATION)
        .build()

    private fun startSound(soundId: String) {
        val resId = resources.getIdentifier(soundId, "raw", packageName)
        if (resId == 0) { Log.w(TAG, "unknown sound $soundId"); return }
        try {
            player = MediaPlayer().apply {
                setAudioAttributes(alarmAttrs())
                resources.openRawResourceFd(resId).use { setDataSource(it.fileDescriptor, it.startOffset, it.length) }
                isLooping = true
                prepare()
                start()
            }
            soundActive = true
            main.postDelayed({ stopSound(); maybeFinish() }, SOUND_MAX_MS)
        } catch (e: Exception) {
            Log.e(TAG, "sound failed", e)
        }
    }

    private fun stopSound() {
        soundActive = false
        player?.runCatching { stop(); release() }
        player = null
    }

    private fun startTts() {
        tts = TextToSpeech(this) { status ->
            main.post {
                if (status != TextToSpeech.SUCCESS) { onTtsFailed("TTS初期化失敗"); return@post }
                val engine = tts ?: return@post
                val r = engine.setLanguage(Locale.JAPAN)
                if (r == TextToSpeech.LANG_MISSING_DATA || r == TextToSpeech.LANG_NOT_SUPPORTED) {
                    onTtsFailed("日本語音声データがありません"); return@post
                }
                engine.setAudioAttributes(alarmAttrs())
                engine.setOnUtteranceProgressListener(object : UtteranceProgressListener() {
                    override fun onStart(id: String?) {}
                    override fun onDone(id: String?) { main.post { speechRemaining--; maybeFinish() } }
                    @Deprecated("Deprecated in Java")
                    override fun onError(id: String?) { main.post { speechRemaining--; maybeFinish() } }
                })
                ttsReady = true
                pendingSpeech.forEachIndexed { i, occ ->
                    VoiceProfile.apply(engine, occ.voice)
                    engine.speak(occ.speech, TextToSpeech.QUEUE_ADD, null, "u$i")
                }
            }
        }
    }

    /** 読み上げできない端末では、無音で終わらないよう既定のチャイムに切り替える。 */
    private fun onTtsFailed(reason: String) {
        Log.w(TAG, reason)
        ttsFailed = true
        speechRemaining = 0
        if (!soundActive) startSound("chime")
        maybeFinish()
    }

    private fun maybeFinish() {
        if (speechRemaining <= 0 && !soundActive && (ttsReady || ttsFailed || pendingSpeech.isEmpty())) finish()
    }

    private fun finish() {
        main.removeCallbacksAndMessages(null)
        stopSound()
        tts?.runCatching { stop(); shutdown() }
        tts = null
        wakeLock?.runCatching { if (isHeld) release() }
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    override fun onDestroy() {
        stopSound()
        tts?.runCatching { shutdown() }
        wakeLock?.runCatching { if (isHeld) release() }
        super.onDestroy()
    }

    private fun startForegroundCompat(text: String) {
        val nm = getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(NotificationChannel(CHANNEL, "アラーム・読み上げ", NotificationManager.IMPORTANCE_HIGH))
        val stopPi = PendingIntent.getService(
            this, 0, Intent(this, AlarmService::class.java).setAction(ACTION_STOP),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
        val n = Notification.Builder(this, CHANNEL)
            .setSmallIcon(android.R.drawable.ic_lock_idle_alarm)
            .setContentTitle("maid-cafe-se")
            .setContentText(text)
            .setOngoing(true)
            .addAction(Notification.Action.Builder(null, "停止", stopPi).build())
            .build()
        if (Build.VERSION.SDK_INT >= 29) {
            startForeground(NOTIF_ID, n, ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK)
        } else {
            startForeground(NOTIF_ID, n)
        }
    }
}

/** 声の選択。端末のTTS音声から best-effort で男女を選び、ピッチ/速度で「高め」「太く低い」を作る。 */
object VoiceProfile {
    private val FEMALE_HINTS = listOf("female", "jab", "htm", "-f-")
    private val MALE_HINTS = listOf("jac", "jad", "-m-")

    fun apply(tts: TextToSpeech, voice: VoiceStyle) {
        val ja = runCatching { tts.voices.filter { it.locale.language == "ja" && !it.isNetworkConnectionRequired } }.getOrDefault(emptyList())
        fun pick(hints: List<String>, avoid: List<String>) = ja.firstOrNull { v ->
            val n = v.name.lowercase()
            hints.any { n.contains(it) } && avoid.none { n.contains(it) }
        }
        when (voice) {
            VoiceStyle.MAID -> {
                pick(FEMALE_HINTS, emptyList())?.let { tts.voice = it }
                tts.setPitch(1.4f); tts.setSpeechRate(1.1f)
            }
            VoiceStyle.DEEP_MALE -> {
                (pick(MALE_HINTS, FEMALE_HINTS) ?: pick(listOf("male"), listOf("female")))?.let { tts.voice = it }
                tts.setPitch(0.55f); tts.setSpeechRate(0.85f)
            }
        }
    }
}
