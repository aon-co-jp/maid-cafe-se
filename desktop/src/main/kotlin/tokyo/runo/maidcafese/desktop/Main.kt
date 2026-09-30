package tokyo.runo.maidcafese.desktop

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
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
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
import androidx.compose.ui.graphics.painter.BitmapPainter
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.graphics.toComposeImageBitmap
import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Notification
import androidx.compose.ui.window.Tray
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.application
import androidx.compose.ui.window.rememberTrayState
import androidx.compose.ui.window.rememberWindowState
import java.awt.Color
import java.awt.Font
import java.awt.RenderingHints
import java.awt.image.BufferedImage
import java.io.File
import java.nio.channels.FileChannel
import java.nio.channels.OverlappingFileLockException
import java.nio.file.StandardOpenOption
import java.time.LocalDateTime
import java.util.concurrent.Executors
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.withContext
import kotlin.system.exitProcess
import tokyo.runo.maidcafese.AlarmCard
import tokyo.runo.maidcafese.EditorDialog
import tokyo.runo.maidcafese.core.AlarmEntry
import tokyo.runo.maidcafese.core.CalendarSettings
import tokyo.runo.maidcafese.core.Occurrence
import tokyo.runo.maidcafese.core.Planner
import tokyo.runo.maidcafese.core.SpeechText
import tokyo.runo.maidcafese.core.VoiceStyle
import tokyo.runo.maidcafese.core.audio.SourceGender
import tokyo.runo.maidcafese.voiceName

/** アプリ全体の状態。画面(Compose)とスケジューラスレッドの橋渡し。 */
class Controller {
    val entries = mutableStateListOf<AlarmEntry>().apply { addAll(DesktopStore.entries()) }
    val runner = AlarmRunner()
    var playing by mutableStateOf<String?>(null)
    var notify: (String) -> Unit = {}
    private val player = Executors.newSingleThreadExecutor { r -> Thread(r, "maid-cafe-se-player").apply { isDaemon = true } }
    val scheduler = SchedulerLoop({ entries.toList() }) { fire(it) }

    private fun fire(list: List<Occurrence>) {
        val title = list.joinToString(" / ") { it.title }
        Log.i("fire: $title at ${list[0].time}")
        notify(title)
        player.execute {
            playing = title
            try { runner.play(list) } finally { playing = null }
        }
    }

    /** エディタの「テスト再生」。保存前の設定をそのまま鳴らす。 */
    fun test(e: AlarmEntry) {
        val speech = SpeechText.alarmSpeech(
            e.kind, e.text.ifBlank { if (e.phrases.isEmpty()) "テストです" else "" }, e.label, e.voice, e.phrases,
        )
        val segments = SpeechText.segments(
            SpeechText.alarmBase(e.kind, e.text.ifBlank { if (e.phrases.isEmpty()) "テストです" else "" }, e.label, e.voice, e.phrases),
            e.phrases,
        )
        player.execute {
            playing = "テスト"
            try {
                runner.play(
                    listOf(
                        Occurrence(
                            LocalDateTime.now(), "test", "テスト", if (e.kind == tokyo.runo.maidcafese.core.AlarmKind.SOUND) e.soundId else null,
                            speech, e.voice, e.harmony, segments,
                        ),
                    ),
                )
            } finally { playing = null }
        }
    }

    fun persist() {
        DesktopStore.saveEntries(entries.toList())
        scheduler.changed()
    }
}

/** 常駐アイコン。ファイルを持たず、その場で描く。 */
val appIcon: Painter by lazy {
    val img = BufferedImage(64, 64, BufferedImage.TYPE_INT_ARGB)
    val g = img.createGraphics()
    g.setRenderingHint(RenderingHints.KEY_ANTIALIASING, RenderingHints.VALUE_ANTIALIAS_ON)
    g.setRenderingHint(RenderingHints.KEY_TEXT_ANTIALIASING, RenderingHints.VALUE_TEXT_ANTIALIAS_ON)
    g.color = Color(0xE8, 0x5D, 0x9E)
    g.fillOval(2, 2, 60, 60)
    g.color = Color.WHITE
    g.font = Font(Font.SANS_SERIF, Font.BOLD, 38)
    val fm = g.fontMetrics
    g.drawString("M", (64 - fm.stringWidth("M")) / 2, (64 - fm.height) / 2 + fm.ascent)
    g.dispose()
    BitmapPainter(img.toComposeImageBitmap())
}

