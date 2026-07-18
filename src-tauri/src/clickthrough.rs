//! Hold-to-click-through for the main window.
//!
//! While the configured modifier key is physically held, the main window
//! gets `WS_EX_TRANSPARENT`, so mouse input falls straight through to
//! whatever is underneath — the semi-transparent window stays visible,
//! focused-app state doesn't change, and nothing minimizes. Releasing the
//! key restores normal hit-testing.
//!
//! A small polling thread (50ms) reads `GetAsyncKeyState`, so the hold
//! works even when the window is NOT focused — the main use case is a game
//! or browser focused with ClipShelf floating on top. Configuration lives
//! in atomics so `update_config` hot-applies changes without restarting
//! the thread. The window is already `WS_EX_LAYERED` (opacity plumbing in
//! `set_window_opacity`), which `WS_EX_TRANSPARENT` requires to pass
//! rendered-content clicks through.
//!
//! One region is carved out: the titlebar close button. While the modifier
//! is held, the poll also checks the cursor position, and when it sits over
//! the close-button rect `WS_EX_TRANSPARENT` is dropped so that one button
//! stays hoverable/clickable (the frontend shows it as a skull = full quit).
//! Everywhere else keeps passing clicks through.

use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

use tauri::{AppHandle, Emitter, Manager};

static ENABLED: AtomicBool = AtomicBool::new(false);
static VK: AtomicI32 = AtomicI32::new(VK_CONTROL);

const VK_SHIFT: i32 = 0x10;
const VK_CONTROL: i32 = 0x11;
const VK_MENU: i32 = 0x12; // Alt

// Close-button hit rect in CSS px — mirrors TitleBar.tsx (`w-10` button in
// the `h-7` titlebar, flush with the window's top-right corner).
const CLOSE_BTN_W_CSS: f64 = 40.0;
const TITLEBAR_H_CSS: f64 = 28.0;

/// Modifier name from config → virtual-key code. Unknown values fall back
/// to Ctrl (the default) rather than disabling the feature silently.
pub fn vk_from_key(key: &str) -> i32 {
    match key.trim().to_ascii_lowercase().as_str() {
        "alt" => VK_MENU,
        "shift" => VK_SHIFT,
        _ => VK_CONTROL,
    }
}

/// Hot-apply settings (startup + every update_config).
pub fn configure(enabled: bool, key: &str) {
    ENABLED.store(enabled, Ordering::Relaxed);
    VK.store(vk_from_key(key), Ordering::Relaxed);
}

/// Start the polling thread. Call once at setup.
pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
        // `mode_active` is the user-facing state (key held → skull button,
        // badge, hint). `transparent` is the actual WS_EX_TRANSPARENT bit,
        // which additionally drops while the cursor hovers the close button
        // so that one spot stays clickable.
        let mut mode_active = false;
        let mut transparent = false;
        loop {
            std::thread::sleep(std::time::Duration::from_millis(50));
            let held = unsafe { GetAsyncKeyState(VK.load(Ordering::Relaxed)) } as u16 & 0x8000 != 0;
            let want_mode = ENABLED.load(Ordering::Relaxed) && held;
            let want_transparent = want_mode && !cursor_over_close(&app);
            if want_transparent != transparent && apply(&app, want_transparent) {
                transparent = want_transparent;
            }
            if want_mode != mode_active {
                mode_active = want_mode;
                let _ = app.emit(
                    "click-through-changed",
                    serde_json::json!({ "active": mode_active }),
                );
            }
        }
    });
}

/// Is the cursor inside the titlebar close button's screen rect?
fn cursor_over_close(app: &AppHandle) -> bool {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;

    let Some(window) = app.get_webview_window("main") else {
        return false;
    };
    let (Ok(pos), Ok(size), Ok(scale)) = (
        window.outer_position(),
        window.outer_size(),
        window.scale_factor(),
    ) else {
        return false;
    };
    let mut pt = POINT { x: 0, y: 0 };
    if unsafe { GetCursorPos(&mut pt) } == 0 {
        return false;
    }
    point_in_close_rect(pt.x, pt.y, pos.x, pos.y, size.width as i32, scale)
}

/// Pure hit test: the close button occupies the top-right
/// `CLOSE_BTN_W_CSS` × `TITLEBAR_H_CSS` corner of the window (physical px
/// via `scale`).
fn point_in_close_rect(px: i32, py: i32, win_x: i32, win_y: i32, win_w: i32, scale: f64) -> bool {
    let btn_w = (CLOSE_BTN_W_CSS * scale).round() as i32;
    let btn_h = (TITLEBAR_H_CSS * scale).round() as i32;
    let right = win_x + win_w;
    px >= right - btn_w && px < right && py >= win_y && py < win_y + btn_h
}

/// Add/remove `WS_EX_TRANSPARENT` on the main window. Returns false when the
/// window isn't available (startup race) so the state machine retries on the
/// next tick instead of getting stuck out of sync.
fn apply(app: &AppHandle, on: bool) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_TRANSPARENT,
    };

    let Some(window) = app.get_webview_window("main") else {
        return false;
    };
    let Ok(hwnd) = window.hwnd() else {
        return false;
    };
    unsafe {
        let prev = GetWindowLongPtrW(hwnd.0, GWL_EXSTYLE);
        let next = if on {
            prev | WS_EX_TRANSPARENT as isize
        } else {
            prev & !(WS_EX_TRANSPARENT as isize)
        };
        if next != prev {
            SetWindowLongPtrW(hwnd.0, GWL_EXSTYLE, next);
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vk_from_key_maps_modifiers_and_defaults_to_ctrl() {
        assert_eq!(vk_from_key("ctrl"), VK_CONTROL);
        assert_eq!(vk_from_key("Alt"), VK_MENU);
        assert_eq!(vk_from_key(" SHIFT "), VK_SHIFT);
        assert_eq!(vk_from_key("bogus"), VK_CONTROL);
        assert_eq!(vk_from_key(""), VK_CONTROL);
    }

    #[test]
    fn test_point_in_close_rect_hits_top_right_corner_only() {
        // 800px-wide window at (100, 50), scale 1.0 → close rect x [860, 900), y [50, 78)
        assert!(point_in_close_rect(880, 60, 100, 50, 800, 1.0));
        assert!(point_in_close_rect(860, 50, 100, 50, 800, 1.0)); // top-left of rect
        assert!(point_in_close_rect(899, 77, 100, 50, 800, 1.0)); // bottom-right inside
        assert!(!point_in_close_rect(859, 60, 100, 50, 800, 1.0)); // left of button
        assert!(!point_in_close_rect(880, 78, 100, 50, 800, 1.0)); // below titlebar
        assert!(!point_in_close_rect(900, 60, 100, 50, 800, 1.0)); // past right edge
        // 150% DPI widens the rect: [840, 900) x [50, 92)
        assert!(point_in_close_rect(845, 90, 100, 50, 800, 1.5));
        assert!(!point_in_close_rect(839, 60, 100, 50, 800, 1.5));
    }
}
