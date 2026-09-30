//! 画面(Web UI)と、そのJSON API。RPoem(Poem互換)で`127.0.0.1`だけに待ち受ける。
//!
//! 安全対策: ①`127.0.0.1`にだけbind ②`Host`が自分(127.0.0.1/localhost+ポート)でなければ拒否
//! (DNSリバインディング対策) ③`Origin`があれば自分のものだけ許可 ④POSTはカスタムヘッダ`x-maid-cafe: 1`必須
//! (他サイトのフォームからの送信を防ぐ) ⑤本文は64KBまで。

use crate::dto::AlarmDto;
use crate::state::AppState;
use bytes::Bytes;
use http_body_util::BodyExt;
use maid_cafe_core::{MaidPhrases, VoiceStyle, PHRASES, SOUNDS};
use open_runo_poem_compat::{get, handler_fn, post, Request, Response, Route, StatusCode};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

const INDEX_HTML: &str = include_str!("../ui/index.html");
const APP_JS: &str = include_str!("../ui/app.js");
const STYLE_CSS: &str = include_str!("../ui/style.css");
const MAX_BODY: usize = 64 * 1024;

fn reply(status: StatusCode, content_type: &str, body: Bytes) -> Response {
    hyper::Response::builder()
        .status(status)
        .header("content-type", content_type)
        .header("cache-control", "no-store")
        .header("x-content-type-options", "nosniff")
        .header("referrer-policy", "no-referrer")
        .body(open_runo_poem_compat::fixed_body(body))
        .expect("固定の有効なヘッダだけなので、応答の組み立ては失敗しない")
}

fn json_reply(status: StatusCode, v: &Value) -> Response {
    reply(status, "application/json; charset=utf-8", Bytes::from(serde_json::to_vec(v).unwrap_or_else(|_| b"{}".to_vec())))
}

fn err(status: StatusCode, msg: &str) -> Response {
    json_reply(status, &json!({ "error": msg }))
}

/// 画面(静的ファイル)用の応答。
fn page(content_type: &str, body: &'static str) -> Response {
    reply(StatusCode::OK, content_type, Bytes::from_static(body.as_bytes()))
}

/// アクセス元の検査(純粋関数)。`Err`は拒否する理由。
pub fn check_access(host: Option<&str>, origin: Option<&str>, custom: Option<&str>, is_post: bool, port: u16) -> Result<(), &'static str> {
    let allowed_hosts = [format!("127.0.0.1:{port}"), format!("localhost:{port}")];
    match host {
        Some(h) if allowed_hosts.iter().any(|a| a.eq_ignore_ascii_case(h)) => {}
        _ => return Err("Hostが不正です"),
    }
    if let Some(o) = origin {
        // `Origin: null`(file:やsandbox)も拒否
        if !allowed_hosts.iter().any(|a| o.eq_ignore_ascii_case(&format!("http://{a}"))) {
            return Err("Originが不正です");
        }
    }
    if is_post && custom != Some("1") {
        return Err("x-maid-cafeヘッダがありません");
    }
    Ok(())
}

fn guard(req: &Request, port: u16) -> Result<(), Response> {
    let h = |name: &str| req.headers().get(name).and_then(|v| v.to_str().ok());
    let is_post = req.method() == hyper::Method::POST;
    check_access(h("host"), h("origin"), h("x-maid-cafe"), is_post, port).map_err(|m| err(StatusCode::FORBIDDEN, m))
}

/// 検査に通らなければ、**本文を読み捨ててから**拒否の応答を返す(本文を残したまま応答すると、
/// クライアント側で接続リセットになることがある)。読み捨ては`MAX_BODY`まで。
async fn guard_post(req: Request, port: u16) -> Result<Request, Response> {
    match guard(&req, port) {
        Ok(()) => Ok(req),
        Err(resp) => {
            let _ = http_body_util::Limited::new(req.into_body(), MAX_BODY).collect().await;
            Err(resp)
        }
    }
}