/** Windows起動時の自動起動(スタートアップフォルダのショートカット)。インストール版(maid-cafe-se.exe)でのみ有効。 */
object Autostart {
    private val lnk = File(System.getenv("APPDATA") ?: "", "Microsoft\\Windows\\Start Menu\\Programs\\Startup\\maid-cafe-se.lnk")
    private val exe: String? = ProcessHandle.current().info().command().orElse(null)?.takeIf { it.endsWith("maid-cafe-se.exe", true) }

    val available: Boolean get() = exe != null
    fun enabled(): Boolean = lnk.isFile

    fun set(on: Boolean) {
        if (!on) { lnk.delete(); return }
        val target = exe ?: return
        val ps = "\$ws = New-Object -ComObject WScript.Shell; \$sc = \$ws.CreateShortcut('${lnk.path.replace("'", "''")}'); " +
            "\$sc.TargetPath = '${target.replace("'", "''")}'; \$sc.Arguments = '--minimized'; " +
            "\$sc.WorkingDirectory = '${File(target).parent.replace("'", "''")}'; \$sc.Save()"
        ProcessBuilder("powershell.exe", "-NoProfile", "-NonInteractive", "-Command", ps).start().waitFor()
    }
}

/** 二重起動を防ぐ(2つ動くとアラームが二重に鳴るため)。取れなければnull。 */
private fun acquireSingleInstanceLock(): FileChannel? {
    val ch = FileChannel.open(File(DesktopStore.dir, "lock").toPath(), StandardOpenOption.CREATE, StandardOpenOption.WRITE)
    return try { if (ch.tryLock() != null) ch else { ch.close(); null } } catch (e: OverlappingFileLockException) { ch.close(); null }
}

