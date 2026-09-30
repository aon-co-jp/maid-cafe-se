package tokyo.runo.maidcafese.core

/**
 * Rust製コア(`crates/maid-cafe-jni`)へのJNI入口。次の発火の計算・読み上げ文・セリフ番号・音声の加工は、すべてRust側にある。
 * 受け渡しは文字列と数値配列だけ(形式は`bridge.rs`の冒頭を参照)。
 *
 * ライブラリの読み込み: Androidでは`libmaid_cafe_jni.so`(Gradleが`cargo ndk`で作ってAPKに入れる)。JVMのテストでは、
 * システムプロパティ`maidcafe.native`のパス(ホスト向けにビルドした`.dll`/`.so`)を読む。
 */
object Native {
    init {
        val path = System.getProperty("maidcafe.native")
        if (path != null) System.load(path) else System.loadLibrary("maid_cafe_jni")
    }

    /** 次に鳴らすもの(同時刻が複数なら複数行)。時刻は「壁時計の秒」(`LocalDateTime.toEpochSecond(UTC)`)。 */
    @JvmStatic external fun planNext(entries: String, events: String, settings: String, after: Long): String

    /** 指定時刻の読み上げ文(全体)。読み上げない設定ならnull。 */
    @JvmStatic external fun alarmSpeech(kind: String, text: String, label: String, voice: String, ids: String): String?

    /** セリフ番号表(`id:番号,...`)の編集。`op`は`add`/`remove`/`assign`。不正な入力はnull。 */
    @JvmStatic external fun phraseEdit(numbers: String, op: String, id: String, n: Int): String?

    /** 番号の小さい順のid(`,`区切り)。 */
    @JvmStatic external fun phraseOrder(numbers: String): String

    /** 保存された順序(喋る順)から`1,2,3…`の番号を振った番号表。 */
    @JvmStatic external fun phraseRanks(ids: String): String

    /** WAVのサンプルレート。読めなければ0。 */
    @JvmStatic external fun wavSampleRate(wav: ByteArray): Int

    /** TTS出力(WAV)1区切りを声質に合わせて加工する。読めない・短すぎるときはnull。 */
    @JvmStatic external fun renderSegment(wav: ByteArray, voice: String, gender: String, harmony: Boolean, pitch: Double): FloatArray?

    /** 区切りを、後ろの間(ミリ秒)を挟んで連結し、16bit PCM(ディザつき)にする。失敗時はnull。 */
    @JvmStatic external fun joinPcm16(parts: Array<FloatArray>, gaps: IntArray, sampleRate: Int): ShortArray?
}
