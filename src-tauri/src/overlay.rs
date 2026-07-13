//! In-game overlay window: a small always-on-top, non-activating panel that
//! shows over a fullscreen/borderless game without stealing input focus.
//!
//! Creation happens once at startup (see `init`, called from `lib.rs`
//! `setup`, mirroring the settings/first-run pre-creation pattern — building
//! it later from a command produces a blank/frozen webview in dev). The
//! window then stays hidden until `show`/`hide` toggle it. Task 6 wires the
//! actual G-key feedback UI into `OverlayApp.tsx`; this task only builds the
//! window plumbing.

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};

/// Window label for the overlay webview.
pub const LABEL: &str = "overlay";

/// Pre-create the overlay window during startup. Called from `lib.rs`
/// `setup` right after the settings/first-run windows are built.
pub fn init(app: &AppHandle) {
    let window = match WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App(std::path::PathBuf::new()))
        .title("GKey Mover — Overlay")
        .inner_size(420.0, 480.0)
        .resizable(false)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .visible(false)
        .build()
    {
        Ok(w) => w,
        Err(e) => {
            log::error!("Failed to create overlay window: {}", e);
            return;
        }
    };

    apply_noactivate(&window);
}

/// Set WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW on the overlay's HWND so clicking
/// it (or showing it) never steals foreground/input focus from whatever the
/// user has focused (the game). Without this, `show()` alone can still
/// activate the window on some window managers/games.
fn apply_noactivate(window: &tauri::WebviewWindow) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    };

    let hwnd = match window.hwnd() {
        Ok(h) => h,
        Err(e) => {
            log::error!("overlay: failed to get HWND: {}", e);
            return;
        }
    };

    unsafe {
        let prev = GetWindowLongPtrW(hwnd.0, GWL_EXSTYLE);
        SetWindowLongPtrW(
            hwnd.0,
            GWL_EXSTYLE,
            prev | (WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW) as isize,
        );
    }
}

/// Show the overlay positioned at the bottom-center of the monitor
/// containing the cursor, without taking focus. Emits `overlay-visible`
/// `{visible: true}` app-wide once shown.
pub fn show(app: &AppHandle) {
    let Some(window) = app.get_webview_window(LABEL) else {
        log::error!("overlay: show called before window was created");
        return;
    };

    if let Some((x, y)) = target_position(&window) {
        let _ = window.set_position(PhysicalPosition::new(x, y));
    }

    // Deliberately no `set_focus()` — that's the whole point of this window.
    let _ = window.show();

    let _ = app.emit("overlay-visible", serde_json::json!({ "visible": true }));
}

/// Hide the overlay. Emits `overlay-visible` `{visible: false}` app-wide.
pub fn hide(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.hide();
    }
    let _ = app.emit("overlay-visible", serde_json::json!({ "visible": false }));
}

/// Compute the bottom-center physical-pixel position for the overlay on the
/// monitor currently under the cursor (falls back to the window's current
/// monitor, then the primary monitor).
fn target_position(window: &tauri::WebviewWindow) -> Option<(i32, i32)> {
    let monitor = window
        .cursor_position()
        .ok()
        .and_then(|pos| window.monitor_from_point(pos.x, pos.y).ok().flatten())
        .or_else(|| window.current_monitor().ok().flatten())
        .or_else(|| window.primary_monitor().ok().flatten())?;

    let win_size = window.inner_size().ok()?;
    let mon_pos = monitor.position();
    let mon_size = monitor.size();

    let x = mon_pos.x + (mon_size.width as i32 - win_size.width as i32) / 2;
    let y = mon_pos.y + mon_size.height as i32 - win_size.height as i32 - 80;
    Some((x, y))
}

/// Dev/testing command to show the overlay on demand.
#[tauri::command]
pub fn show_overlay(app: AppHandle) {
    show(&app);
}

/// Dev/testing command to hide the overlay on demand.
#[tauri::command]
pub fn hide_overlay(app: AppHandle) {
    hide(&app);
}
