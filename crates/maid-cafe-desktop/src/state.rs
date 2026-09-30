//! アプリ全体の状態。画面(API)・スケジューラ・トレイが共有する。

use crate::player::Player;
use crate::sapi::{self, SapiVoice};
use crate::store::Store;
use chrono::{Local, NaiveDateTime};
use maid_cafe_core::{AlarmEntry, AlarmKind, CalendarSettings, JapaneseHolidays, Occurrence, Planner, SpeechText, VoiceStyle};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

pub struct AppState {
    pub store: Arc<Store>,
    pub player: Arc<Player>,
    entries: RwLock<Vec<AlarmEntry>>,
    /// アラームが編集されたことを、スケジューラへ知らせる(待機中でも、すぐ再計算させる)
    pub changed: tokio::sync::Notify,
    voices: Mutex<Option<(Instant, Vec<SapiVoice>)>>,
    port: AtomicU16,
    exe: PathBuf,
}

impl AppState {
    pub fn new(store: Arc<Store>) -> Arc<Self> {
        let entries = store.load_entries();
        let player = Arc::new(Player::new(store.clone()));
        Arc::new(AppState {
            store,
            player,
            entries: RwLock::new(entries),
            changed: tokio::sync::Notify::new(),
            voices: Mutex::new(None),
            port: AtomicU16::new(0),
            exe: std::env::current_exe().unwrap_or_else(|_| PathBuf::from("maid-cafe-se.exe")),
        })
    }

    pub fn set_port(&self, p: u16) {
        self.port.store(p, Ordering::SeqCst);
    }

    pub fn port(&self) -> u16 {
        self.port.load(Ordering::SeqCst)
    }

