package tokyo.runo.maidcafese.desktop

import java.io.BufferedInputStream
import java.nio.file.Files
import java.time.Duration
import java.time.LocalDateTime
import java.util.concurrent.atomic.AtomicBoolean
import javax.sound.sampled.AudioFormat
import javax.sound.sampled.AudioSystem
import javax.sound.sampled.Clip
import javax.sound.sampled.SourceDataLine
import tokyo.runo.maidcafese.core.AlarmEntry
import tokyo.runo.maidcafese.core.CalendarSettings
import tokyo.runo.maidcafese.core.Occurrence
import tokyo.runo.maidcafese.core.Planner
import tokyo.runo.maidcafese.core.Segment
import tokyo.runo.maidcafese.core.VoiceStyle
import tokyo.runo.maidcafese.core.audio.Pcm
import tokyo.runo.maidcafese.core.audio.VoiceDsp
import tokyo.runo.maidcafese.core.audio.Wav

/** 音の出力(javax.sound)。 */
object AudioOut {
    /** PCM(モノラル)を最後まで鳴らす。[stop]が立ったら途中でやめる。鳴らし終えるまでブロックする。 */
    fun playPcm(pcm: Pcm, stop: AtomicBoolean) {
        val fmt = AudioFormat(pcm.sampleRate.toFloat(), 16, 1, true, false)
        val data = Wav.toPcm16(pcm.samples)
        val bytes = ByteArray(data.size * 2)
        for (i in data.indices) {
            bytes[i * 2] = (data[i].toInt() and 0xFF).toByte()
            bytes[i * 2 + 1] = (data[i].toInt() shr 8).toByte()
        }
        val line: SourceDataLine = AudioSystem.getSourceDataLine(fmt)
        line.open(fmt)
        line.start()
        var pos = 0
        while (pos < bytes.size && !stop.get()) {
            val n = minOf(4096, bytes.size - pos)
            line.write(bytes, pos, n)
            pos += n
        }
        if (stop.get()) line.stop() else line.drain()
        line.close()
    }

    /** 同梱の音(chime/alarm/melody)をループ再生して[Clip]を返す。呼び出し側が止める。 */
    fun startLoop(soundId: String): Clip? {
        val res = AudioOut::class.java.getResourceAsStream("/$soundId.wav") ?: return null
        return try {
            val ais = AudioSystem.getAudioInputStream(BufferedInputStream(res))
            val clip = AudioSystem.getClip()
            clip.open(ais)
            clip.loop(Clip.LOOP_CONTINUOUSLY)
            clip
        } catch (e: Exception) {
            Log.i("sound failed: $e")
            null
        }
    }
}

/**
 * 発火した[Occurrence]を鳴らす。音は最大30秒ループ、読み上げは
 * Windows音声(SAPI)で区切りごとにWAVを作り→[VoiceDsp]で声を加工→間を挟んで連結→再生(Android版と同じ処理)。
 * 読み上げに失敗したら、無音で終わらないようチャイムに切り替える。
 */
class AlarmRunner {
    companion object {
        const val SOUND_MAX_MS = 30_000L
    }

    private val stopFlag = AtomicBoolean(false)
    @Volatile var playing: String? = null
        private set

    fun stop() = stopFlag.set(true)

    /** 鳴らし終えるまでブロックする(呼び出し側のスレッドで実行)。 */
    fun play(list: List<Occurrence>) {
        stopFlag.set(false)
        playing = list.joinToString(" / ") { it.title }
        var clip: Clip? = null
        try {
            list.firstNotNullOfOrNull { it.soundId }?.let { clip = AudioOut.startLoop(it) }
            val soundEnd = System.currentTimeMillis() + SOUND_MAX_MS
            var spoke = false
            for (o in list) {
                if (o.speech == null || stopFlag.get()) continue
                spoke = true
                if (!speak(o)) {
                    if (clip == null) clip = AudioOut.startLoop("chime")
                }
            }
            // 音だけの場合(または読み上げ後も音が残っている場合)は、30秒たつか止められるまで鳴らす
            if (clip != null) {
                val until = if (spoke) minOf(soundEnd, System.currentTimeMillis() + 500) else soundEnd
                while (System.currentTimeMillis() < until && !stopFlag.get()) Thread.sleep(100)
            }
        } finally {
            clip?.runCatching { stop(); close() }
            playing = null
        }
    }

