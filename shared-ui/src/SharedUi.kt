package tokyo.runo.maidcafese

// Android版(app)とWindows版(desktop)で共有する画面部品。プラットフォーム依存(Context等)は持たない。
// 両モジュールの build.gradle.kts が `../shared-ui/src` をソースディレクトリに加えてコンパイルする。

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Card
import androidx.compose.material3.Checkbox
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import java.time.DayOfWeek
import java.time.format.TextStyle
import java.util.Locale
import tokyo.runo.maidcafese.core.AlarmEntry
import tokyo.runo.maidcafese.core.AlarmKind
import tokyo.runo.maidcafese.core.MaidPhrases
import tokyo.runo.maidcafese.core.Recurrence
import tokyo.runo.maidcafese.core.SoundCatalog
import tokyo.runo.maidcafese.core.VoiceStyle

fun dowName(d: DayOfWeek): String = d.getDisplayName(TextStyle.SHORT, Locale.JAPAN)

fun describe(r: Recurrence): String = when (r) {
    Recurrence.Daily -> "毎日"
    is Recurrence.Weekdays -> if (r.skipHolidays) "平日(祝日を除く)" else "平日(月〜金)"
    Recurrence.WeekendsAndHolidays -> "土日祝日"
    is Recurrence.DaysOfWeek -> "毎週 " + r.days.sorted().joinToString("・") { dowName(it) }
    is Recurrence.NthWeekdayOfMonth ->
        "毎月 " + (if (r.nth == -1) "最終" else "第${r.nth}") + dowName(r.day) + "曜日"
    is Recurrence.EveryNWeeks -> "${r.interval}週間ごと " + r.days.sorted().joinToString("・") { dowName(it) } + "曜日"
}

fun voiceName(v: VoiceStyle) = when (v) {
    VoiceStyle.MAID -> "メイドカフェ風"
    VoiceStyle.DEEP_MALE -> "太く低い男性"
}

@Composable
fun VoicePicker(current: VoiceStyle, onPick: (VoiceStyle) -> Unit) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        VoiceStyle.entries.forEach { v ->
            RadioButton(selected = current == v, onClick = { onPick(v) })
            Text(voiceName(v), Modifier.padding(end = 12.dp))
        }
    }
}

/**
 * メイドのセリフの選択。選んだセリフの横に**喋る順の番号**を出し、番号は書き換えられる。番号の小さい順に喋る。
 * 同じ番号を後から別のセリフに入れると、元の持ち主は空いている最小の番号へ自動で振り直される
 * (番号がかぶることは無い。ロジックは[MaidPhrases.assign])。チェックを外すと番号は欠番になり、
 * 付け直すと最後の番号の次に加わる。
 */
@Composable
fun PhrasePicker(selected: Set<String>, onChange: (Set<String>) -> Unit) {
    // 番号はこのピッカーが持つ(保存されるのは番号順に並べたid列)。欠番を保つため、並びだけからは再計算しない
    var nums by remember { mutableStateOf(MaidPhrases.ranks(selected)) }
    fun update(n: Map<String, Int>) {
        nums = n
        onChange(MaidPhrases.ordered(n))
    }
    Column {
        Text("チェックしたセリフを、横の番号の小さい順に喋ります。番号は書き換えられます", style = MaterialTheme.typography.labelSmall)
        MaidPhrases.all.forEach { p ->
            val number = nums[p.id]
            Row(verticalAlignment = Alignment.CenterVertically) {
                Checkbox(
                    checked = number != null,
                    onCheckedChange = { on -> update(if (on) MaidPhrases.add(nums, p.id) else MaidPhrases.remove(nums, p.id)) },
                )
                if (number != null) {
                    var text by remember(number) { mutableStateOf(number.toString()) }
                    OutlinedTextField(
                        value = text,
                        onValueChange = { v ->
                            val digits = v.filter(Char::isDigit).take(2)
                            text = digits
                            digits.toIntOrNull()?.let { n -> if (n != number) update(MaidPhrases.assign(nums, p.id, n)) }
                        },
                        modifier = Modifier.width(64.dp),
                        singleLine = true,
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                        label = { Text("順") },
                    )
                    Text(p.display, style = MaterialTheme.typography.bodySmall, modifier = Modifier.padding(start = 8.dp))
                } else {
                    Text(p.display, style = MaterialTheme.typography.bodySmall)
                }
            }
        }
    }
}

@Composable
fun HarmonyCheck(harmony: Boolean, onChange: (Boolean) -> Unit) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        Checkbox(checked = harmony, onCheckedChange = onChange)
        Text("メイドちゃん2人でハモる(声を重ねる)")
    }
}

/** アラーム1件のカード(一覧用)。 */
@Composable
fun AlarmCard(e: AlarmEntry, onToggle: (Boolean) -> Unit, onEdit: () -> Unit, onDelete: () -> Unit) {
    Card(Modifier.fillMaxWidth()) {
        Column(Modifier.padding(12.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text(e.schedule.time.toString(), style = MaterialTheme.typography.headlineMedium)
                    Text(e.label, style = MaterialTheme.typography.titleMedium)
                    Text(describe(e.schedule.recurrence))
                    Text(
                        (if (e.kind == AlarmKind.SOUND) "音: " + (SoundCatalog.all.firstOrNull { it.id == e.soundId }?.displayName ?: e.soundId)
                        else "読み上げ(" + voiceName(e.voice) + ")") +
                            (e.schedule.preNoticeMinutes?.let { " / ${it}分前に予告" } ?: "") +
                            (if (e.phrases.isNotEmpty() || e.prePhrases.isNotEmpty()) " / メイドのセリフ" else "") +
                            (if (e.harmony) " / ハモり" else ""),
                        style = MaterialTheme.typography.bodySmall,
                    )
                }
                Switch(checked = e.enabled, onCheckedChange = onToggle)
            }
            Row {
                TextButton(onClick = onEdit) { Text("編集") }
                TextButton(onClick = onDelete) { Text("削除") }
            }
        }
    }
}
