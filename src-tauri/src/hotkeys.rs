use tokio::sync::mpsc;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetMessageW, MSG, WM_HOTKEY};

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
    match name.to_lowercase().as_str() {
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
        "0" => Ok(0x30),
        "1" => Ok(0x31),
        "2" => Ok(0x32),
        "3" => Ok(0x33),
        "4" => Ok(0x34),
        "5" => Ok(0x35),
        "6" => Ok(0x36),
        "7" => Ok(0x37),
        "8" => Ok(0x38),
        "9" => Ok(0x39),
        other => Err(format!("Unknown key name: '{}'", other)),
    }
}

pub fn spawn_hotkey_listener(
    bindings: Vec<(HotkeyAction, String)>,
) -> Result<mpsc::Receiver<HotkeyAction>, String> {
    let (tx, rx) = mpsc::channel::<HotkeyAction>(32);

    std::thread::spawn(move || {
        // Parse bindings and associate each with an ID
        let mut registered: Vec<(i32, HotkeyAction)> = Vec::new();

        for (id, (action, binding_str)) in bindings.into_iter().enumerate() {
            let hotkey_id = (id + 1) as i32; // IDs must be > 0

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

        // Message pump
        loop {
            let mut msg: MSG = unsafe { std::mem::zeroed() };
            let result = unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) };

            if result <= 0 {
                // 0 = WM_QUIT, -1 = error
                break;
            }

            if msg.message == WM_HOTKEY {
                let hotkey_id = msg.wParam as i32;
                if let Some((_, action)) = registered.iter().find(|(id, _)| *id == hotkey_id) {
                    if let Err(e) = tx.blocking_send(action.clone()) {
                        log::warn!("Failed to send hotkey action: {}", e);
                        break;
                    }
                }
            }
        }

        // Cleanup
        for (hotkey_id, _) in &registered {
            unsafe {
                UnregisterHotKey(std::ptr::null_mut(), *hotkey_id);
            }
        }
    });

    Ok(rx)
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
