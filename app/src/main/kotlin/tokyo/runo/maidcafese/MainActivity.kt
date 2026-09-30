package tokyo.runo.maidcafese

import android.Manifest
import android.content.Intent
import android.os.Build
import android.os.Bundle
import android.speech.tts.TextToSpeech
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.Checkbox
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.neverEqualPolicy
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import java.time.DayOfWeek
import java.time.format.TextStyle
import java.util.Locale
import java.util.UUID
import tokyo.runo.maidcafese.core.AlarmEntry
import tokyo.runo.maidcafese.core.AlarmKind
import tokyo.runo.maidcafese.core.CalendarSettings
import tokyo.runo.maidcafese.core.MaidPhrases
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
    // Setの等価判定は順序を無視する(=セリフの並べ替えだけでは変更とみなされない)ので、常に更新扱いにする
    var calendar by remember { mutableStateOf(Store.calendar(ctx), neverEqualPolicy()) }
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
            item { VoiceSettingsCard() }
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
                                        (e.schedule.preNoticeMinutes?.let { " / ${it}分前に予告" } ?: "") +
                                        (if (e.phrases.isNotEmpty() || e.prePhrases.isNotEmpty()) " / メイドのセリフ" else "") +
                                        (if (e.harmony) " / ハモり" else ""),
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
                // 並べ替えだけの変更は data class の等価判定では「同じ」になるため、置換ではなく削除+挿入で確実に反映する
                if (i >= 0) { entries.removeAt(i); entries.add(i, saved) } else entries.add(saved)
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
                Text("メイドのセリフ(予定の時刻に)", style = MaterialTheme.typography.titleSmall)
                PhrasePicker(s.phrases) { onChange(s.copy(phrases = it)) }
                if (s.preNotice) {
                    Text("メイドのセリフ(${s.preNoticeMinutes}分前の予告に)", style = MaterialTheme.typography.titleSmall)
                    PhrasePicker(s.prePhrases) { onChange(s.copy(prePhrases = it)) }
                }
                HarmonyCheck(s.harmony) { onChange(s.copy(harmony = it)) }
            }
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

@Composable
fun TestButton(entry: AlarmEntry) {
    val ctx = LocalContext.current
    OutlinedButton(onClick = {
        val i = Intent(ctx, AlarmService::class.java).setAction(AlarmService.ACTION_TEST)
            .putExtra(AlarmService.EXTRA_VOICE, entry.voice.name)
            .putExtra(AlarmService.EXTRA_HARMONY, entry.harmony)
        if (entry.kind == AlarmKind.SOUND) i.putExtra(AlarmService.EXTRA_SOUND, entry.soundId)
        val speech = SpeechText.alarmSpeech(
            entry.kind, entry.text.ifBlank { if (entry.phrases.isEmpty()) "テストです" else "" }, entry.label, entry.voice, entry.phrases,
        )
        if (speech != null) i.putExtra(AlarmService.EXTRA_SPEECH, speech)
        ctx.startForegroundService(i)
    }) { Text("テスト再生") }
}

@Composable
@OptIn(ExperimentalMaterial3Api::class)
fun EditorDialog(initial: AlarmEntry?, onDismiss: () -> Unit, onSave: (AlarmEntry) -> Unit) {
    editorDialogImpl(initial, onDismiss, onSave)
}


/** 声の元になる端末の日本語音声を、声質(メイド風/低い男性)ごとに選ぶ。未選択は自動選択。 */
@Composable
fun VoiceSettingsCard() {
    val ctx = LocalContext.current
    var names by remember { mutableStateOf<List<String>>(emptyList()) }
    var loaded by remember { mutableStateOf(false) }
    var picks by remember { mutableStateOf(VoiceStyle.entries.associateWith { Store.voiceName(ctx, it) }) }
    DisposableEffect(Unit) {
        var engine: TextToSpeech? = null
        engine = TextToSpeech(ctx.applicationContext) { status ->
            if (status == TextToSpeech.SUCCESS) names = Voices.japaneseVoices(engine).map { it.name }
            loaded = true
        }
        onDispose { engine.shutdown() }
    }
    Card(Modifier.fillMaxWidth()) {
        Column(Modifier.padding(12.dp)) {
            Text("声の元になる端末の音声", style = MaterialTheme.typography.titleMedium)
            Text(
                "読み上げの声はこの音声を加工して作ります。音声が複数ある端末では選ぶと変わります",
                style = MaterialTheme.typography.bodySmall,
            )
            if (!loaded) {
                Text("音声を確認中…")
            } else if (names.isEmpty()) {
                Text("この端末にはオフラインの日本語音声が見つかりません(設定アプリのテキスト読み上げから日本語音声データを入れてください)")
            } else {
                VoiceStyle.entries.forEach { style ->
                    Text(voiceName(style), style = MaterialTheme.typography.titleSmall, modifier = Modifier.padding(top = 6.dp))
                    fun pick(n: String?) {
                        Store.saveVoiceName(ctx, style, n)
                        picks = picks + (style to n)
                    }
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        RadioButton(selected = picks[style] == null, onClick = { pick(null) })
                        Text("自動")
                    }
                    names.forEach { n ->
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            RadioButton(selected = picks[style] == n, onClick = { pick(n) })
                            val g = when (Voices.genderOfName(n)) {
                                tokyo.runo.maidcafese.core.audio.SourceGender.FEMALE -> "(女性の声)"
                                tokyo.runo.maidcafese.core.audio.SourceGender.MALE -> "(男性の声)"
                                else -> ""
                            }
                            Text("$n $g", style = MaterialTheme.typography.bodySmall)
                        }
                    }
                }
            }
        }
    }
}
