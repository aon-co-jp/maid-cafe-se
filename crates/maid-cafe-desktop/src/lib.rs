#![allow(clippy::result_large_err)] // HTTPの応答(hyper::Response)をそのままErrで返す設計。大きさは問題にならない

//! maid-cafe-se Windows版。画面とAPI(RPoem)・スケジューラ・Windows音声(SAPI)・保存。`main.rs`がトレイと起動処理をつなぐ。

pub mod api;
pub mod dto;
pub mod player;
pub mod sapi;
pub mod scheduler;
pub mod selftest;
pub mod state;
pub mod store;
pub mod tray;

use open_runo_poem_compat::{Server, TcpListener};
use state::AppState;
use std::net::SocketAddr;
use std::sync::Arc;

/// 画面の既定ポート。使用中なら空いているポートを使う。
pub const PREFERRED_PORT: u16 = 47411;

/// `127.0.0.1`だけに待ち受けるポートを決める(既定→空きポートの順)。
pub fn pick_port() -> u16 {
    for p in [PREFERRED_PORT, 0] {
        if let Ok(l) = std::net::TcpListener::bind(("127.0.0.1", p)) {
            if let Ok(a) = l.local_addr() {
                return a.port();
            }
        }
    }
    PREFERRED_PORT
}

/// サーバーとスケジューラを起動する(tokioランタイム内で呼ぶ)。実際のアドレスを返す。
pub async fn start(state: Arc<AppState>, port: u16) -> std::io::Result<SocketAddr> {
    state.set_port(port);
    let app = api::routes(state.clone(), port);
    let (addr, _join) = Server::new(TcpListener::bind(([127, 0, 0, 1], port))).run(app).await?;
    state.store.write_port(addr.port()).ok();
    state.store.log(&format!("server listening on {addr}"));
    tokio::spawn(scheduler::run(state));
    Ok(addr)
}

/// 画面のURL。
pub fn ui_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}/")
}

/// 画面を、アプリ風のウィンドウ(Edgeの`--app`)で開く。Edgeが無ければ既定のブラウザ。
pub fn open_ui(port: u16) {
    let url = ui_url(port);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const NO_WINDOW: u32 = 0x0800_0000;
        let edge = ["ProgramFiles(x86)", "ProgramFiles"]
            .iter()
            .filter_map(std::env::var_os)
            .map(|p| std::path::PathBuf::from(p).join(r"Microsoft\Edge\Application\msedge.exe"))
            .find(|p| p.exists());
        let spawned = match edge {
            Some(e) => std::process::Command::new(e).arg(format!("--app={url}")).creation_flags(NO_WINDOW).spawn().is_ok(),
            None => false,
        };
        if !spawned {
            let _ = std::process::Command::new("cmd").args(["/c", "start", "", &url]).creation_flags(NO_WINDOW).spawn();
        }
    }
    #[cfg(not(windows))]
    {
        let _ = url;
    }
}
