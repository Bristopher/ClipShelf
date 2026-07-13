//! In-overlay "type mode": a `WH_KEYBOARD_LL` low-level keyboard hook that
//! captures text WITHOUT stealing focus from the game.
//!
//! The overlay window is deliberately non-activating (see `overlay.rs`), so it
//! can never receive normal keyboard input — the focused game does. To let the
//! user type a label/description into the overlay while still in-game, we
//! install a system-wide low-level keyboard hook. While type mode is ACTIVE the
//! hook swallows every non-modifier key (so the game never sees the typing) and
//! forwards each keystroke to the frontend as an `overlay-type` event.
//!
//! ## Threading / lifetime design (the important part)
//!
//! A low-level keyboard hook MUST be serviced by a thread that runs a message
//! pump (`GetMessageW`); Windows calls the hook proc on that thread. We spawn a
//! dedicated OS thread the first time `start` is called, install the hook ONCE,
//! and keep both the thread and the hook alive for the rest of the process
//! lifetime.
//!
//! We gate behavior purely on an `ACTIVE: AtomicBool` rather than
//! installing/uninstalling the hook on every start/stop. This is the simplest
//! and completely race-free approach: `stop` just flips `ACTIVE` to false and
//! the still-installed hook instantly `CallNextHookEx`'s on its very first line
//! for every key. An idle LL hook that does nothing but pass through is
//! negligible overhead. The alternative — unhooking on stop and re-hooking on
//! start — reintroduces install/teardown races across the pump thread for no
//! practical benefit, so we do not do it.
//!
//! ## Hook-proc safety contract
//!
//! The hook proc runs in the low-level input path; Windows imposes a timeout on
//! it. It therefore does NO locking, NO blocking, and NO heap-heavy work beyond
//! building one tiny JSON payload. State it needs is read from atomics; the
//! `AppHandle` used to emit is stored once in a `OnceLock`. If emit fails we
//! drop the event silently.

use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Once, OnceLock};

use tauri::{AppHandle, Emitter, State};

use crate::state::AppState;

/// True while type mode is on. The hook proc gates every keystroke on this;
/// `start`/`stop` flip it. Also the sole switch that makes the always-installed
/// hook transparent when type mode is off.
static ACTIVE: AtomicBool = AtomicBool::new(false);

/// Shift state, tracked from the hook itself (down on either Shift keydown, up
/// on its keyup). Used by `translate_vk` for letter casing. Reset on `stop` so
/// a Shift held when the overlay closes can't leave a stuck modifier here.
static SHIFT_DOWN: AtomicBool = AtomicBool::new(false);

/// The app handle the hook proc emits `overlay-type` through. Set once, on the
/// first `start`. `get()` in the proc is lock-free.
static APP: OnceLock<AppHandle> = OnceLock::new();

/// Guards the one-time spawn of the hook thread.
static THREAD_SPAWN: Once = Once::new();

/// True once the LL keyboard hook actually installed. Set from the first
/// `start`'s bounded wait on the hook thread's ack. If it stays false the hook
/// never went in and type mode can't capture anything — `start` returns Err so
/// the frontend falls back to needs-label instead of silently doing nothing.
static HOOK_INSTALLED: AtomicBool = AtomicBool::new(false);

/// How long the first `start` waits for the hook thread to confirm install.
/// Bounded so an async command never hangs; install is a single Win32 call and
/// completes in microseconds, so 200ms is generous slack, not a real delay.
const INSTALL_ACK_TIMEOUT_MS: u64 = 200;