fun main(args: Array<String>) {
    if (args.firstOrNull() == "--selftest") exitProcess(SelfTest.run(args.getOrNull(1), play = "--play" in args))
    if (args.firstOrNull() == "--autostart") { // maid-cafe-se.exe --autostart on|off : 自動起動の設定(スクリプト/動作確認用)
        val on = args.getOrNull(1) == "on"
        Autostart.set(on)
        exitProcess(if (Autostart.enabled() == on) 0 else 1)
    }
    val lock = acquireSingleInstanceLock() ?: run { Log.i("already running, exit"); return }
    val ctl = Controller()
    ctl.scheduler.start()
    val minimized = "--minimized" in args
    Log.i("started (minimized=$minimized, entries=${ctl.entries.size})")
    application {
        var visible by remember { mutableStateOf(!minimized) }
        val trayState = rememberTrayState()
        ctl.notify = { trayState.sendNotification(Notification("maid-cafe-se", it)) }
        Tray(
            icon = appIcon,
            state = trayState,
            tooltip = "maid-cafe-se 秘書",
            onAction = { visible = true },
            menu = {
                Item("開く") { visible = true }
                Item("止める") { ctl.runner.stop() }
                Item("終了") { ctl.scheduler.shutdown(); lock.close(); exitApplication() }
            },
        )
        Window(
            onCloseRequest = { visible = false }, // 閉じてもトレイに残ってアラームを鳴らし続ける
            visible = visible,
            title = "maid-cafe-se 秘書",
            icon = appIcon,
            state = rememberWindowState(size = DpSize(520.dp, 860.dp)),
        ) {
            MaterialTheme { DesktopApp(ctl) }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DesktopApp(ctl: Controller) {
    var editing by remember { mutableStateOf<AlarmEntry?>(null) }
    var creating by remember { mutableStateOf(false) }
    var preview by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(ctl.entries.toList()) {
        while (true) {
            val n = Planner.next(ctl.entries.toList(), emptyList(), CalendarSettings(), LocalDateTime.now())
            preview = n.firstOrNull()?.let { "${it.time.toLocalDate()} ${it.time.toLocalTime()}  ${it.title}" }
            delay(15_000)
        }
    }

    Scaffold(
        topBar = { TopAppBar(title = { Text("maid-cafe-se 秘書") }) },
        floatingActionButton = { FloatingActionButton(onClick = { creating = true }) { Text("＋") } },
    ) { pad ->
        LazyColumn(Modifier.padding(pad).padding(horizontal = 12.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            item {
                Text("次の予定: " + (preview ?: "なし"), style = MaterialTheme.typography.titleSmall)
                ctl.playing?.let { p ->
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text("再生中: $p", Modifier.weight(1f))
                        Button(onClick = { ctl.runner.stop() }) { Text("止める") }
                    }
                }
                Text(
                    "ウィンドウを閉じてもタスクトレイに残り、時刻になると鳴ります。完全に止めるにはトレイのアイコンから「終了」してください",
                    style = MaterialTheme.typography.labelSmall,
                )
            }
            item { VoiceSettingsCard() }
            item { AutostartCard() }
            items(ctl.entries, key = { it.id }) { e ->
                AlarmCard(
                    e,
                    onToggle = { on ->
                        val i = ctl.entries.indexOfFirst { it.id == e.id }
                        if (i >= 0) { ctl.entries[i] = e.copy(enabled = on); ctl.persist() }
                    },
                    onEdit = { editing = e },
                    onDelete = { ctl.entries.removeAll { it.id == e.id }; ctl.persist() },
                )
            }
            item { Text("", Modifier.padding(40.dp)) }
        }
    }

    if (creating || editing != null) {
        EditorDialog(
            initial = editing,
            onDismiss = { creating = false; editing = null },
            onSave = { saved ->
                val i = ctl.entries.indexOfFirst { it.id == saved.id }
                // 並べ替えだけの変更はdata classの等価判定で「同じ」になるため、置換ではなく削除+挿入で確実に反映する
                if (i >= 0) { ctl.entries.removeAt(i); ctl.entries.add(i, saved) } else ctl.entries.add(saved)
                creating = false; editing = null
                ctl.persist()
            },
            onTest = { ctl.test(it) },
        )
    }
}

/** 声の元になるWindows音声(日本語)を、声質ごとに選ぶ。未選択は自動。 */
@Composable
fun VoiceSettingsCard() {
    var voices by remember { mutableStateOf<List<SapiVoice>?>(null) }
    var picks by remember { mutableStateOf(VoiceStyle.entries.associateWith { DesktopStore.voiceName(it) }) }
    LaunchedEffect(Unit) { voices = withContext(Dispatchers.IO) { Sapi.listVoices().filter { it.isJapanese } } }
    Card(Modifier.fillMaxWidth()) {
        Column(Modifier.padding(12.dp)) {
            Text("声の元になるWindowsの音声", style = MaterialTheme.typography.titleMedium)
            Text(
                "読み上げの声はこの音声を加工して作ります。日本語音声が複数あると選べます",
                style = MaterialTheme.typography.bodySmall,
            )
            val list = voices
            if (list == null) {
                Text("音声を確認中…")
            } else if (list.isEmpty()) {
                Text("日本語の音声が見つかりません。Windowsの「設定 → 時刻と言語 → 言語と地域」で日本語を追加し、音声を入れてください")
            } else {
                VoiceStyle.entries.forEach { style ->
                    Text(voiceName(style), style = MaterialTheme.typography.titleSmall, modifier = Modifier.padding(top = 6.dp))
                    fun pick(n: String?) { DesktopStore.saveVoiceName(style, n); picks = picks + (style to n) }
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        RadioButton(selected = picks[style] == null, onClick = { pick(null) })
                        Text("自動")
                    }
                    list.forEach { v ->
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            RadioButton(selected = picks[style] == v.name, onClick = { pick(v.name) })
                            val g = when (v.gender) { SourceGender.FEMALE -> "(女性の声)"; SourceGender.MALE -> "(男性の声)"; else -> "" }
                            Text("${v.name} $g", style = MaterialTheme.typography.bodySmall)
                        }
                    }
                }
            }
        }
    }
}

@Composable
fun AutostartCard() {
    var on by remember { mutableStateOf(Autostart.enabled()) }
    Card(Modifier.fillMaxWidth()) {
        Row(Modifier.padding(12.dp), verticalAlignment = Alignment.CenterVertically) {
            Checkbox(
                checked = on,
                enabled = Autostart.available,
                onCheckedChange = { Autostart.set(it); on = Autostart.enabled() },
            )
            Column {
                Text("Windowsの起動時に自動で起動する(トレイに常駐)")
                if (!Autostart.available) {
                    Text("インストール版(maid-cafe-se.exe)でのみ設定できます", style = MaterialTheme.typography.labelSmall)
                }
            }
        }
    }
}
