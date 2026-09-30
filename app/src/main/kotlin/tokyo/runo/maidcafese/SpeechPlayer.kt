package tokyo.runo.maidcafese

import android.content.Context
import android.media.AudioAttributes
import android.media.AudioFormat
import android.media.AudioTrack
import android.os.Bundle
import android.os.Handler
import android.speech.tts.TextToSpeech
import android.speech.tts.UtteranceProgressListener
import android.util.Log
import java.io.File
import java.util.Locale
import tokyo.runo.maidcafese.core.Occurrence
import tokyo.runo.maidcafese.core.Segment
import tokyo.runo.maidcafese.core.VoiceStyle
import tokyo.runo.maidcafese.core.audio.Pcm
import tokyo.runo.maidcafese.core.audio.SourceGender
import tokyo.runo.maidcafese.core.audio.VoiceDsp
import tokyo.runo.maidcafese.core.audio.Wav

/**
 * 読み上げ再生。文(セリフ)ごとに端末のTTSでWAVへ合成し(`synthesizeToFile`)、[VoiceDsp]でピッチ・声の太さを
 * 加工(セリフごとの抑揚つき)、無音トリム・音量統一のうえ、指定の「間」を挟んで連結して1本にして鳴らす。
 * ハモりは同じ音声から3度違いの声を重ねる。合成・解析・加工のどこかで失敗したら、従来の直接読み上げに切り替える。
 */
class SpeechPlayer(
    private val ctx: Context,
    private val main: Handler,
    private val onFinished: () -> Unit,
    private val onUnavailable: (String) -> Unit,
) {
    private companion object {
        const val TAG = "MaidCafeSe"
    }

    private var tts: TextToSpeech? = null
    private var items: List<Occurrence> = emptyList()
    private var index = 0
    private var segs: List<Segment> = emptyList()
    private var parts = ArrayList<FloatArray>()
    private var sampleRate = 0
    private var track: AudioTrack? = null
    private var stopped = false

    fun start(list: List<Occurrence>) {
        items = list
        tts = TextToSpeech(ctx.applicationContext) { status ->
            main.post {
                val engine = tts
                if (stopped || engine == null) return@post
                if (status != TextToSpeech.SUCCESS) { onUnavailable("TTS初期化失敗"); return@post }
                val r = engine.setLanguage(Locale.JAPAN)
                if (r == TextToSpeech.LANG_MISSING_DATA || r == TextToSpeech.LANG_NOT_SUPPORTED) {
                    onUnavailable("日本語音声データがありません"); return@post
                }
                engine.setAudioAttributes(alarmAttrs())
                engine.setOnUtteranceProgressListener(listener)
                next()
            }
        }
    }

    fun stop() {
        stopped = true
        track?.runCatching { stop(); release() }
        track = null
        tts?.runCatching { stop(); shutdown() }
        tts = null
    }

    private fun alarmAttrs() = AudioAttributes.Builder()
        .setUsage(AudioAttributes.USAGE_ALARM)
        .setContentType(AudioAttributes.CONTENT_TYPE_SPEECH)
        .build()

    /** 発話IDは `syn<アイテム>_<区切り>` / `spk<アイテム>`(フォールバック直接読み上げ)。 */
    private val listener = object : UtteranceProgressListener() {
        override fun onStart(id: String?) {}
        override fun onDone(id: String?) {
            when {
                id?.startsWith("syn") == true -> {
                    val (i, k) = id.removePrefix("syn").split("_").map { it.toInt() }
                    Thread { processSegment(i, k) }.start()
                }
                id?.startsWith("spk") == true -> main.post { advance() }
            }
        }
        @Deprecated("Deprecated in Java")
        override fun onError(id: String?) {
            when {
                id?.startsWith("syn") == true ->
                    main.post { fallback(id.removePrefix("syn").split("_")[0].toInt(), "合成失敗") }
                id?.startsWith("spk") == true -> main.post { advance() }
            }
        }
    }

    private fun advance() {
        index++
        next()
    }

    private fun next() {
        if (stopped) return
        while (index < items.size && items[index].speech == null) index++
        if (index >= items.size) { onFinished(); return }
        val item = items[index]
        segs = item.segments.ifEmpty { listOf(Segment(item.speech.orEmpty())) }
        parts = ArrayList()
        synth(0)
    }

    private fun synth(k: Int) {
        if (stopped) return
        val engine = tts ?: return
        val item = items[index]
        val (_, baseRate) = Voices.prepare(engine, item.voice, Store.voiceName(ctx, item.voice), neutralPitch = true)
        engine.setSpeechRate((baseRate * segs[k].rate).coerceIn(0.5f, 2.0f))
        val file = File(ctx.cacheDir, "tts_${index}_$k.wav")
        val res = engine.synthesizeToFile(segs[k].text, Bundle(), file, "syn${index}_$k")
        if (res != TextToSpeech.SUCCESS) fallback(index, "synthesizeToFile失敗")
    }

    /** バックグラウンドスレッド: 1区切りのWAV読み込み→加工。全区切りが揃ったら連結して再生。 */
    private fun processSegment(i: Int, k: Int) {
        if (stopped || i != index || k != parts.size) return
        val item = items[i]
        val file = File(ctx.cacheDir, "tts_${i}_$k.wav")
        try {
            val pcm = Wav.parse(file.readBytes())
            file.delete()
            if (pcm == null || pcm.samples.size < pcm.sampleRate / 20) {
                main.post { fallback(i, "WAV解析不可/空") }
                return
            }
            sampleRate = pcm.sampleRate
            val t0 = System.nanoTime()
            val out = VoiceDsp.render(pcm, item.voice, Voices.genderOf(tts, item.voice), item.harmony, segs[k].pitch)
            Log.i(TAG, "DSP seg ${k + 1}/${segs.size} ${pcm.seconds}s -> ${out.seconds}s in ${(System.nanoTime() - t0) / 1_000_000}ms style=${item.voice} harmony=${item.harmony} pitchMul=${segs[k].pitch} sr=${pcm.sampleRate}")
            parts.add(out.samples)
            if (k + 1 < segs.size) {
                main.post { synth(k + 1) }
            } else {
                val joined = Pcm(VoiceDsp.join(parts, segs.map { it.gapAfterMs }, sampleRate), sampleRate)
                if (ctx.applicationInfo.flags and android.content.pm.ApplicationInfo.FLAG_DEBUGGABLE != 0) {
                    // デバッグビルドのみ: 最終WAVを保存(実機での検証用)
                    File(ctx.cacheDir, "debug_final_${item.voice}_${item.harmony}.wav").writeBytes(Wav.toBytes(joined))
                }
                main.post { play(joined) }
            }
        } catch (e: Exception) {
            Log.e(TAG, "process failed", e)
            main.post { fallback(i, "加工失敗") }
        }
    }

    private fun play(pcm: Pcm) {
        if (stopped) return
        val data = Wav.toPcm16(pcm.samples)
        val t = AudioTrack.Builder()
            .setAudioAttributes(alarmAttrs())
            .setAudioFormat(
                AudioFormat.Builder().setEncoding(AudioFormat.ENCODING_PCM_16BIT)
                    .setSampleRate(pcm.sampleRate).setChannelMask(AudioFormat.CHANNEL_OUT_MONO).build(),
            )
            .setTransferMode(AudioTrack.MODE_STATIC)
            .setBufferSizeInBytes(data.size * 2)
            .build()
        t.write(data, 0, data.size)
        t.setNotificationMarkerPosition(data.size - 1)
        t.setPlaybackPositionUpdateListener(object : AudioTrack.OnPlaybackPositionUpdateListener {
            override fun onMarkerReached(track: AudioTrack?) { done(t) }
            override fun onPeriodicNotification(track: AudioTrack?) {}
        }, main)
        track = t
        t.play()
        // マーカーが来ない端末への保険
        main.postDelayed({ done(t) }, (pcm.seconds * 1000).toLong() + 1500)
    }

    private fun done(t: AudioTrack) {
        if (track !== t) return
        track = null
        t.runCatching { stop(); release() }
        advance()
    }

    private fun fallback(i: Int, reason: String) {
        if (stopped) return
        Log.w(TAG, "fallback to direct speech: $reason")
        val engine = tts ?: return
        val item = items[i]
        val (pitch, rate) = Voices.prepare(engine, item.voice, Store.voiceName(ctx, item.voice), neutralPitch = false)
        engine.setPitch(pitch)
        engine.setSpeechRate(rate)
        engine.speak(item.speech.orEmpty(), TextToSpeech.QUEUE_FLUSH, null, "spk$i")
    }
}