/// Enable type mode. Idempotent: the first call spawns the dedicated hook
/// thread, remembers the app handle, and waits (briefly) for the hook to
/// confirm it installed; every later call just re-checks install state and arms
/// `ACTIVE`. Returns Err if the hook never installed so callers can fall back.
pub fn start(app: AppHandle) -> Result<(), String> {
    // Remember the handle for the hook proc (first start wins).
    let _ = APP.set(app);

    // Spawn the pump thread exactly once. It installs the hook, acks whether the
    // install succeeded, then lives for the rest of the process.
    let mut first_spawn = false;
    let mut ack_rx: Option<std::sync::mpsc::Receiver<bool>> = None;
    THREAD_SPAWN.call_once(|| {
        first_spawn = true;
        let (tx, rx) = std::sync::mpsc::channel::<bool>();
        ack_rx = Some(rx);
        if let Err(e) = std::thread::Builder::new()
            .name("gkey-keyhook".into())
            .spawn(move || hook_thread_main(tx))
        {
            log::error!("keyhook: failed to spawn hook thread: {}", e);
            // The spawn failed, so `tx` was dropped with the closure; the
            // recv_timeout below will see a disconnected channel and treat it
            // as an install failure.
        }
    });

    if first_spawn {
        // Bounded, non-hanging wait for the hook thread's install ack. A dropped
        // sender (spawn failed) or a timeout both resolve to "not installed".
        let installed = ack_rx
            .and_then(|rx| {
                rx.recv_timeout(std::time::Duration::from_millis(INSTALL_ACK_TIMEOUT_MS))
                    .ok()
            })
            .unwrap_or(false);
        HOOK_INSTALLED.store(installed, Ordering::SeqCst);
        if !installed {
            return Err("keyboard hook failed to install".to_string());
        }
    } else if !HOOK_INSTALLED.load(Ordering::SeqCst) {
        // A prior start already determined the hook can't be installed.
        return Err("keyboard hook is not installed".to_string());
    }

    ACTIVE.store(true, Ordering::SeqCst);
    Ok(())
}

/// Disable type mode. Idempotent — safe to call when type mode was never
/// started (the hook thread simply may not exist yet; `ACTIVE` stays false).
/// Also clears tracked Shift so a modifier held at close can't stick.
pub fn stop() {
    ACTIVE.store(false, Ordering::SeqCst);
    SHIFT_DOWN.store(false, Ordering::SeqCst);
}

/// The hook thread: install the LL keyboard hook once, then pump messages
/// forever. `GetMessageW` blocks the thread while keeping the hook serviced.
fn hook_thread_main(ack: std::sync::mpsc::Sender<bool>) {
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetMessageW, SetWindowsHookExW, MSG, WH_KEYBOARD_LL,
    };

    unsafe {
        let hinstance = GetModuleHandleW(ptr::null());
        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), hinstance, 0);
        if hook.is_null() {
            log::error!("keyhook: SetWindowsHookExW failed");
            let _ = ack.send(false);
            return;
        }
        // Confirm the install to the waiting `start` before entering the pump.
        let _ = ack.send(true);

        // Message pump keeps the hook alive. An LL keyboard hook delivers its
        // callbacks on this thread; the messages themselves need no dispatch.
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
            // No TranslateMessage/DispatchMessage needed for a bare LL hook.
        }
        // Unreachable in practice (we never post WM_QUIT), but if the pump ever
        // exits, drop the hook cleanly.
        use windows_sys::Win32::UI::WindowsAndMessaging::UnhookWindowsHookEx;
        UnhookWindowsHookEx(hook);
    }
}

/// Emit an `overlay-type` payload to the frontend. Lock-free, non-blocking;
/// errors are dropped (the proc must never block on a slow/absent frontend).
#[inline]
fn emit(payload: serde_json::Value) {
    if let Some(app) = APP.get() {
        let _ = app.emit("overlay-type", payload);
    }
}

/// Is this vkCode one of the Shift keys (either side, or the generic VK_SHIFT)?
/// PURE (no OS calls, no state) so it is unit-testable.
#[inline]
pub(crate) fn is_shift(vk: u32) -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{VK_LSHIFT, VK_RSHIFT, VK_SHIFT};
    vk == VK_SHIFT as u32 || vk == VK_LSHIFT as u32 || vk == VK_RSHIFT as u32
}

/// Is this vkCode ANY keyboard modifier — Shift, Ctrl, Alt (Menu), or a Win
/// key, generic or side-specific? While type mode swallows every ordinary key,
/// modifiers must pass through (`CallNextHookEx`) so the game never sees a
/// half-delivered chord: swallowing a Ctrl/Alt/Win keydown while its keyup goes
/// through (or vice versa) leaves that modifier stuck "down" in the game.
/// PURE (no OS calls, no state) so it is unit-testable.
#[inline]
pub(crate) fn is_modifier(vk: u32) -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LWIN, VK_MENU, VK_RCONTROL, VK_RMENU, VK_RWIN,
    };
    is_shift(vk)
        || vk == VK_CONTROL as u32
        || vk == VK_LCONTROL as u32
        || vk == VK_RCONTROL as u32
        || vk == VK_MENU as u32
        || vk == VK_LMENU as u32
        || vk == VK_RMENU as u32
        || vk == VK_LWIN as u32
        || vk == VK_RWIN as u32
}