async fn read_json<T: serde::de::DeserializeOwned>(req: Request) -> Result<T, Response> {
    // 上限を超える本文は、全部をメモリに溜める前に打ち切る
    let bytes = match http_body_util::Limited::new(req.into_body(), MAX_BODY).collect().await {
        Ok(c) => c.to_bytes(),
        Err(e) if e.downcast_ref::<http_body_util::LengthLimitError>().is_some() => {
            return Err(err(StatusCode::PAYLOAD_TOO_LARGE, "本文が大きすぎます"));
        }
        Err(_) => return Err(err(StatusCode::BAD_REQUEST, "本文を読めません")),
    };
    serde_json::from_slice(&bytes).map_err(|e| err(StatusCode::BAD_REQUEST, &format!("JSONが不正です: {e}")))
}

fn style_key(s: VoiceStyle) -> &'static str {
    s.name()
}

async fn state_json(state: Arc<AppState>) -> Value {
    let st = state.clone();
    let voices = tokio::task::spawn_blocking(move || st.voices()).await.unwrap_or_default();
    let next = state.next_preview().map(|(t, title)| json!({ "time": t.format("%Y-%m-%d %H:%M").to_string(), "title": title }));
    json!({
        "version": env!("CARGO_PKG_VERSION"),
        "alarms": state.entries().iter().map(AlarmDto::from_core).collect::<Vec<_>>(),
        "next": next,
        "playing": state.player.playing(),
        "autostart": state.autostart_enabled(),
        "voices": voices.iter().map(|v| json!({"name": v.name, "culture": v.culture, "japanese": v.is_japanese()})).collect::<Vec<_>>(),
        "prefs": {
            style_key(VoiceStyle::Maid): state.voice_pref(VoiceStyle::Maid),
            style_key(VoiceStyle::DeepMale): state.voice_pref(VoiceStyle::DeepMale),
        },
        "sounds": SOUNDS.iter().map(|s| json!({"id": s.id, "name": s.display_name})).collect::<Vec<_>>(),
        "phrases": PHRASES.iter().map(|p| json!({"id": p.id, "display": p.display})).collect::<Vec<_>>(),
    })
}

#[derive(Deserialize)]
struct IdBody {
    id: String,
}

#[derive(Deserialize)]
struct ToggleBody {
    id: String,
    enabled: bool,
}

#[derive(Deserialize)]
struct PhraseEdit {
    /// 今の番号表 `[["okite",1],["okaeri",2]]`
    numbers: Vec<(String, i32)>,
    /// `add` / `remove` / `assign`
    op: String,
    id: String,
    #[serde(default)]
    n: i32,
}

#[derive(Deserialize)]
struct Settings {
    /// 声の元のWindows音声名。空文字で自動に戻す。
    #[serde(default)]
    voice_maid: Option<String>,
    #[serde(default)]
    voice_deep_male: Option<String>,
    #[serde(default)]
    autostart: Option<bool>,
}

/// 番号表の編集(画面のセリフ選択・番号入力)。選択は`numbers`の並びが持ち、読み上げ順は番号順。
pub fn edit_numbers(numbers: &[(String, i32)], op: &str, id: &str, n: i32) -> Result<Vec<(String, i32)>, String> {
    if MaidPhrases::by_id(id).is_none() {
        return Err(format!("セリフが不正です: {id}"));
    }
    let cur: Vec<(String, i32)> = numbers.to_vec();
    match op {
        "add" => Ok(MaidPhrases::add(&cur, id)),
        "remove" => Ok(MaidPhrases::remove(&cur, id)),
        "assign" => Ok(MaidPhrases::assign(&cur, id, n)),
        _ => Err(format!("操作が不正です: {op}")),
    }
}

