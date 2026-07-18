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

use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

use tauri::{AppHandle, Emitter, Manager};

static ENABLED: AtomicBool = AtomicBool::new(false);
static VK: AtomicI32 = AtomicI32::new(VK_CONTROL);

const VK_SHIFT: i32 = 0x10;
const VK_CONTROL: i32 = 0x11;
const VK_MENU: i32 = 0x12; // Alt

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
        let mut active = false;
        loop {
            std::thread::sleep(std::time::Duration::from_millis(50));
            let held = unsafe { GetAsyncKeyState(VK.load(Ordering::Relaxed)) } as u16 & 0x8000 != 0;
            let want = ENABLED.load(Ordering::Relaxed) && held;
            if want != active && apply(&app, want) {
                active = want;
                let _ = app.emit(
                    "click-through-changed",
                    serde_json::json!({ "active": active }),
                );
            }
        }
    });
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
}
