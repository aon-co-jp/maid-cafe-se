package tokyo.runo.maidcafese

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Checkbox
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TimePicker
import androidx.compose.material3.rememberTimePickerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.neverEqualPolicy
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import java.time.DayOfWeek
import java.time.LocalDate
import java.time.LocalTime
import java.util.UUID
import tokyo.runo.maidcafese.core.AlarmEntry
import tokyo.runo.maidcafese.core.AlarmKind
import tokyo.runo.maidcafese.core.Recurrence
import tokyo.runo.maidcafese.core.Schedule
import tokyo.runo.maidcafese.core.SoundCatalog
import tokyo.runo.maidcafese.core.VoiceStyle

private enum class RecKind(val label: String) {
    DAILY("毎日"), WEEKDAYS("平日"), WEEKENDS("土日祝日"), DOW("曜日指定"), NTH("第N週の曜日"), EVERY("N週間ごと")
}

private fun kindOf(r: Recurrence) = when (r) {
    Recurrence.Daily -> RecKind.DAILY
    is Recurrence.Weekdays -> RecKind.WEEKDAYS
    Recurrence.WeekendsAndHolidays -> RecKind.WEEKENDS
    is Recurrence.DaysOfWeek -> RecKind.DOW
    is Recurrence.NthWeekdayOfMonth -> RecKind.NTH
    is Recurrence.EveryNWeeks -> RecKind.EVERY
}

@Composable
private fun DayChips(selected: Set<DayOfWeek>, onChange: (Set<DayOfWeek>) -> Unit) {
    Row(horizontalArrangement = Arrangement.spacedBy(2.dp)) {
        DayOfWeek.entries.forEach { d ->
            FilterChip(
                selected = d in selected,
                onClick = { onChange(if (d in selected) selected - d else selected + d) },
                label = { Text(dowName(d)) },
            )
        }
    }
}

@Composable
private fun <T> Choice(items: List<T>, selected: T, label: (T) -> String, onPick: (T) -> Unit) {
    Column {
        items.chunked(2).forEach { row ->
            Row(verticalAlignment = Alignment.CenterVertically) {
                row.forEach { item ->
                    RadioButton(selected = item == selected, onClick = { onPick(item) })
                    Text(label(item), Modifier.padding(end = 8.dp))
                }
            }
        }
    }
}