/// ルート全体を組み立てる。
pub fn routes(state: Arc<AppState>, port: u16) -> Route {
    let mut app = Route::new();

    // 画面(静的)。Hostだけ検査する(GET)。
    macro_rules! static_page {
        ($path:expr, $ct:expr, $body:expr) => {
            app = app.at(
                $path,
                get(handler_fn(move |req, _p| async move {
                    match guard(&req, port) {
                        Ok(()) => page($ct, $body),
                        Err(r) => r,
                    }
                })),
            );
        };
    }
    static_page!("/", "text/html; charset=utf-8", INDEX_HTML);
    static_page!("/app.js", "text/javascript; charset=utf-8", APP_JS);
    static_page!("/style.css", "text/css; charset=utf-8", STYLE_CSS);

    let s = state.clone();
    app = app.at(
        "/api/state",
        get(handler_fn(move |req, _p| {
            let s = s.clone();
            async move {
                let _req = match guard_post(req, port).await {
                    Ok(r) => r,
                    Err(r) => return r,
                };
                json_reply(StatusCode::OK, &state_json(s).await)
            }
        })),
    );

    let s = state.clone();
    app = app.at(
        "/api/alarms",
        post(handler_fn(move |req, _p| {
            let s = s.clone();
            async move {
                let req = match guard_post(req, port).await {
                    Ok(r) => r,
                    Err(r) => return r,
                };
                let dto: AlarmDto = match read_json(req).await {
                    Ok(d) => d,
                    Err(r) => return r,
                };
                match dto.to_core().and_then(|e| s.upsert(e)) {
                    Ok(()) => json_reply(StatusCode::OK, &json!({ "ok": true })),
                    Err(m) => err(StatusCode::BAD_REQUEST, &m),
                }
            }
        })),
    );

    let s = state.clone();
    app = app.at(
        "/api/alarms/delete",
        post(handler_fn(move |req, _p| {
            let s = s.clone();
            async move {
                let req = match guard_post(req, port).await {
                    Ok(r) => r,
                    Err(r) => return r,
                };
                let b: IdBody = match read_json(req).await {
                    Ok(b) => b,
                    Err(r) => return r,
                };
                match s.delete(&b.id) {
                    Ok(()) => json_reply(StatusCode::OK, &json!({ "ok": true })),
                    Err(m) => err(StatusCode::INTERNAL_SERVER_ERROR, &m),
                }
            }
        })),
    );

    let s = state.clone();
    app = app.at(
        "/api/alarms/toggle",
        post(handler_fn(move |req, _p| {
            let s = s.clone();
            async move {
                let req = match guard_post(req, port).await {
                    Ok(r) => r,
                    Err(r) => return r,
                };
                let b: ToggleBody = match read_json(req).await {
                    Ok(b) => b,
                    Err(r) => return r,
                };
                match s.set_enabled(&b.id, b.enabled) {
                    Ok(()) => json_reply(StatusCode::OK, &json!({ "ok": true })),
                    Err(m) => err(StatusCode::NOT_FOUND, &m),
                }
            }
        })),
    );

    app = app.at(
        "/api/phrases/edit",
        post(handler_fn(move |req, _p| async move {
            let req = match guard_post(req, port).await {
                Ok(r) => r,
                Err(r) => return r,
            };
            let b: PhraseEdit = match read_json(req).await {
                Ok(b) => b,
                Err(r) => return r,
            };
            match edit_numbers(&b.numbers, &b.op, &b.id, b.n) {
                Ok(numbers) => {
                    let ordered = MaidPhrases::ordered(&numbers);
                    json_reply(StatusCode::OK, &json!({ "numbers": numbers, "ordered": ordered }))
                }
                Err(m) => err(StatusCode::BAD_REQUEST, &m),
            }
        })),
    );

    let s = state.clone();
    app = app.at(
        "/api/test",
        post(handler_fn(move |req, _p| {
            let s = s.clone();
            async move {
                let req = match guard_post(req, port).await {
                    Ok(r) => r,
                    Err(r) => return r,
                };
                let dto: AlarmDto = match read_json(req).await {
                    Ok(d) => d,
                    Err(r) => return r,
                };
                let entry = match dto.to_core() {
                    Ok(e) => e,
                    Err(m) => return err(StatusCode::BAD_REQUEST, &m),
                };
                if s.player.playing().is_some() {
                    return err(StatusCode::CONFLICT, "いま再生中です。止めてからもう一度どうぞ");
                }
                let occ = AppState::test_occurrence(&entry);
                let st = s.clone();
                tokio::task::spawn_blocking(move || {
                    let s2 = st.clone();
                    st.player.play(&[occ], &move |style| s2.voice_pref(style));
                });
                json_reply(StatusCode::OK, &json!({ "ok": true }))
            }
        })),
    );

    let s = state.clone();
    app = app.at(
        "/api/stop",
        post(handler_fn(move |req, _p| {
            let s = s.clone();
            async move {
                let _req = match guard_post(req, port).await {
                    Ok(r) => r,
                    Err(r) => return r,
                };
                s.player.stop();
                json_reply(StatusCode::OK, &json!({ "ok": true }))
            }
        })),
    );

    let s = state.clone();
    app = app.at(
        "/api/settings",
        post(handler_fn(move |req, _p| {
            let s = s.clone();
            async move {
                let req = match guard_post(req, port).await {
                    Ok(r) => r,
                    Err(r) => return r,
                };
                let b: Settings = match read_json(req).await {
                    Ok(b) => b,
                    Err(r) => return r,
                };
                for (style, v) in [(VoiceStyle::Maid, &b.voice_maid), (VoiceStyle::DeepMale, &b.voice_deep_male)] {
                    if let Some(name) = v {
                        if name.chars().count() > 200 {
                            return err(StatusCode::BAD_REQUEST, "音声名が長すぎます");
                        }
                        let pref = if name.trim().is_empty() { None } else { Some(name.trim()) };
                        if let Err(e) = s.store.save_voice_pref(style, pref) {
                            return err(StatusCode::INTERNAL_SERVER_ERROR, &format!("保存できませんでした: {e}"));
                        }
                    }
                }
                if let Some(on) = b.autostart {
                    let st = s.clone();
                    match tokio::task::spawn_blocking(move || st.set_autostart(on)).await {
                        Ok(Ok(())) => {}
                        Ok(Err(m)) => return err(StatusCode::INTERNAL_SERVER_ERROR, &m),
                        Err(_) => return err(StatusCode::INTERNAL_SERVER_ERROR, "内部エラー"),
                    }
                }
                json_reply(StatusCode::OK, &json!({ "ok": true }))
            }
        })),
    );

    app
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn access_check_accepts_only_own_host_and_origin() {
        let ok = |h, o, c, p| check_access(h, o, c, p, 47411);
        assert!(ok(Some("127.0.0.1:47411"), None, None, false).is_ok());
        assert!(ok(Some("localhost:47411"), Some("http://localhost:47411"), Some("1"), true).is_ok());
        assert!(ok(Some("evil.example:47411"), None, None, false).is_err()); // DNSリバインディング
        assert!(ok(Some("127.0.0.1:1"), None, None, false).is_err()); // 別ポート
        assert!(ok(None, None, None, false).is_err());
        assert!(ok(Some("127.0.0.1:47411"), Some("https://evil.example"), Some("1"), true).is_err());
        assert!(ok(Some("127.0.0.1:47411"), Some("null"), Some("1"), true).is_err());
        assert!(ok(Some("127.0.0.1:47411"), None, None, true).is_err()); // POSTはヘッダ必須
        assert!(ok(Some("127.0.0.1:47411"), None, Some("0"), true).is_err());
    }

    #[test]
    fn phrase_edit_follows_core_rules() {
        let n = edit_numbers(&[], "add", "okite", 0).unwrap();
        let n = edit_numbers(&n, "add", "okaeri", 0).unwrap();
        assert_eq!(vec![("okite".to_string(), 1), ("okaeri".to_string(), 2)], n);
        // okaeriを1にすると、okiteは空いている最小番号(2)へ
        let n = edit_numbers(&n, "assign", "okaeri", 1).unwrap();
        assert_eq!(vec!["okaeri".to_string(), "okite".to_string()], MaidPhrases::ordered(&n));
        let n = edit_numbers(&n, "remove", "okaeri", 0).unwrap();
        assert_eq!(1, n.len());
        assert!(edit_numbers(&n, "add", "bogus", 0).is_err());
        assert!(edit_numbers(&n, "shuffle", "okite", 0).is_err());
    }
}