/// The low-level keyboard hook procedure.
///
/// Contract (see module docs): the FIRST line passes through untouched when
/// `code < 0` or type mode is not ACTIVE. When ACTIVE, pure modifier keys pass
/// through (so the game never sees a stuck/blocked Shift) but EVERY other key
/// is swallowed — mapped keys and the Enter/Esc/Backspace sentinels emit an
/// `overlay-type` event first, unmapped keys are swallowed silently.
unsafe extern "system" fn hook_proc(
    code: i32,
    wparam: windows_sys::Win32::Foundation::WPARAM,
    lparam: windows_sys::Win32::Foundation::LPARAM,
) -> windows_sys::Win32::Foundation::LRESULT {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{VK_BACK, VK_ESCAPE, VK_RETURN};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, KBDLLHOOKSTRUCT, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
    };

    // Pass-through fast path: not our concern, or type mode off.
    if code < 0 || !ACTIVE.load(Ordering::SeqCst) {
        return CallNextHookEx(ptr::null_mut(), code, wparam, lparam);
    }

    let kb = &*(lparam as *const KBDLLHOOKSTRUCT);
    let vk = kb.vkCode;
    let msg = wparam as u32;

    match msg {
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            // Pure modifiers (Shift/Ctrl/Alt/Win, either side) pass through —
            // swallowing a modifier keydown while letting its keyup through (or
            // vice versa) leaves that modifier stuck "down" in the game. Shift
            // additionally maintains SHIFT_DOWN for letter casing.
            if is_modifier(vk) {
                if is_shift(vk) {
                    SHIFT_DOWN.store(true, Ordering::SeqCst);
                }
                return CallNextHookEx(ptr::null_mut(), code, wparam, lparam);
            }

            if vk == VK_RETURN as u32 {
                emit(serde_json::json!({ "kind": "enter" }));
            } else if vk == VK_ESCAPE as u32 {
                emit(serde_json::json!({ "kind": "esc" }));
                // Hard keyboard-recovery hatch: Esc ALWAYS frees the keyboard,
                // even if the webview is dead and never round-trips a
                // stopTypeMode back to us. Inline the same atomic stores as
                // stop() (never call anything that could block in the hook
                // proc). Happy path is unchanged — the frontend's stopTypeMode
                // is idempotent, so a live overlay just stops twice.
                ACTIVE.store(false, Ordering::SeqCst);
                SHIFT_DOWN.store(false, Ordering::SeqCst);
            } else if vk == VK_BACK as u32 {
                emit(serde_json::json!({ "kind": "backspace" }));
            } else if let Some(ch) = translate_vk(vk, SHIFT_DOWN.load(Ordering::SeqCst)) {
                emit(serde_json::json!({ "kind": "char", "ch": ch.to_string() }));
            }
            // Every non-modifier key is swallowed while ACTIVE (mapped or not).
            1
        }
        WM_KEYUP | WM_SYSKEYUP => {
            // Symmetric with the keydown: modifiers pass through (and Shift
            // clears its tracked state), everything else is swallowed.
            if is_modifier(vk) {
                if is_shift(vk) {
                    SHIFT_DOWN.store(false, Ordering::SeqCst);
                }
                return CallNextHookEx(ptr::null_mut(), code, wparam, lparam);
            }
            1
        }
        _ => CallNextHookEx(ptr::null_mut(), code, wparam, lparam),
    }
}

/// Translate a virtual-key code to the character it types in the overlay.
///
/// PURE (no OS calls, no state) so it is unit-testable. Deliberately narrow —
/// only what a clip label/description needs: letters (case from `shift`),
/// digits (shift ignored — plain digits), space, `-`/`_`, `.`, and the numpad
/// digits. Everything else returns `None` and is swallowed silently.
pub(crate) fn translate_vk(vk: u32, shift: bool) -> Option<char> {
    const VK_SPACE: u32 = 0x20;
    const VK_OEM_MINUS: u32 = 0xBD;
    const VK_OEM_PERIOD: u32 = 0xBE;

    match vk {
        // 'A'..='Z'
        0x41..=0x5A => {
            let offset = (vk - 0x41) as u8;
            let base = if shift { b'A' } else { b'a' };
            Some((base + offset) as char)
        }
        // '0'..='9' (shift ignored — plain digits, no shifted symbols)
        0x30..=0x39 => Some((b'0' + (vk - 0x30) as u8) as char),
        // Numpad 0..9
        0x60..=0x69 => Some((b'0' + (vk - 0x60) as u8) as char),
        VK_SPACE => Some(' '),
        VK_OEM_MINUS => Some(if shift { '_' } else { '-' }),
        VK_OEM_PERIOD => Some('.'),
        _ => None,
    }
}

