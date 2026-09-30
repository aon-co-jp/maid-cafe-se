//! 実際にHTTPサーバー(RPoem)を立てて、画面とAPIを端から端まで確かめる。SAPI(音声)は使わない(鳴らすAPIは呼ばない)。

use maid_cafe_desktop::state::AppState;
use maid_cafe_desktop::store::Store;
use serde_json::{json, Value};
use std::sync::Arc;

struct Server {
    base: String,
    port: u16,
    state: Arc<AppState>,
    dir: std::path::PathBuf,
    _rt: tokio::runtime::Runtime,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn server(tag: &str) -> Server {
    let dir = std::env::temp_dir().join(format!("mcs-http-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
    let state = AppState::new(Arc::new(Store::new(dir.clone()).unwrap()));
    // 実行中のアプリ(既定ポート)とぶつからないよう、空きポートを使う
    let port = {
        let l = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        l.local_addr().unwrap().port()
    };
    let addr = rt.block_on(maid_cafe_desktop::start(state.clone(), port)).unwrap();
    Server { base: format!("http://127.0.0.1:{}", addr.port()), port: addr.port(), state, dir, _rt: rt }
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().http_status_as_error(false).build().into()
}

fn get(s: &Server, path: &str) -> (u16, String) {
    let mut r = agent().get(&format!("{}{path}", s.base)).call().unwrap();
    (r.status().as_u16(), r.body_mut().read_to_string().unwrap())
}

fn post(s: &Server, path: &str, body: &Value, headers: &[(&str, &str)], with_marker: bool) -> (u16, Value) {
    let mut req = agent().post(&format!("{}{path}", s.base)).header("content-type", "application/json");
    if with_marker {
        req = req.header("x-maid-cafe", "1");
    }
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let mut r = req.send(body.to_string()).unwrap();
    let status = r.status().as_u16();
    let text = r.body_mut().read_to_string().unwrap();
    (status, serde_json::from_str(&text).unwrap_or(Value::Null))
}

fn alarm(id: &str) -> Value {
    json!({"id": id, "label": "起床", "time": "07:30", "recurrence": {"kind": "weekdays", "skip_holidays": true},
           "kind": "SPEECH", "sound": "chime", "text": "薬を飲む", "voice": "MAID", "enabled": true, "speech_sound": false,
           "phrases": ["okite", "fight"], "pre_phrases": ["fight"], "harmony": true, "pre_notice_minutes": 30})
}

#[test]
fn serves_the_ui_files() {
    let s = server("ui");
    let (st, html) = get(&s, "/");
    assert_eq!(200, st);
    assert!(html.contains("maid-cafe-se") && html.contains("/app.js"));
    let (st, js) = get(&s, "/app.js");
    assert!(st == 200 && js.contains("/api/state"));
    let (st, css) = get(&s, "/style.css");
    assert!(st == 200 && css.contains("--accent"));
}

#[test]
fn alarm_crud_over_http_persists_to_disk() {
    let s = server("crud");
    let (st, state) = {
        let (st, t) = get(&s, "/api/state");
        (st, serde_json::from_str::<Value>(&t).unwrap())
    };
    assert_eq!(200, st);
    assert_eq!(json!([]), state["alarms"]);
    assert!(state["next"].is_null());
    assert_eq!(3, state["sounds"].as_array().unwrap().len());
    assert_eq!(7, state["phrases"].as_array().unwrap().len());

    let (st, r) = post(&s, "/api/alarms", &alarm("a1"), &[], true);
    assert_eq!((200, json!({"ok": true})), (st, r));
    let (_, t) = get(&s, "/api/state");
    let state: Value = serde_json::from_str(&t).unwrap();
    assert_eq!("起床", state["alarms"][0]["label"]);
    assert_eq!(json!(["okite", "fight"]), state["alarms"][0]["phrases"]);
    assert!(state["next"]["title"].as_str().unwrap().contains("起床"), "{state}");

    // ディスク(alarms.txt)にも保存されている: 再起動相当で読み直しても同じ
    assert_eq!(1, Store::new(s.dir.clone()).unwrap().load_entries().len());

    let (st, _) = post(&s, "/api/alarms/toggle", &json!({"id": "a1", "enabled": false}), &[], true);
    assert_eq!(200, st);
    assert!(!s.state.entries()[0].enabled);
    let (st, _) = post(&s, "/api/alarms/toggle", &json!({"id": "nope", "enabled": true}), &[], true);
    assert_eq!(404, st);

    let (st, _) = post(&s, "/api/alarms/delete", &json!({"id": "a1"}), &[], true);
    assert_eq!(200, st);
    assert!(s.state.entries().is_empty());
    assert!(Store::new(s.dir.clone()).unwrap().load_entries().is_empty());
}

#[test]
fn invalid_input_is_rejected_with_a_readable_message() {
    let s = server("invalid");
    let mut bad = alarm("x");
    bad["time"] = json!("25:99");
    let (st, r) = post(&s, "/api/alarms", &bad, &[], true);
    assert_eq!(400, st);
    assert!(r["error"].as_str().unwrap().contains("時刻"));
    let (st, r) = post(&s, "/api/alarms", &json!({"nonsense": true}), &[], true);
    assert_eq!(400, st);
    assert!(r["error"].as_str().unwrap().contains("JSON"));
    assert!(s.state.entries().is_empty());
    // 大きすぎる本文
    let huge = json!({"id": "x".repeat(70_000)});
    assert_eq!(413, post(&s, "/api/alarms", &huge, &[], true).0);
}

#[test]
fn cross_site_and_rebinding_requests_are_refused() {
    let s = server("guard");
    // マーカーヘッダなし(他サイトのフォーム送信を想定)
    assert_eq!(403, post(&s, "/api/alarms", &alarm("a1"), &[], false).0);
    // 別サイトのOrigin
    assert_eq!(403, post(&s, "/api/alarms", &alarm("a1"), &[("origin", "https://evil.example")], true).0);
    // 自分のOriginなら通る
    let own = format!("http://127.0.0.1:{}", s.port);
    assert_eq!(200, post(&s, "/api/alarms", &alarm("a1"), &[("origin", &own)], true).0);
    // Hostが違う(DNSリバインディング)
    let r = agent().get(&format!("{}/api/state", s.base)).header("host", "evil.example").call().unwrap();
    assert_eq!(403, r.status().as_u16());
    let r = agent().get(&format!("{}/", s.base)).header("host", "evil.example").call().unwrap();
    assert_eq!(403, r.status().as_u16());
    assert_eq!(1, s.state.entries().len()); // 拒否された分は保存されていない
}

#[test]
fn phrase_numbering_endpoint_matches_the_core_rules() {
    let s = server("phrases");
    let (st, r) = post(&s, "/api/phrases/edit", &json!({"numbers": [["okite", 1], ["okaeri", 2]], "op": "assign", "id": "okaeri", "n": 1}), &[], true);
    assert_eq!(200, st);
    // okaeriが1を取り、okiteは空いている最小番号2へ
    assert_eq!(json!(["okaeri", "okite"]), r["ordered"]);
    assert_eq!(json!([["okite", 2], ["okaeri", 1]]), r["numbers"]);
    assert_eq!(400, post(&s, "/api/phrases/edit", &json!({"numbers": [], "op": "add", "id": "bogus"}), &[], true).0);
}

#[test]
fn settings_store_voice_preferences() {
    let s = server("settings");
    let (st, _) = post(&s, "/api/settings", &json!({"voice_maid": "Microsoft Haruka Desktop"}), &[], true);
    assert_eq!(200, st);
    let (_, t) = get(&s, "/api/state");
    let state: Value = serde_json::from_str(&t).unwrap();
    assert_eq!("Microsoft Haruka Desktop", state["prefs"]["MAID"]);
    assert!(state["prefs"]["DEEP_MALE"].is_null());
    post(&s, "/api/settings", &json!({"voice_maid": ""}), &[], true);
    assert!(s.state.voice_pref(maid_cafe_core::VoiceStyle::Maid).is_none());
    assert_eq!(400, post(&s, "/api/settings", &json!({"voice_maid": "x".repeat(300)}), &[], true).0);
}
