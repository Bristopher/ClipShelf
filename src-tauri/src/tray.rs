//! System tray icon + custom themed context menu.
//!
//! The tray uses no native menu: right-click shows a small frameless,
//! always-on-top webview window ("traymenu", rendered by `TrayMenuApp.tsx`)
//! positioned at the cursor, so the menu matches the app theme. The window
//! is pre-created hidden at startup (same reason as settings/first-run:
//! building webviews later in dev produces a blank window) and hides itself
//! on blur/Esc from the frontend.

use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

/// Window label for the tray context-menu webview.
pub const MENU_LABEL: &str = "traymenu";

/// Logical size of the menu window. Must match the layout in
/// `TrayMenuApp.tsx` — the window is fixed-size and the content is designed
/// to fill it exactly.
const MENU_WIDTH: f64 = 224.0;
const MENU_HEIGHT: f64 = 274.0;

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    // Pre-create the (hidden) menu window.
    if let Err(e) = WebviewWindowBuilder::new(app, MENU_LABEL, WebviewUrl::App(std::path::PathBuf::new()))
        .title("ClipShelf — Tray Menu")
        .inner_size(MENU_WIDTH, MENU_HEIGHT)
        .resizable(false)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .build()
    {
        log::error!("Failed to create tray menu window: {}", e);
    }

    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or("default window icon not available")?;

    TrayIconBuilder::new()
        .icon(icon)
        .tooltip(format!("ClipShelf v{}", env!("CARGO_PKG_VERSION")))
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button,
                button_state: MouseButtonState::Up,
                position,
                ..
            } = event
            {
                let app = tray.app_handle();
                match button {
                    MouseButton::Left => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    MouseButton::Right => show_menu(app, position.x, position.y),
                    _ => {}
                }
            }
        })
        .build(app)?;

    Ok(())
}

/// Position the menu window near the tray click and show it focused.
/// The click is in physical pixels; the menu opens above-left of the cursor
/// (tray lives at the bottom-right on standard taskbars), clamped into the
/// monitor under the cursor so it never opens off-screen.
fn show_menu(app: &AppHandle, click_x: f64, click_y: f64) {
    let Some(window) = app.get_webview_window(MENU_LABEL) else {
        log::error!("traymenu: window missing");
        return;
    };

    // Re-assert size on every show — some WebView2 versions let a transparent
    // window drift a few pixels after hide/show cycles.
    let _ = window.set_size(LogicalSize::new(MENU_WIDTH, MENU_HEIGHT));

    let scale = window
        .monitor_from_point(click_x, click_y)
        .ok()
        .flatten()
        .map(|m| m.scale_factor())
        .or_else(|| window.scale_factor().ok())
        .unwrap_or(1.0);
    let w = (MENU_WIDTH * scale) as i32;
    let h = (MENU_HEIGHT * scale) as i32;

    let mut x = click_x as i32 - w;
    let mut y = click_y as i32 - h - 8;

    if let Ok(Some(monitor)) = window.monitor_from_point(click_x, click_y) {
        let mp = monitor.position();
        let ms = monitor.size();
        x = x.clamp(mp.x, mp.x + ms.width as i32 - w);
        y = y.clamp(mp.y, mp.y + ms.height as i32 - h);
    }

    let _ = window.set_position(PhysicalPosition::new(x, y));
    let _ = window.show();
    let _ = window.set_focus();
    // Tell the frontend to refresh its snapshot (pause state, folder).
    let _ = app.emit_to(MENU_LABEL, "traymenu-visible", ());
}
