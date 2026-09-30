#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
//! maid-cafe-se Windows版(Rust + RPoem)。
//!
//! ```text
//! maid-cafe-se                 画面を開いて起動(トレイに常駐)
//! maid-cafe-se --minimized     画面は開かず、トレイへ常駐(Windows起動時の自動起動用)
//! maid-cafe-se --no-tray       トレイなし(サーバーだけ動く)
//! maid-cafe-se --autostart on|off   Windows起動時の自動起動を設定/解除して終了
//! maid-cafe-se --selftest [出力先] [--play]   読み上げの自己診断
//! ```

use maid_cafe_desktop::state::AppState;
use maid_cafe_desktop::store::Store;
use std::sync::Arc;

/// 画面なし(GUIサブシステム)のexeでも、コマンドラインから実行したときは、親のコンソールへ出力できるようにする。
#[cfg(windows)]
fn attach_parent_console() {
    use windows_sys::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
    // SAFETY: 引数は定数。親にコンソールが無くて失敗しても、出力が捨てられるだけ。
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}
#[cfg(not(windows))]
fn attach_parent_console() {}

#[cfg(windows)]
mod single {
    use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows_sys::Win32::System::Threading::CreateMutexW;

    /// 2つ目の起動なら`false`(最初のプロセスだけが`true`)。ハンドルはプロセス終了まで保持する。
    pub fn first_instance() -> bool {
        let name: Vec<u16> = "Local\\maid-cafe-se-single-instance".encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: `name`はNUL終端のUTF-16。戻り値のハンドルは閉じない(プロセス終了で解放される)。
        unsafe {
            let h = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
            !h.is_null() && GetLastError() != ERROR_ALREADY_EXISTS
        }
    }
}
#[cfg(not(windows))]
mod single {
    pub fn first_instance() -> bool {
        true
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let has = |f: &str| args.iter().any(|a| a == f);

    if has("--selftest") || has("--autostart") {
        attach_parent_console();
    }

    if has("--selftest") {
        let rest: Vec<String> = args.iter().filter(|a| *a != "--selftest").cloned().collect();
        std::process::exit(maid_cafe_desktop::selftest::run(&rest));
    }

    let dir = Store::default_dir();
    let store = match Store::new(dir.clone()) {
        Ok(s) => Arc::new(s),
        Err(e) => {
            eprintln!("保存先を作れません({}): {e}", dir.display());
            std::process::exit(2);
        }
    };

    if let Some(i) = args.iter().position(|a| a == "--autostart") {
        let on = args.get(i + 1).map(|s| s == "on").unwrap_or(false);
        let state = AppState::new(store);
        match state.set_autostart(on) {
            Ok(()) => println!("自動起動を{}にしました", if on { "オン" } else { "オフ" }),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return;
    }

    if !single::first_instance() {
        // すでに起動している: 起動済みの画面を開いて終わる
        if !has("--minimized") {
            if let Some(port) = store.read_port() {
                maid_cafe_desktop::open_ui(port);
            }
        }
        return;
    }

    let runtime = match tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build() {
        Ok(r) => r,
        Err(e) => {
            store.log(&format!("runtime failed: {e}"));
            std::process::exit(2);
        }
    };
    let state = AppState::new(store.clone());
    let port = maid_cafe_desktop::pick_port();
    let addr = match runtime.block_on(maid_cafe_desktop::start(state.clone(), port)) {
        Ok(a) => a,
        Err(e) => {
            store.log(&format!("server failed: {e}"));
            eprintln!("起動できません: {e}");
            std::process::exit(2);
        }
    };

    if !has("--minimized") {
        maid_cafe_desktop::open_ui(addr.port());
    }
    if has("--no-tray") {
        runtime.block_on(std::future::pending::<()>());
    } else {
        maid_cafe_desktop::tray::run(state, addr.port());
    }
}
