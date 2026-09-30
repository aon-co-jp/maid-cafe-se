package tokyo.runo.maidcafese.desktop

import java.io.File
import java.time.LocalDateTime
import java.time.LocalTime
import java.util.concurrent.CopyOnWriteArrayList
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import tokyo.runo.maidcafese.core.AlarmEntry
import tokyo.runo.maidcafese.core.AlarmKind
import tokyo.runo.maidcafese.core.Occurrence
import tokyo.runo.maidcafese.core.Recurrence
import tokyo.runo.maidcafese.core.Schedule
import tokyo.runo.maidcafese.core.VoiceStyle

class DesktopTest {
    @Test fun rateStepMapsMultiplierToSapiRange() {
        assertEquals(0, Sapi.rateStep(1.0))
        assertEquals(10, Sapi.rateStep(3.0))
        assertEquals(-10, Sapi.rateStep(1.0 / 3.0))
        assertEquals(10, Sapi.rateStep(100.0)) // 範囲外は丸める
        assertEquals(-10, Sapi.rateStep(0.001))
        assertTrue(Sapi.rateStep(0.85) < 0 && Sapi.rateStep(1.1) > 0) // ゆっくり=負、速い=正
    }

    @Test fun storeRoundTripsAlarmsAndVoicePicks() {
        val dir = System.getProperty("maidcafese.data")
        // DesktopStoreはシステムプロパティ`maidcafese.data`(build.gradle.ktsのtest設定)を保存先に使う
        assertTrue(dir != null && File(dir).isDirectory, "テスト用の保存先が設定されていない: $dir")
        val e = AlarmEntry(
            "id1", "起床", Schedule(Recurrence.Weekdays(), LocalTime.of(7, 30), preNoticeMinutes = 30),
            AlarmKind.SPEECH, text = "薬を飲む", voice = VoiceStyle.DEEP_MALE,
            phrases = linkedSetOf("perfect", "okite"), prePhrases = linkedSetOf("fight"), harmony = true,
        )
        DesktopStore.saveEntries(listOf(e))
        val back = DesktopStore.entries()
        assertEquals(listOf(e), back)
        assertEquals(listOf("perfect", "okite"), back.single().phrases.toList()) // 喋る順が保たれる
        DesktopStore.saveVoiceName(VoiceStyle.MAID, "Microsoft Haruka Desktop")
        assertEquals("Microsoft Haruka Desktop", DesktopStore.voiceName(VoiceStyle.MAID))
        assertEquals(null, DesktopStore.voiceName(VoiceStyle.DEEP_MALE))
        DesktopStore.saveVoiceName(VoiceStyle.MAID, null)
        assertEquals(null, DesktopStore.voiceName(VoiceStyle.MAID))
    }

    @Test fun schedulerFiresDueAlarmOnceAndSkipsNothingFresh() {
        // 直近の分(いま+1秒後の時刻ではなく、次の分)に鳴るアラームを登録し、鳴った時刻を検証する
        val now = LocalDateTime.now()
        val at = now.plusSeconds(3).withNano(0)
        val entry = AlarmEntry(
            "s1", "テスト", Schedule(Recurrence.Daily, at.toLocalTime().withNano(0)), AlarmKind.SOUND,
        )
        val fired = CopyOnWriteArrayList<List<Occurrence>>()
        val loop = SchedulerLoop({ listOf(entry) }) { fired += it }
        loop.start()
        val deadline = System.currentTimeMillis() + 15_000
        while (fired.isEmpty() && System.currentTimeMillis() < deadline) Thread.sleep(100)
        loop.shutdown()
        assertEquals(1, fired.size, "1回だけ鳴る")
        assertEquals("alarm:s1", fired[0][0].key)
        assertEquals(at, fired[0][0].time)
    }
}
