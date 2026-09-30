package tokyo.runo.maidcafese.core

import java.time.DayOfWeek
import java.time.LocalDate
import java.time.LocalTime

/**
 * 「どの日に鳴らすか」の定型ルール(画面・保存用のデータ)。**どの日に当たるかの判定と、次の発火時刻の計算はRust側**
 * (`maid-cafe-core`の`Recurrence`/`Schedule`、日本の祝日つき)。
 */
sealed interface Recurrence {
    /** 毎日。 */
    data object Daily : Recurrence

    /** 平日(月〜金)。[skipHolidays]=true なら祝日を除く。 */
    data class Weekdays(val skipHolidays: Boolean = true) : Recurrence

    /** 土日祝日(平日ルールの補集合、祝日を休みとして扱う)。 */
    data object WeekendsAndHolidays : Recurrence

    /** 曜日指定(複数可)。 */
    data class DaysOfWeek(val days: Set<DayOfWeek>) : Recurrence {
        init { require(days.isNotEmpty()) { "曜日を1つ以上指定してください" } }
    }

    /** 毎月の第[nth]週の[day]曜日。[nth]は1..5、最終週は-1。存在しない月(第5など)は鳴らさない。 */
    data class NthWeekdayOfMonth(val nth: Int, val day: DayOfWeek) : Recurrence {
        init { require(nth in 1..5 || nth == -1) { "nthは1..5または-1(最終): $nth" } }
    }

    /** [interval]週間ごとの[days]曜日。基準週は[anchor]を含む週(月曜始まり)。 */
    data class EveryNWeeks(val interval: Int, val days: Set<DayOfWeek>, val anchor: LocalDate) : Recurrence {
        init {
            require(interval >= 1) { "intervalは1以上: $interval" }
            require(days.isNotEmpty()) { "曜日を1つ以上指定してください" }
        }
    }
}

/** 何時に鳴らすか+有効期間。[preNoticeMinutes]が非nullなら、その分前に予告も鳴らす。 */
data class Schedule(
    val recurrence: Recurrence,
    val time: LocalTime,
    val startDate: LocalDate? = null,
    val endDate: LocalDate? = null,
    val preNoticeMinutes: Int? = null,
) {
    init { require(preNoticeMinutes == null || preNoticeMinutes > 0) { "予告は1分以上前" } }
}