    /**
     * 1件の読み上げ音声を作る(区切りごとにSAPIで合成→[VoiceDsp]で加工→間を挟んで連結)。失敗したらnull。
     * 実再生([speak])と自己診断(--selftest)で共通に使う。
     */
    fun render(o: Occurrence): Pcm? {
        val segs = o.segments.ifEmpty { listOf(Segment(o.speech.orEmpty())) }
        val baseRate = if (o.voice == VoiceStyle.DEEP_MALE) 0.92 else 1.0
        val tmp = Files.createTempDirectory("mcs").toFile()
        try {
            val res = Sapi.synthesize(
                segs.map { Sapi.rateStep(baseRate * it.rate) to it.text }, DesktopStore.voiceName(o.voice), tmp,
            )
            if (res == null) { Log.i("speech synthesis failed (no Japanese voice?)"); return null }
            val parts = ArrayList<FloatArray>()
            var sr = 0
            for ((k, f) in res.files.withIndex()) {
                val pcm = Wav.parse(f.readBytes()) ?: run { Log.i("WAV parse failed: $f"); return null }
                sr = pcm.sampleRate
                parts += VoiceDsp.render(pcm, o.voice, res.voice.gender, o.harmony, segs[k].pitch).samples
            }
            val joined = Pcm(VoiceDsp.join(parts, segs.map { it.gapAfterMs }, sr), sr)
            Log.i("speech ${segs.size} segments voice='${res.voice.name}' ${"%.2f".format(joined.seconds)}s style=${o.voice} harmony=${o.harmony}")
            return joined
        } catch (e: Exception) {
            Log.i("speech failed: $e")
            return null
        } finally {
            tmp.deleteRecursively()
        }
    }

    /** 1件を読み上げる。成功したらtrue。 */
    fun speak(o: Occurrence): Boolean {
        val pcm = render(o) ?: return false
        return try {
            AudioOut.playPcm(pcm, stopFlag)
            true
        } catch (e: Exception) {
            Log.i("audio output failed: $e")
            false
        }
    }
}

/**
 * 次に鳴らすアラームを監視するスレッド。30秒ごと(またはアラームが変わったとき)に再計算し、
 * 時刻になったら[onFire]を呼ぶ。スリープ復帰などで90秒以上遅れた分は鳴らさず飛ばす。
 */
class SchedulerLoop(
    private val entries: () -> List<AlarmEntry>,
    private val onFire: (List<Occurrence>) -> Unit,
) : Thread("maid-cafe-se-scheduler") {
    @Volatile private var running = true

    init { isDaemon = true }

    fun shutdown() { running = false; interrupt() }

    /** アラームが編集されたら呼ぶ(待機中なら即座に再計算する)。 */
    fun changed() = interrupt()

    override fun run() {
        var cursor = LocalDateTime.now()
        while (running) {
            try {
                val next = Planner.next(entries(), emptyList(), CalendarSettings(), cursor)
                val now = LocalDateTime.now()
                if (next.isEmpty()) { sleep(30_000); continue }
                val at = next[0].time
                if (at.isAfter(now)) {
                    sleep(minOf(30_000L, Duration.between(now, at).toMillis().coerceAtLeast(50)))
                    continue
                }
                cursor = at
                if (Duration.between(at, now).seconds <= 90) onFire(next) else Log.i("skipped late alarm at $at")
            } catch (_: InterruptedException) {
                // 編集通知/終了要求: 次の周回で再計算
            } catch (e: Exception) {
                Log.i("scheduler error: $e")
                try { sleep(5_000) } catch (_: InterruptedException) {}
            }
        }
    }
}
