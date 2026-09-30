//! タスクトレイ常駐。メニュー: 画面を開く / 止める / 終了。メインスレッドでイベントループを回す。

use crate::state::AppState;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tao::event::Event;
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIconBuilder, TrayIconEvent};

/// ピンクの丸のアイコン(画像ファイルを持たず、コードで作る)。
fn icon() -> Icon {
    const N: u32 = 32;
    let mut rgba = Vec::with_capacity((N * N * 4) as usize);
    let c = (N as f32 - 1.0) / 2.0;
    for y in 0..N {
        for x in 0..N {
            let d = ((x as f32 - c).powi(2) + (y as f32 - c).powi(2)).sqrt();
            let a = ((c - d) * 1.5 + 0.5).clamp(0.0, 1.0);
            let t = (d / c).clamp(0.0, 1.0);
            let (r, g, b) = (255.0 - 30.0 * t, 150.0 - 60.0 * t, 190.0 - 60.0 * t);
            rgba.extend_from_slice(&[r as u8, g as u8, b as u8, (a * 255.0) as u8]);
        }
    }
    Icon::from_rgba(rgba, N, N).expect("32x32のRGBAなので、アイコンは必ず作れる")
}

/// トレイを出して、「終了」が選ばれるまで戻らない。
pub fn run(state: Arc<AppState>, port: u16) {
    let event_loop = EventLoopBuilder::new().build();
    let menu = Menu::new();
    let open = MenuItem::new("画面を開く", true, None);
    let stop = MenuItem::new("鳴っている音を止める", true, None);
    let quit = MenuItem::new("終了", true, None);
    let _ = menu.append_items(&[&open, &stop, &PredefinedMenuItem::separator(), &quit]);
    let tray = TrayIconBuilder::new().with_menu(Box::new(menu)).with_tooltip("maid-cafe-se").with_icon(icon()).build();
    if let Err(e) = &tray {
        state.store.log(&format!("tray failed: {e}"));
    }
    let (open_id, stop_id, quit_id) = (open.id().clone(), stop.id().clone(), quit.id().clone());

    event_loop.run(move |_event: Event<'_, ()>, _, flow| {
        *flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(150));
        let _keep = &tray;
        while let Ok(ev) = MenuEvent::receiver().try_recv() {
            if ev.id == open_id {
                crate::open_ui(port);
            } else if ev.id == stop_id {
                state.player.stop();
            } else if ev.id == quit_id {
                state.player.stop();
                *flow = ControlFlow::Exit;
            }
        }
        while let Ok(ev) = TrayIconEvent::receiver().try_recv() {
            if matches!(ev, TrayIconEvent::DoubleClick { .. }) {
                crate::open_ui(port);
            }
        }
    });
}