/** 端末のTTS音声の選択(ベストエフォート)。 */
object Voices {
    private val FEMALE_HINTS = listOf("female", "jab", "htm", "-f-")
    private val MALE_HINTS = listOf("jac", "jad", "-m-")

    /** 端末にある日本語の音声(オフラインで使えるもののみ)。 */
    fun japaneseVoices(tts: TextToSpeech?) =
        runCatching { tts?.voices?.filter { it.locale.language == "ja" && !it.isNetworkConnectionRequired } }
            .getOrNull().orEmpty().sortedBy { it.name }

    fun genderOfName(name: String): SourceGender {
        val n = name.lowercase()
        return when {
            FEMALE_HINTS.any { n.contains(it) } -> SourceGender.FEMALE
            MALE_HINTS.any { n.contains(it) } && !n.contains("female") -> SourceGender.MALE
            else -> SourceGender.UNKNOWN
        }
    }

    /**
     * 声質に合うエンジン音声を選んで設定し、(直接読み上げ用ピッチ, 速度)を返す。DSP経路ではピッチを触らない。
     * ユーザーが音声を指定していて端末に存在すれば、それを最優先にする。
     */
    fun prepare(tts: TextToSpeech, style: VoiceStyle, preferredName: String?, neutralPitch: Boolean): Pair<Float, Float> {
        val ja = japaneseVoices(tts)
        val want = if (style == VoiceStyle.MAID) SourceGender.FEMALE else SourceGender.MALE
        (ja.firstOrNull { it.name == preferredName }
            ?: ja.firstOrNull { genderOfName(it.name) == want }
            ?: ja.firstOrNull { genderOfName(it.name) != SourceGender.UNKNOWN })
            ?.let { tts.voice = it }
        tts.setPitch(1f)
        return when (style) {
            VoiceStyle.MAID -> (if (neutralPitch) 1f else 1.4f) to (if (neutralPitch) 1.0f else 1.1f)
            VoiceStyle.DEEP_MALE -> (if (neutralPitch) 1f else 0.55f) to (if (neutralPitch) 0.92f else 0.85f)
        }
    }

    /** 現在エンジンが使っている声の性別(音声名からの推定、不明はUNKNOWN)。 */
    fun genderOf(tts: TextToSpeech?, @Suppress("UNUSED_PARAMETER") style: VoiceStyle): SourceGender =
        runCatching { tts?.voice?.name }.getOrNull()?.let(::genderOfName) ?: SourceGender.UNKNOWN
}