/// Enable type mode. Errors unless the user opted in via
/// `overlay_typing_enabled` (short config lock).
#[tauri::command]
pub fn start_type_mode(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    {
        let s = state.lock().map_err(|e| e.to_string())?;
        if !s.config.overlay_typing_enabled {
            return Err("typing in overlay is disabled".to_string());
        }
    }
    start(app)
}

/// Disable type mode. Also called defensively by `overlay::close`, so closing
/// the overlay always ends type mode.
#[tauri::command]
pub fn stop_type_mode() {
    stop();
}

#[cfg(test)]
mod tests {
    use super::{is_modifier, translate_vk};

    #[test]
    fn all_modifiers_are_passthrough() {
        // Shift/Ctrl/Alt/Win — generic and both sides — must all be modifiers
        // so type mode passes them through and never strands a game chord.
        for vk in [
            0x10, // VK_SHIFT
            0xA0, // VK_LSHIFT
            0xA1, // VK_RSHIFT
            0x11, // VK_CONTROL
            0xA2, // VK_LCONTROL
            0xA3, // VK_RCONTROL
            0x12, // VK_MENU (Alt)
            0xA4, // VK_LMENU
            0xA5, // VK_RMENU
            0x5B, // VK_LWIN
            0x5C, // VK_RWIN
        ] {
            assert!(is_modifier(vk), "vk {:#x} should be a modifier", vk);
        }
    }

    #[test]
    fn ordinary_keys_are_not_modifiers() {
        for vk in [
            0x41, // 'A'
            0x30, // '0'
            0x20, // Space
            0x0D, // Enter
            0x1B, // Esc
            0x08, // Backspace
            0x09, // Tab
            0x70, // F1
        ] {
            assert!(!is_modifier(vk), "vk {:#x} should not be a modifier", vk);
        }
    }

    #[test]
    fn letters_lowercase_without_shift() {
        assert_eq!(translate_vk(0x41, false), Some('a')); // A
        assert_eq!(translate_vk(0x5A, false), Some('z')); // Z
        assert_eq!(translate_vk(0x4D, false), Some('m')); // M
    }

    #[test]
    fn letters_uppercase_with_shift() {
        assert_eq!(translate_vk(0x41, true), Some('A'));
        assert_eq!(translate_vk(0x5A, true), Some('Z'));
        assert_eq!(translate_vk(0x4D, true), Some('M'));
    }

    #[test]
    fn digits_ignore_shift() {
        for (vk, ch) in [(0x30u32, '0'), (0x35, '5'), (0x39, '9')] {
            assert_eq!(translate_vk(vk, false), Some(ch));
            assert_eq!(translate_vk(vk, true), Some(ch)); // shift ignored
        }
    }

    #[test]
    fn numpad_digits() {
        assert_eq!(translate_vk(0x60, false), Some('0'));
        assert_eq!(translate_vk(0x65, false), Some('5'));
        assert_eq!(translate_vk(0x69, true), Some('9')); // shift ignored
    }

    #[test]
    fn space() {
        assert_eq!(translate_vk(0x20, false), Some(' '));
        assert_eq!(translate_vk(0x20, true), Some(' '));
    }

    #[test]
    fn minus_and_underscore() {
        assert_eq!(translate_vk(0xBD, false), Some('-'));
        assert_eq!(translate_vk(0xBD, true), Some('_'));
    }

    #[test]
    fn period() {
        assert_eq!(translate_vk(0xBE, false), Some('.'));
        assert_eq!(translate_vk(0xBE, true), Some('.'));
    }

    #[test]
    fn unmapped_keys_return_none() {
        assert_eq!(translate_vk(0x70, false), None); // F1
        assert_eq!(translate_vk(0x09, false), None); // TAB
        assert_eq!(translate_vk(0x0D, false), None); // ENTER (handled elsewhere)
        assert_eq!(translate_vk(0x1B, false), None); // ESC (handled elsewhere)
        assert_eq!(translate_vk(0x08, false), None); // BACKSPACE (handled elsewhere)
        assert_eq!(translate_vk(0xA0, false), None); // LSHIFT (modifier)
    }
}