    pub fn entries(&self) -> Vec<AlarmEntry> {
        self.entries.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn persist(&self, list: &[AlarmEntry]) -> Result<(), String> {
        self.store.save_entries(list).map_err(|e| format!("保存できませんでした: {e}"))?;
        self.changed.notify_one();
        Ok(())
    }

    /// 追加または(同じidがあれば)置き換える。
    pub fn upsert(&self, e: AlarmEntry) -> Result<(), String> {
        let mut g = self.entries.write().unwrap_or_else(|e| e.into_inner());
        match g.iter().position(|x| x.id == e.id) {
            Some(i) => g[i] = e,
            None => g.push(e),
        }
        self.persist(&g)
    }

    pub fn delete(&self, id: &str) -> Result<(), String> {
        let mut g = self.entries.write().unwrap_or_else(|e| e.into_inner());
        g.retain(|x| x.id != id);
        self.persist(&g)
    }

    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<(), String> {
        let mut g = self.entries.write().unwrap_or_else(|e| e.into_inner());
        let e = g.iter_mut().find(|x| x.id == id).ok_or("アラームが見つかりません")?;
        e.enabled = enabled;
        self.persist(&g)
    }

    /// 直近の予定(画面表示用)。
    pub fn next_preview(&self) -> Option<(NaiveDateTime, String)> {
        let now = Local::now().naive_local();
        Planner::next(&self.entries(), &[], &CalendarSettings::default(), now, &JapaneseHolidays)
            .into_iter()
            .next()
            .map(|o| (o.time, o.title))
    }

    /// インストール済みのWindows音声(10分キャッシュ。空なら30秒)。取得は遅い(PowerShell起動)ので、呼び出し側は
    /// `spawn_blocking`で呼ぶこと。
    pub fn voices(&self) -> Vec<SapiVoice> {
        {
            let g = self.voices.lock().unwrap_or_else(|e| e.into_inner());
            if let Some((at, v)) = g.as_ref() {
                let ttl = if v.is_empty() { Duration::from_secs(30) } else { Duration::from_secs(600) };
                if at.elapsed() < ttl {
                    return v.clone();
                }
            }
        }
        let listed = sapi::list_voices();
        *self.voices.lock().unwrap_or_else(|e| e.into_inner()) = Some((Instant::now(), listed.clone()));
        listed
    }

    pub fn voice_pref(&self, style: VoiceStyle) -> Option<String> {
        self.store.voice_pref(style)
    }

    /// エディタの「テスト再生」用: 保存前の設定を、そのまま鳴らす1件の発火にする。
    pub fn test_occurrence(e: &AlarmEntry) -> Occurrence {
        let text = if e.text.trim().is_empty() && e.phrases.is_empty() { "テストです".to_string() } else { e.text.clone() };
        let base = SpeechText::alarm_base(e.kind, &text, &e.label, e.voice, &e.phrases);
        Occurrence {
            time: Local::now().naive_local(),
            key: "test".into(),
            title: "テスト".into(),
            sound_id: if e.kind == AlarmKind::Sound || e.speech_sound { Some(e.sound_id.clone()) } else { None },
            speech: SpeechText::alarm_speech(e.kind, &text, &e.label, e.voice, &e.phrases),
            voice: e.voice,
            harmony: e.harmony,
            segments: SpeechText::segments(base.as_deref(), &e.phrases),
        }
    }

    // ---- Windows起動時の自動起動(レジストリのRunキー) ----

    const RUN_KEY: &'static str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
    const RUN_NAME: &'static str = "maid-cafe-se";

    pub fn autostart_enabled(&self) -> bool {
        #[cfg(windows)]
        {
            reg(&["query", Self::RUN_KEY, "/v", Self::RUN_NAME])
        }
        #[cfg(not(windows))]
        {
            false
        }
    }

    /// 自動起動を設定/解除する(タスクトレイに常駐して起動する`--minimized`)。
    pub fn set_autostart(&self, on: bool) -> Result<(), String> {
        #[cfg(windows)]
        {
            let ok = if on {
                let value = format!("\"{}\" --minimized", self.exe.display());
                reg(&["add", Self::RUN_KEY, "/v", Self::RUN_NAME, "/t", "REG_SZ", "/d", &value, "/f"])
            } else {
                // 元から無い場合も成功扱い(`reg delete`は無いとエラーになる)
                !self.autostart_enabled() || reg(&["delete", Self::RUN_KEY, "/v", Self::RUN_NAME, "/f"])
            };
            if ok { Ok(()) } else { Err("自動起動の設定を変更できませんでした".into()) }
        }
        #[cfg(not(windows))]
        {
            let _ = (on, &self.exe);
            Err("この環境(Windows以外)では自動起動を設定できません".into())
        }
    }
}

#[cfg(windows)]
fn reg(args: &[&str]) -> bool {
    use std::os::windows::process::CommandExt;
    std::process::Command::new("reg.exe")
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .creation_flags(0x0800_0000)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use maid_cafe_core::{Recurrence, Schedule};

    fn state(tag: &str) -> Arc<AppState> {
        let dir = std::env::temp_dir().join(format!("mcs-state-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        AppState::new(Arc::new(Store::new(dir).unwrap()))
    }

    fn entry(id: &str, h: u32) -> AlarmEntry {
        let t = chrono::NaiveTime::from_hms_opt(h, 0, 0).unwrap();
        AlarmEntry::new(id, "テスト", Schedule::new(Recurrence::Daily, t), AlarmKind::Sound)
    }

    #[test]
    fn upsert_replaces_by_id_and_persists() {
        let s = state("upsert");
        s.upsert(entry("a", 7)).unwrap();
        s.upsert(entry("b", 8)).unwrap();
        let mut a2 = entry("a", 9);
        a2.label = "更新後".into();
        s.upsert(a2.clone()).unwrap();
        let list = s.entries();
        assert_eq!(2, list.len());
        assert_eq!(a2, list[0]); // 位置は保たれる
        // 再起動(=保存されたファイルからの読み込み)で同じ内容
        assert_eq!(list, s.store.load_entries());
        let _ = std::fs::remove_dir_all(s.store.dir());
    }

    #[test]
    fn delete_and_toggle() {
        let s = state("delete");
        s.upsert(entry("a", 7)).unwrap();
        s.upsert(entry("b", 8)).unwrap();
        s.set_enabled("a", false).unwrap();
        assert!(!s.entries()[0].enabled);
        assert!(s.set_enabled("nope", true).is_err());
        s.delete("a").unwrap();
        assert_eq!(vec!["b".to_string()], s.entries().iter().map(|e| e.id.clone()).collect::<Vec<_>>());
        s.delete("nope").unwrap(); // 無いidの削除は何も起きない
        assert_eq!(1, s.entries().len());
        let _ = std::fs::remove_dir_all(s.store.dir());
    }

    #[test]
    fn next_preview_ignores_disabled_entries() {
        let s = state("preview");
        assert!(s.next_preview().is_none());
        let mut e = entry("a", 7);
        e.enabled = false;
        s.upsert(e).unwrap();
        assert!(s.next_preview().is_none());
        s.set_enabled("a", true).unwrap();
        assert_eq!("テスト", s.next_preview().unwrap().1);
        let _ = std::fs::remove_dir_all(s.store.dir());
    }

    #[test]
    fn test_occurrence_builds_speech_with_phrases_in_order() {
        let mut e = entry("t", 7);
        e.kind = AlarmKind::Speech;
        e.phrases = vec!["okaeri".into(), "okite".into()];
        e.harmony = true;
        let o = AppState::test_occurrence(&e);
        let sp = o.speech.unwrap();
        assert!(sp.starts_with("おかえりなさいませ"), "{sp}"); // テキストが空でセリフだけなら、セリフのみ
        assert!(sp.find("おーきーてー").unwrap() > sp.find("おかえり").unwrap());
        assert!(o.harmony && o.sound_id.is_none() && o.segments.len() == 2);
        // 何も指定が無ければ「テストです」
        let mut blank = entry("t2", 7);
        blank.kind = AlarmKind::Speech;
        assert!(AppState::test_occurrence(&blank).speech.unwrap().contains("テストです"));
        // 音の場合は、音だけ
        assert_eq!(Some("chime".to_string()), AppState::test_occurrence(&entry("t3", 7)).sound_id);
    }
}
