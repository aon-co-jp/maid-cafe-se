package tokyo.runo.maidcafese

import android.Manifest
import android.content.Context
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
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Card
import androidx.compose.material3.Checkbox
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
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
import androidx.compose.ui.unit.dp
import tokyo.runo.maidcafese.core.AlarmEntry
import tokyo.runo.maidcafese.core.Codec
import tokyo.runo.maidcafese.core.AlarmKind
import tokyo.runo.maidcafese.core.CalendarSettings
import tokyo.runo.maidcafese.core.SpeechText
import tokyo.runo.maidcafese.core.VoiceStyle
import tokyo.runo.maidcafese.core.audio.SourceGender

// dowName / describe / voiceName / VoicePicker / PhrasePicker / HarmonyCheck / AlarmCard / EditorDialog は
// Android版とWindows版で共有する `shared-ui/src` にある。

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        Scheduler.reschedule(this)
        setContent { MaterialTheme { App() } }
    }
}

/** エディタの「テスト再生」: 保存前の設定をそのままサービスに渡して鳴らす(本番と同じ、Rust製コアのPlannerを通る)。 */
fun startTestPlayback(ctx: Context, entry: AlarmEntry) {
    val e = entry.copy(text = entry.text.ifBlank { if (entry.phrases.isEmpty()) "テストです" else "" })
    ctx.startForegroundService(
        Intent(ctx, AlarmService::class.java).setAction(AlarmService.ACTION_TEST).putExtra(AlarmService.EXTRA_ENTRY, Codec.encode(e)),
    )
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
                AlarmCard(
                    e,
                    onToggle = { on ->
                        val i = entries.indexOfFirst { it.id == e.id }
                        if (i >= 0) { entries[i] = e.copy(enabled = on); persist() }
                    },
                    onEdit = { editing = e },
                    onDelete = { entries.removeAll { it.id == e.id }; persist() },
                )
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
            onTest = { startTestPlayback(ctx, it) },
        )
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
                                SourceGender.FEMALE -> "(女性の声)"
                                SourceGender.MALE -> "(男性の声)"
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
