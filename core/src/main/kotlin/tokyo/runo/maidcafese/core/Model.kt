package tokyo.runo.maidcafese.core

import java.time.LocalDateTime

/** 読み上げの声。MAID=メイドカフェ風(高め・口調変換)、DEEP_MALE=太くて低い男性。 */
enum class VoiceStyle { MAID, DEEP_MALE }

/** 鳴らし方。SOUND=著作権フリー音、SPEECH=入力文章の読み上げ。 */
enum class AlarmKind { SOUND, SPEECH }

/** ユーザーが登録する定型アラーム1件。[schedule]の予告設定(30分前チェック)が予告の有無を決める。 */
data class AlarmEntry(
    val id: String,
    val label: String,
    val schedule: Schedule,
    val kind: AlarmKind,
    val soundId: String = SoundCatalog.DEFAULT_ID,
    val text: String = "",
    val voice: VoiceStyle = VoiceStyle.MAID,
    val enabled: Boolean = true,
    /** 指定時刻に喋るメイドのセリフ([MaidPhrases]のid、複数可)。**選んだ順**に喋る(Setの反復順を保つ)。 */
    val phrases: Set<String> = emptySet(),
    /** 予告時に喋るメイドのセリフ。 */
    val prePhrases: Set<String> = emptySet(),
    /** メイドちゃん2人でハモる(同じ文を2つの声で同時に)。 */
    val harmony: Boolean = false,
)

/** Googleカレンダー(端末同期分)の予定1件。 */
data class CalendarEvent(val id: String, val title: String, val start: LocalDateTime)

/** カレンダー連動の設定。[preNotice]が「30分前に予告」チェックボックス。 */
data class CalendarSettings(
    val enabled: Boolean = false,
    val preNotice: Boolean = true,
    val preNoticeMinutes: Int = 30,
    val voice: VoiceStyle = VoiceStyle.MAID,
    val phrases: Set<String> = emptySet(),
    val prePhrases: Set<String> = emptySet(),
    val harmony: Boolean = false,
) {
    init { require(preNoticeMinutes > 0) { "予告は1分以上前" } }
}

/** 同梱する音源(すべて本プロジェクトで生成したCC0、詳細は`tools/gen-sounds`(Rust))。 */
object SoundCatalog {
    data class Sound(val id: String, val displayName: String)

    val all = listOf(
        Sound("chime", "チャイム"),
        Sound("alarm", "アラーム(ピピピ)"),
        Sound("melody", "やさしいメロディ"),
    )
    const val DEFAULT_ID = "chime"
    fun isKnown(id: String) = all.any { it.id == id }
}

/** 1回の発火。[soundId]非nullなら音を鳴らし、[speech]非nullなら[voice]で読み上げる。 */
data class Occurrence(
    val time: LocalDateTime,
    val key: String,
    val title: String,
    val soundId: String?,
    val speech: String?,
    val voice: VoiceStyle,
    val harmony: Boolean = false,
    /** 読み上げの区切り(文ごとの間・抑揚)。空なら[speech]を1文として読む。 */
    val segments: List<Segment> = emptyList(),
)

/**
 * 読み上げの1区切り。[pitch]は声全体の音程への倍率、[rate]は話速への倍率、[gapAfterMs]は後ろの間(ミリ秒)。
 */
data class Segment(val text: String, val pitch: Double = 1.0, val rate: Float = 1.0f, val gapAfterMs: Int = 0)