@Composable
@OptIn(ExperimentalMaterial3Api::class)
fun editorDialogImpl(initial: AlarmEntry?, onDismiss: () -> Unit, onSave: (AlarmEntry) -> Unit) {
    val rec0 = initial?.schedule?.recurrence ?: Recurrence.Weekdays()
    val t0 = initial?.schedule?.time ?: LocalTime.of(7, 0)
    var label by remember { mutableStateOf(initial?.label ?: "") }
    val timeState = rememberTimePickerState(t0.hour, t0.minute, is24Hour = true)
    var recKind by remember { mutableStateOf(kindOf(rec0)) }
    var skipHolidays by remember { mutableStateOf((rec0 as? Recurrence.Weekdays)?.skipHolidays ?: true) }
    var days by remember {
        mutableStateOf(
            when (rec0) {
                is Recurrence.DaysOfWeek -> rec0.days
                is Recurrence.EveryNWeeks -> rec0.days
                else -> setOf(DayOfWeek.MONDAY)
            },
        )
    }
    var nth by remember { mutableStateOf((rec0 as? Recurrence.NthWeekdayOfMonth)?.nth ?: 1) }
    var nthDay by remember { mutableStateOf((rec0 as? Recurrence.NthWeekdayOfMonth)?.day ?: DayOfWeek.MONDAY) }
    var interval by remember { mutableStateOf(((rec0 as? Recurrence.EveryNWeeks)?.interval ?: 2).toString()) }
    var kind by remember { mutableStateOf(initial?.kind ?: AlarmKind.SOUND) }
    var soundId by remember { mutableStateOf(initial?.soundId ?: SoundCatalog.DEFAULT_ID) }
    var text by remember { mutableStateOf(initial?.text ?: "") }
    var voice by remember { mutableStateOf(initial?.voice ?: VoiceStyle.MAID) }
    var pre by remember { mutableStateOf(initial?.schedule?.preNoticeMinutes != null) }
    // Setの等価判定は順序を無視するので、並べ替えでも再描画されるよう常に更新扱いにする
    var phrases by remember { mutableStateOf(initial?.phrases ?: emptySet(), neverEqualPolicy()) }
    var prePhrases by remember { mutableStateOf(initial?.prePhrases ?: emptySet(), neverEqualPolicy()) }
    var harmony by remember { mutableStateOf(initial?.harmony ?: false) }

    val intervalNum = interval.toIntOrNull()
    val recurrence: Recurrence? = when (recKind) {
        RecKind.DAILY -> Recurrence.Daily
        RecKind.WEEKDAYS -> Recurrence.Weekdays(skipHolidays)
        RecKind.WEEKENDS -> Recurrence.WeekendsAndHolidays
        RecKind.DOW -> if (days.isEmpty()) null else Recurrence.DaysOfWeek(days)
        RecKind.NTH -> Recurrence.NthWeekdayOfMonth(nth, nthDay)
        RecKind.EVERY -> if (days.isEmpty() || intervalNum == null || intervalNum < 1) null
        else Recurrence.EveryNWeeks(intervalNum, days, (initial?.schedule?.recurrence as? Recurrence.EveryNWeeks)?.anchor ?: LocalDate.now())
    }
    val canSave = recurrence != null && (kind == AlarmKind.SOUND || text.isNotBlank() || label.isNotBlank() || phrases.isNotEmpty())

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(if (initial == null) "アラームを追加" else "アラームを編集") },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(label, { label = it }, label = { Text("名前(例: 起床)") }, modifier = Modifier.fillMaxWidth())
                TimePicker(timeState)
                Text("いつ鳴らす")
                Choice(RecKind.entries, recKind, { it.label }) { recKind = it }
                when (recKind) {
                    RecKind.WEEKDAYS -> Row(verticalAlignment = Alignment.CenterVertically) {
                        Checkbox(skipHolidays, { skipHolidays = it }); Text("祝日は鳴らさない")
                    }
                    RecKind.DOW -> DayChips(days) { days = it }
                    RecKind.NTH -> {
                        Choice(listOf(1, 2, 3, 4, 5, -1), nth, { if (it == -1) "最終" else "第$it" }) { nth = it }
                        Choice(DayOfWeek.entries, nthDay, { dowName(it) + "曜" }) { nthDay = it }
                    }
                    RecKind.EVERY -> {
                        OutlinedTextField(interval, { interval = it.filter(Char::isDigit).take(2) }, label = { Text("何週間ごと") })
                        DayChips(days) { days = it }
                    }
                    else -> {}
                }
                Text("鳴らし方")
                Choice(AlarmKind.entries, kind, { if (it == AlarmKind.SOUND) "音(アラーム)" else "文章の読み上げ" }) { kind = it }
                if (kind == AlarmKind.SOUND) {
                    Choice(SoundCatalog.all, SoundCatalog.all.first { it.id == soundId }, { it.displayName }) { soundId = it.id }
                } else {
                    OutlinedTextField(text, { text = it }, label = { Text("読み上げる文章") }, modifier = Modifier.fillMaxWidth())
                }
                Text("声")
                VoicePicker(voice) { voice = it }
                Text("メイドのセリフ(時間になったら。複数選ぶと続けて喋る)")
                PhrasePicker(phrases) { phrases = it }
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Checkbox(pre, { pre = it }); Text("30分前に予告する")
                }
                if (pre) {
                    Text("メイドのセリフ(30分前の予告に)")
                    PhrasePicker(prePhrases) { prePhrases = it }
                }
                HarmonyCheck(harmony) { harmony = it }
                TestButton(
                    AlarmEntry(
                        "test", label.ifBlank { "アラーム" }, Schedule(Recurrence.Daily, LocalTime.of(0, 0)),
                        kind, soundId, text, voice, phrases = phrases, harmony = harmony,
                    ),
                )
            }
        },
        confirmButton = {
            TextButton(enabled = canSave, onClick = {
                onSave(
                    AlarmEntry(
                        id = initial?.id ?: UUID.randomUUID().toString(),
                        label = label.ifBlank { "アラーム" },
                        schedule = Schedule(
                            recurrence!!, LocalTime.of(timeState.hour, timeState.minute),
                            preNoticeMinutes = if (pre) 30 else null,
                        ),
                        kind = kind, soundId = soundId, text = text, voice = voice,
                        enabled = initial?.enabled ?: true,
                        phrases = phrases, prePhrases = if (pre) prePhrases else emptySet(), harmony = harmony,
                    ),
                )
            }) { Text("保存") }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("キャンセル") } },
    )
}
