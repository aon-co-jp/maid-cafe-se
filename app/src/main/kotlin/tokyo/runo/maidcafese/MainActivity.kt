package tokyo.runo.maidcafese

import android.Manifest
import android.content.Intent
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.Checkbox
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import java.time.DayOfWeek
import java.time.format.TextStyle
import java.util.Locale
import java.util.UUID
import tokyo.runo.maidcafese.core.AlarmEntry
import tokyo.runo.maidcafese.core.AlarmKind
import tokyo.runo.maidcafese.core.CalendarSettings
import tokyo.runo.maidcafese.core.Recurrence
import tokyo.runo.maidcafese.core.SoundCatalog
import tokyo.runo.maidcafese.core.SpeechText
import tokyo.runo.maidcafese.core.VoiceStyle

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        Scheduler.reschedule(this)
        setContent { MaterialTheme { App() } }
    }
}

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

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun App() {
    val ctx = LocalContext.current
    val entries = remember { mutableStateListOf<AlarmEntry>().apply { addAll(Store.entries(ctx)) } }
    var calendar by remember { mutableStateOf(Store.calendar(ctx)) }
    var editing by remember { mutableStateOf<AlarmEntry?>(null) }
    var creating by remember { mutableStateOf(false) }
    var preview by remember { mutableStateOf(Scheduler.nextPreview(ctx)) }

    fun persist() {
        Store.saveEntries(ctx, entries.toList())
        Store.saveCalendar(ctx, calendar)
        Scheduler.reschedule(ctx)
        preview = Scheduler.nextPreview(ctx)
    }

    val notifPerm = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) {}
    val calPerm = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        calendar = calendar.copy(enabled = granted)
        persist()
    }
    LaunchedEffect(Unit) {
        if (Build.VERSION.SDK_INT >= 33) notifPerm.launch(Manifest.permission.POST_NOTIFICATIONS)
    }

    Scaffold(
        topBar = { TopAppBar(title = { Text("maid-cafe-se 秘書") }) },
        floatingActionButton = { FloatingActionButton(onClick = { creating = true }) { Text("＋") } },
    ) { pad ->
        LazyColumn(
            Modifier.padding(pad).padding(horizontal = 12.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            item {
                Text("次の予定: " + (preview ?: "なし"), style = MaterialTheme.typography.titleSmall)
            }
            item {
                CalendarCard(
                    calendar,
                    onChange = { calendar = it; persist() },
                    onEnable = { on ->
                        if (on && !CalendarRepo.hasPermission(ctx)) calPerm.launch(Manifest.permission.READ_CALENDAR)
                        else { calendar = calendar.copy(enabled = on); persist() }
                    },
                )
            }
            items(entries, key = { it.id }) { e ->
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
                                        (e.schedule.preNoticeMinutes?.let { " / ${it}分前に予告" } ?: ""),
                                    style = MaterialTheme.typography.bodySmall,
                                )
                            }
                            Switch(checked = e.enabled, onCheckedChange = { on ->
                                val i = entries.indexOfFirst { it.id == e.id }
                                if (i >= 0) { entries[i] = e.copy(enabled = on); persist() }
                            })
                        }
                        Row {
                            TextButton(onClick = { editing = e }) { Text("編集") }
                            TextButton(onClick = { entries.removeAll { it.id == e.id }; persist() }) { Text("削除") }
                        }
                    }
                }
            }
            item { Text("", Modifier.padding(40.dp)) }
        }
    }

    if (creating || editing != null) {
        val initial = editing
        EditorDialog(
            initial = initial,
            onDismiss = { creating = false; editing = null },
            onSave = { saved ->
                val i = entries.indexOfFirst { it.id == saved.id }
                if (i >= 0) entries[i] = saved else entries.add(saved)
                creating = false; editing = null
                persist()
            },
        )
    }
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

@Composable
fun CalendarCard(s: CalendarSettings, onChange: (CalendarSettings) -> Unit, onEnable: (Boolean) -> Unit) {
    Card(Modifier.fillMaxWidth()) {
        Column(Modifier.padding(12.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text("Googleカレンダー連動", style = MaterialTheme.typography.titleMedium)
                    Text("端末に同期済みの予定を、開始時刻に読み上げます", style = MaterialTheme.typography.bodySmall)
                }
                Switch(checked = s.enabled, onCheckedChange = onEnable)
            }
            if (s.enabled) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Checkbox(checked = s.preNotice, onCheckedChange = { onChange(s.copy(preNotice = it)) })
                    Text("${s.preNoticeMinutes}分前に予告する")
                }
                VoicePicker(s.voice) { onChange(s.copy(voice = it)) }
            }
        }
    }
}

@Composable
fun TestButton(kind: AlarmKind, soundId: String, text: String, voice: VoiceStyle) {
    val ctx = LocalContext.current
    OutlinedButton(onClick = {
        val i = Intent(ctx, AlarmService::class.java).setAction(AlarmService.ACTION_TEST)
            .putExtra(AlarmService.EXTRA_VOICE, voice.name)
        if (kind == AlarmKind.SOUND) i.putExtra(AlarmService.EXTRA_SOUND, soundId)
        else i.putExtra(AlarmService.EXTRA_SPEECH, SpeechText.alarm(text.ifBlank { "テストです" }, voice))
        ctx.startForegroundService(i)
    }) { Text("テスト再生") }
}

@Composable
@OptIn(ExperimentalMaterial3Api::class)
fun EditorDialog(initial: AlarmEntry?, onDismiss: () -> Unit, onSave: (AlarmEntry) -> Unit) {
    editorDialogImpl(initial, onDismiss, onSave)
}
