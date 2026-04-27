use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetMessageW, PostThreadMessageW, MSG, WM_HOTKEY, WM_USER,
};

/// Custom message ID our listener thread responds to to swap bindings.
/// Values >= WM_USER are reserved for application use.
const WM_HOTKEY_RELOAD: u32 = WM_USER + 1;

/// Lets other threads swap the registered hotkeys at runtime. Cloned
/// freely; all clones target the same listener thread.
#[derive(Clone)]
pub struct HotkeyController {
    thread_id: u32,
    pending: Arc<Mutex<Option<Vec<(HotkeyAction, String)>>>>,
}

impl HotkeyController {
    /// Replace the current bindings. The listener thread will unregister
    /// all old hotkeys and register the new set on its next message-pump
    /// iteration. Safe to call from any thread.
    pub fn reload(&self, bindings: Vec<(HotkeyAction, String)>) {
        *self.pending.lock().unwrap() = Some(bindings);
        unsafe {
            PostThreadMessageW(self.thread_id, WM_HOTKEY_RELOAD, 0, 0);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HotkeyAction {
    MoveG1,
    MoveG2,
    MoveG3,
    Rename,
    RestartWatcher,
    /// Fired when the user presses the configured `save_clip_bind`. We use it
    /// as a watcher health probe: the user's capture software just saved a
    /// clip, so a `FileCreated` event should arrive within a few seconds. If
    /// not, the watcher is wedged and we restart it + rescan the folder.
    SaveClipHealthCheck,
    /// Toggle the count-up stopwatch — running ↔ reset-stopped-at-zero.
    CountUpToggle,
}

#[derive(Debug, Clone)]
pub struct HotkeyBinding {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub vk_code: u32,
}

pub fn parse_hotkey(binding: &str) -> Result<HotkeyBinding, String> {
    let parts: Vec<&str> = binding.split('+').collect();
    if parts.len() < 2 {
        return Err(format!("Invalid hotkey binding: '{}'", binding));
    }

    let mut ctrl = false;
    let mut alt = false;
    let mut shift = false;

    // All parts except the last are modifiers; the last is the key name
    let key_name = parts[parts.len() - 1];
    let modifier_parts = &parts[..parts.len() - 1];

    for part in modifier_parts {
        match part.to_lowercase().as_str() {
            "ctrl" | "control" => ctrl = true,
            "alt" => alt = true,
            "shift" => shift = true,
            other => return Err(format!("Unknown modifier: '{}'", other)),
        }
    }

    let vk_code = key_name_to_vk(key_name)?;

    Ok(HotkeyBinding {
        ctrl,
        alt,
        shift,
        vk_code,
    })
}

pub fn key_name_to_vk(name: &str) -> Result<u32, String> {
    let lower = name.to_lowercase();
    // Single ASCII letter a-z → VK 0x41..0x5A
    if lower.len() == 1 {
        let c = lower.chars().next().unwrap();
        if c.is_ascii_alphabetic() {
            return Ok(0x41 + (c.to_ascii_uppercase() as u32 - 'A' as u32));
        }
        if c.is_ascii_digit() {
            return Ok(0x30 + (c as u32 - '0' as u32));
        }
    }
    match lower.as_str() {
        "f1" => Ok(0x70),
        "f2" => Ok(0x71),
        "f3" => Ok(0x72),
        "f4" => Ok(0x73),
        "f5" => Ok(0x74),
        "f6" => Ok(0x75),
        "f7" => Ok(0x76),
        "f8" => Ok(0x77),
        "f9" => Ok(0x78),
        "f10" => Ok(0x79),
        "f11" => Ok(0x7A),
        "f12" => Ok(0x7B),
        "f13" => Ok(0x7C),
        "f14" => Ok(0x7D),
        "f15" => Ok(0x7E),
        "f16" => Ok(0x7F),
        "f17" => Ok(0x80),
        "f18" => Ok(0x81),
        "f19" => Ok(0x82),
        "f20" => Ok(0x83),
        "f21" => Ok(0x84),
        "f22" => Ok(0x85),
        "f23" => Ok(0x86),
        "f24" => Ok(0x87),
        // Common named keys produced by KeybindInput (e.target.key strings).
        "space" | " " => Ok(0x20),
        "enter" => Ok(0x0D),
        "tab" => Ok(0x09),
        "backspace" => Ok(0x08),
        "delete" => Ok(0x2E),
        "insert" => Ok(0x2D),
        "home" => Ok(0x24),
        "end" => Ok(0x23),
        "pageup" => Ok(0x21),
        "pagedown" => Ok(0x22),
        "arrowleft" => Ok(0x25),
        "arrowup" => Ok(0x26),
        "arrowright" => Ok(0x27),
        "arrowdown" => Ok(0x28),
        "`" => Ok(0xC0),
        "-" => Ok(0xBD),
        "=" => Ok(0xBB),
        "[" => Ok(0xDB),
        "]" => Ok(0xDD),
        "\\" => Ok(0xDC),
        ";" => Ok(0xBA),
        "'" => Ok(0xDE),
        "," => Ok(0xBC),
        "." => Ok(0xBE),
        "/" => Ok(0xBF),
        other => Err(format!("Unknown key name: '{}'", other)),
    }
}

/// Build the hotkey-binding list from a config snapshot. Empty/unset
/// binds are skipped so we don't attempt to register zero-length keys.
pub fn bindings_from_config(config: &crate::config::AppConfig) -> Vec<(HotkeyAction, String)> {
    let mut bindings = vec![
        (HotkeyAction::MoveG1, config.g1_bind.clone()),
        (HotkeyAction::MoveG2, config.g2_bind.clone()),
        (HotkeyAction::MoveG3, config.g3_bind.clone()),
        (HotkeyAction::Rename, config.rename_bind.clone()),
        (
            HotkeyAction::RestartWatcher,
            config.restart_watcher_bind.clone(),
        ),
    ];
    if !config.save_clip_bind.is_empty() {
        bindings.push((HotkeyAction::SaveClipHealthCheck, config.save_clip_bind.clone()));
    }
    if !config.count_up_bind.is_empty() {
        bindings.push((HotkeyAction::CountUpToggle, config.count_up_bind.clone()));
    }
    bindings.into_iter().filter(|(_, s)| !s.is_empty()).collect()
}

/// Register a fresh set of hotkeys, replacing any previously registered
/// IDs in `registered`. Must be called from the message-pump thread —
/// RegisterHotKey is per-thread.
fn apply_bindings(
    bindings: Vec<(HotkeyAction, String)>,
    registered: &mut Vec<(i32, HotkeyAction)>,
) {
    // Unregister existing IDs first.
    for (id, _) in registered.drain(..) {
        unsafe {
            UnregisterHotKey(std::ptr::null_mut(), id);
        }
    }

    for (idx, (action, binding_str)) in bindings.into_iter().enumerate() {
        let hotkey_id = (idx + 1) as i32; // IDs must be > 0

        let binding = match parse_hotkey(&binding_str) {
            Ok(b) => b,
            Err(e) => {
                log::warn!("Failed to parse hotkey binding '{}': {}", binding_str, e);
                continue;
            }
        };

        let mut modifiers: u32 = MOD_NOREPEAT as u32;
        if binding.ctrl {
            modifiers |= MOD_CONTROL as u32;
        }
        if binding.alt {
            modifiers |= MOD_ALT as u32;
        }
        if binding.shift {
            modifiers |= MOD_SHIFT as u32;
        }

        let result = unsafe {
            RegisterHotKey(
                std::ptr::null_mut(),
                hotkey_id,
                modifiers,
                binding.vk_code,
            )
        };

        if result == 0 {
            log::warn!(
                "Failed to register hotkey for action {:?} (binding: '{}')",
                action,
                binding_str
            );
        } else {
            log::info!(
                "Registered hotkey id={} for action {:?}",
                hotkey_id,
                action
            );
            registered.push((hotkey_id, action));
        }
    }
}

pub fn spawn_hotkey_listener(
    initial_bindings: Vec<(HotkeyAction, String)>,
) -> Result<(mpsc::Receiver<HotkeyAction>, HotkeyController), String> {
    let (tx, rx) = mpsc::channel::<HotkeyAction>(32);
    let pending: Arc<Mutex<Option<Vec<(HotkeyAction, String)>>>> = Arc::new(Mutex::new(None));
    let pending_clone = pending.clone();

    // Use a sync channel so we can hand the listener thread's GetCurrentThreadId
    // back to the controller before this function returns.
    let (tid_tx, tid_rx) = std::sync::mpsc::sync_channel::<u32>(1);

    std::thread::spawn(move || {
        // Capture our own thread ID so other threads can target us with
        // PostThreadMessageW.
        use windows_sys::Win32::System::Threading::GetCurrentThreadId;
        let tid = unsafe { GetCurrentThreadId() };
        let _ = tid_tx.send(tid);

        let mut registered: Vec<(i32, HotkeyAction)> = Vec::new();
        apply_bindings(initial_bindings, &mut registered);

        loop {
            let mut msg: MSG = unsafe { std::mem::zeroed() };
            let result = unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) };

            if result <= 0 {
                // 0 = WM_QUIT, -1 = error
                break;
            }

            match msg.message {
                WM_HOTKEY => {
                    let hotkey_id = msg.wParam as i32;
                    if let Some((_, action)) = registered.iter().find(|(id, _)| *id == hotkey_id) {
                        if let Err(e) = tx.blocking_send(action.clone()) {
                            log::warn!("Failed to send hotkey action: {}", e);
                            break;
                        }
                    }
                }
                WM_HOTKEY_RELOAD => {
                    // Take pending bindings and re-register.
                    let new_bindings = pending_clone.lock().unwrap().take();
                    if let Some(bindings) = new_bindings {
                        log::info!("Reloading hotkey bindings ({} entries)", bindings.len());
                        apply_bindings(bindings, &mut registered);
                    }
                }
                _ => {}
            }
        }

        // Cleanup
        for (hotkey_id, _) in &registered {
            unsafe {
                UnregisterHotKey(std::ptr::null_mut(), *hotkey_id);
            }
        }
    });

    let thread_id = tid_rx
        .recv()
        .map_err(|e| format!("Hotkey listener didn't report its thread id: {}", e))?;

    Ok((rx, HotkeyController { thread_id, pending }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hotkey_ctrl_f13() {
        let result = parse_hotkey("ctrl+F13").unwrap();
        assert!(result.ctrl);
        assert!(!result.alt);
        assert!(!result.shift);
        assert_eq!(result.vk_code, 0x7C);
    }

    #[test]
    fn test_parse_hotkey_alt_f13() {
        let result = parse_hotkey("alt+F13").unwrap();
        assert!(!result.ctrl);
        assert!(result.alt);
        assert_eq!(result.vk_code, 0x7C);
    }

    #[test]
    fn test_parse_hotkey_ctrl_shift_f12() {
        let result = parse_hotkey("ctrl+shift+F12").unwrap();
        assert!(result.ctrl);
        assert!(result.shift);
        assert_eq!(result.vk_code, 0x7B);
    }

    #[test]
    fn test_parse_hotkey_invalid_key() {
        let result = parse_hotkey("ctrl+XYZ");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_hotkey_no_key() {
        // "ctrl+alt" — "alt" will be treated as key name, not found in vk map
        let result = parse_hotkey("ctrl+alt");
        assert!(result.is_err());
    }

    #[test]
    fn test_key_name_to_vk_all_f_keys() {
        assert_eq!(key_name_to_vk("f1").unwrap(), 0x70);
        assert_eq!(key_name_to_vk("f12").unwrap(), 0x7B);
        assert_eq!(key_name_to_vk("f13").unwrap(), 0x7C);
        assert_eq!(key_name_to_vk("f24").unwrap(), 0x87);
    }
}
