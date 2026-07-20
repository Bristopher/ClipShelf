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

/// The controller's own view of what should currently be registered: the
/// full "base" bind list (G-keys, rename, overlay toggle, ...) plus whether
/// the overlay is open (which adds the temporary digit/Esc keys). Both
/// `reload` and `set_overlay_keys` mutate this, recompose the effective list,
/// and hand it to the listener thread.
struct ControllerInner {
    base: Vec<(HotkeyAction, String)>,
    overlay_active: bool,
    /// Whether the overlay temp keys also alias W/S/A/D to the four arrows
    /// (opt-in Settings toggle; only meaningful while `overlay_active`).
    overlay_wasd: bool,
}

/// Lets other threads swap the registered hotkeys at runtime. Cloned
/// freely; all clones target the same listener thread.
#[derive(Clone)]
pub struct HotkeyController {
    thread_id: u32,
    inner: Arc<Mutex<ControllerInner>>,
    pending: Arc<Mutex<Option<Vec<(HotkeyAction, String)>>>>,
}

impl HotkeyController {
    /// Replace the current BASE bindings (everything except the overlay's
    /// temporary digit keys). The listener thread will unregister all old
    /// hotkeys and register the new set on its next message-pump iteration.
    /// If the overlay is open, its temp keys are preserved. Safe to call
    /// from any thread.
    pub fn reload(&self, bindings: Vec<(HotkeyAction, String)>) {
        // Mutate AND publish while holding the `inner` lock: composing under
        // the lock but posting after releasing it would let a concurrent
        // reload/set_overlay_keys publish out of order (e.g. digits staying
        // registered after the overlay closed). Lock order is always
        // inner → pending; nothing acquires them the other way.
        let mut inner = self.inner.lock().unwrap();
        inner.base = bindings;
        self.post(compose(&inner));
    }

    /// Toggle the overlay's temporary keys. When `active`, the listener ALSO
    /// registers plain "1".."9", "0", and Esc mapped to `OverlayKey(..)`;
    /// when inactive those are unregistered (base binds untouched). A no-op
    /// if the flag already matches, so re-opening/re-closing doesn't churn
    /// the base registrations.
    pub fn set_overlay_keys(&self, active: bool, wasd: bool) {
        // Same atomicity requirement as `reload` — see comment there.
        let mut inner = self.inner.lock().unwrap();
        if inner.overlay_active == active && inner.overlay_wasd == wasd {
            return;
        }
        inner.overlay_active = active;
        inner.overlay_wasd = wasd;
        self.post(compose(&inner));
    }

    /// Publish a composed bind list to the listener thread. Callers MUST hold
    /// the `inner` lock for the duration of this call so mutation + publish
    /// is one atomic step (inner → pending is the only lock order).
    fn post(&self, composed: Vec<(HotkeyAction, String)>) {
        *self.pending.lock().unwrap() = Some(composed);
        unsafe {
            PostThreadMessageW(self.thread_id, WM_HOTKEY_RELOAD, 0, 0);
        }
    }
}

/// Compose the effective bind list = base + (overlay temp keys if active).
fn compose(inner: &ControllerInner) -> Vec<(HotkeyAction, String)> {
    let mut list = inner.base.clone();
    if inner.overlay_active {
        list.extend(overlay_temp_bindings(inner.overlay_wasd));
    }
    list
}

/// The temporary keys registered while the overlay is open: bare digits
/// "1".."9" → `OverlayKey(1..=9)`, "0" → `OverlayKey(0)`, "escape" →
/// `OverlayKey(10)` (the Esc sentinel that closes the overlay), arrows
/// "up"/"down"/"left"/"right" → `OverlayKey(11..=14)` (menu + thumbnail-strip
/// navigation), and "enter" → `OverlayKey(15)` (activate the highlighted
/// row). With `wasd` (opt-in Settings toggle) W/S/A/D are also bound as
/// aliases of the four arrows. Bare keys (no modifier) — they only exist
/// while the overlay panel is up, so they never touch gameplay input outside
/// the menu.
pub fn overlay_temp_bindings(wasd: bool) -> Vec<(HotkeyAction, String)> {
    let mut v: Vec<(HotkeyAction, String)> = (1u8..=9)
        .map(|n| (HotkeyAction::OverlayKey(n), n.to_string()))
        .collect();
    v.push((HotkeyAction::OverlayKey(0), "0".to_string()));
    v.push((HotkeyAction::OverlayKey(10), "escape".to_string()));
    v.push((HotkeyAction::OverlayKey(11), "up".to_string()));
    v.push((HotkeyAction::OverlayKey(12), "down".to_string()));
    v.push((HotkeyAction::OverlayKey(13), "left".to_string()));
    v.push((HotkeyAction::OverlayKey(14), "right".to_string()));
    v.push((HotkeyAction::OverlayKey(15), "enter".to_string()));
    if wasd {
        v.push((HotkeyAction::OverlayKey(11), "w".to_string()));
        v.push((HotkeyAction::OverlayKey(12), "s".to_string()));
        v.push((HotkeyAction::OverlayKey(13), "a".to_string()));
        v.push((HotkeyAction::OverlayKey(14), "d".to_string()));
    }
    v
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
    /// Undo the last move/rename.
    Undo,
    /// Show/hide the in-game overlay. User-configured global bind.
    OverlayToggle,
    /// A key pressed while the overlay is open. `1..=9` and `0` are the digit
    /// selections; `10` is the Esc sentinel (closes the overlay); `11..=14`
    /// are Up/Down/Left/Right (menu + thumbnail-strip navigation, optionally
    /// aliased to W/S/A/D); `15` is Enter (activate highlighted row). These
    /// are registered as bare temporary hotkeys only while the overlay is up.
    OverlayKey(u8),
}

impl HotkeyAction {
    /// Human-readable name for UI-facing failure messages.
    pub fn label(&self) -> &'static str {
        match self {
            HotkeyAction::MoveG1 => "G1 move",
            HotkeyAction::MoveG2 => "G2 move",
            HotkeyAction::MoveG3 => "G3 move",
            HotkeyAction::Rename => "Rename",
            HotkeyAction::RestartWatcher => "Restart watcher",
            HotkeyAction::SaveClipHealthCheck => "Save-clip health check",
            HotkeyAction::CountUpToggle => "Count-up toggle",
            HotkeyAction::Undo => "Undo",
            HotkeyAction::OverlayToggle => "Overlay toggle",
            HotkeyAction::OverlayKey(_) => "Overlay key",
        }
    }
}

/// A hotkey that failed to parse or register. Surfaced to the UI — a silent
/// failure here means a G-key just stops working after a Settings save (e.g.
/// another app already owns the combo) with no visible indication.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyRegFailure {
    pub action: String,
    pub binding: String,
    pub reason: String,
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
    // A bare single key (no modifier) is valid — the overlay's temporary keys
    // register plain "1".."9"/"0"/"escape". The last part is always the key
    // name; an empty key name still fails below via key_name_to_vk.
    if binding.is_empty() {
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
        "escape" | "esc" => Ok(0x1B),
        "enter" => Ok(0x0D),
        "tab" => Ok(0x09),
        "backspace" => Ok(0x08),
        "delete" => Ok(0x2E),
        "insert" => Ok(0x2D),
        "home" => Ok(0x24),
        "end" => Ok(0x23),
        "pageup" => Ok(0x21),
        "pagedown" => Ok(0x22),
        "arrowleft" | "left" => Ok(0x25),
        "arrowup" | "up" => Ok(0x26),
        "arrowright" | "right" => Ok(0x27),
        "arrowdown" | "down" => Ok(0x28),
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
    if !config.undo_bind.is_empty() {
        bindings.push((HotkeyAction::Undo, config.undo_bind.clone()));
    }
    if config.overlay_enabled && !config.overlay_bind.is_empty() {
        bindings.push((HotkeyAction::OverlayToggle, config.overlay_bind.clone()));
    }
    bindings.into_iter().filter(|(_, s)| !s.is_empty()).collect()
}

/// Register a fresh set of hotkeys, replacing any previously registered
/// IDs in `registered`. Must be called from the message-pump thread —
/// RegisterHotKey is per-thread. Returns the bindings that failed so the
/// caller can surface them to the UI.
fn apply_bindings(
    bindings: Vec<(HotkeyAction, String)>,
    registered: &mut Vec<(i32, HotkeyAction)>,
) -> Vec<HotkeyRegFailure> {
    // Unregister existing IDs first.
    for (id, _) in registered.drain(..) {
        unsafe {
            UnregisterHotKey(std::ptr::null_mut(), id);
        }
    }

    let mut failures = Vec::new();

    for (idx, (action, binding_str)) in bindings.into_iter().enumerate() {
        let hotkey_id = (idx + 1) as i32; // IDs must be > 0

        let binding = match parse_hotkey(&binding_str) {
            Ok(b) => b,
            Err(e) => {
                log::warn!("Failed to parse hotkey binding '{}': {}", binding_str, e);
                failures.push(HotkeyRegFailure {
                    action: action.label().to_string(),
                    binding: binding_str,
                    reason: e,
                });
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
            // Overlay temp digit/Esc keys are bare, globally-common keys that
            // another running app can legitimately hold. A failure here must
            // NOT reach the user-facing failure toast (it isn't a broken user
            // bind) — log a warning and carry on so the toggle still works.
            if matches!(action, HotkeyAction::OverlayKey(_)) {
                log::warn!(
                    "overlay keys: could not register {:?} (binding: '{}') — likely held by another app; skipping",
                    action,
                    binding_str
                );
                continue;
            }
            log::warn!(
                "Failed to register hotkey for action {:?} (binding: '{}')",
                action,
                binding_str
            );
            failures.push(HotkeyRegFailure {
                action: action.label().to_string(),
                binding: binding_str,
                reason: "already in use by another application (or invalid combo)"
                    .to_string(),
            });
        } else {
            log::info!(
                "Registered hotkey id={} for action {:?}",
                hotkey_id,
                action
            );
            registered.push((hotkey_id, action));
        }
    }

    failures
}

pub fn spawn_hotkey_listener(
    initial_bindings: Vec<(HotkeyAction, String)>,
) -> Result<
    (
        mpsc::Receiver<HotkeyAction>,
        mpsc::Receiver<Vec<HotkeyRegFailure>>,
        HotkeyController,
    ),
    String,
> {
    let (tx, rx) = mpsc::channel::<HotkeyAction>(32);
    let (failure_tx, failure_rx) = mpsc::channel::<Vec<HotkeyRegFailure>>(8);
    let pending: Arc<Mutex<Option<Vec<(HotkeyAction, String)>>>> = Arc::new(Mutex::new(None));
    let pending_clone = pending.clone();
    let inner = Arc::new(Mutex::new(ControllerInner {
        base: initial_bindings.clone(),
        overlay_active: false,
        overlay_wasd: false,
    }));

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
        let failures = apply_bindings(initial_bindings, &mut registered);
        if !failures.is_empty() {
            let _ = failure_tx.blocking_send(failures);
        }

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
                        let failures = apply_bindings(bindings, &mut registered);
                        if !failures.is_empty() {
                            let _ = failure_tx.blocking_send(failures);
                        }
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

    Ok((rx, failure_rx, HotkeyController { thread_id, inner, pending }))
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

    #[test]
    fn test_key_name_to_vk_digits() {
        // "0".."9" map to VK 0x30..0x39.
        assert_eq!(key_name_to_vk("0").unwrap(), 0x30);
        assert_eq!(key_name_to_vk("1").unwrap(), 0x31);
        assert_eq!(key_name_to_vk("9").unwrap(), 0x39);
    }

    #[test]
    fn test_key_name_to_vk_escape() {
        assert_eq!(key_name_to_vk("escape").unwrap(), 0x1B);
        assert_eq!(key_name_to_vk("esc").unwrap(), 0x1B);
        assert_eq!(key_name_to_vk("ESCAPE").unwrap(), 0x1B);
    }

    #[test]
    fn test_key_name_to_vk_up_down() {
        // Overlay arrow-key temp binds use the bare "up"/"down" names (not
        // the DOM "arrowup"/"arrowdown" strings, which are also supported).
        assert_eq!(key_name_to_vk("up").unwrap(), 0x26);
        assert_eq!(key_name_to_vk("arrowup").unwrap(), 0x26);
        assert_eq!(key_name_to_vk("down").unwrap(), 0x28);
        assert_eq!(key_name_to_vk("arrowdown").unwrap(), 0x28);
    }

    #[test]
    fn test_parse_hotkey_bare_key() {
        // Overlay temp keys are modifier-less; parse must accept them.
        let d = parse_hotkey("1").unwrap();
        assert!(!d.ctrl && !d.alt && !d.shift);
        assert_eq!(d.vk_code, 0x31);

        let esc = parse_hotkey("escape").unwrap();
        assert_eq!(esc.vk_code, 0x1B);
    }

    #[test]
    fn test_overlay_temp_bindings_shape() {
        let b = overlay_temp_bindings(false);
        // 1-9, 0, escape, up, down, left, right, enter = 16 entries.
        assert_eq!(b.len(), 16);
        assert_eq!(b[0], (HotkeyAction::OverlayKey(1), "1".to_string()));
        assert_eq!(b[8], (HotkeyAction::OverlayKey(9), "9".to_string()));
        assert_eq!(b[9], (HotkeyAction::OverlayKey(0), "0".to_string()));
        assert_eq!(b[10], (HotkeyAction::OverlayKey(10), "escape".to_string()));
        assert_eq!(b[11], (HotkeyAction::OverlayKey(11), "up".to_string()));
        assert_eq!(b[12], (HotkeyAction::OverlayKey(12), "down".to_string()));
        assert_eq!(b[13], (HotkeyAction::OverlayKey(13), "left".to_string()));
        assert_eq!(b[14], (HotkeyAction::OverlayKey(14), "right".to_string()));
        assert_eq!(b[15], (HotkeyAction::OverlayKey(15), "enter".to_string()));
        // Every temp binding must parse (else it can never register).
        for (_, s) in &b {
            assert!(parse_hotkey(s).is_ok(), "temp bind '{}' must parse", s);
        }
    }

    #[test]
    fn test_overlay_temp_bindings_wasd_aliases() {
        let b = overlay_temp_bindings(true);
        // Base 16 + W/S/A/D aliases = 20 entries, aliased to the arrow codes.
        assert_eq!(b.len(), 20);
        assert_eq!(b[16], (HotkeyAction::OverlayKey(11), "w".to_string()));
        assert_eq!(b[17], (HotkeyAction::OverlayKey(12), "s".to_string()));
        assert_eq!(b[18], (HotkeyAction::OverlayKey(13), "a".to_string()));
        assert_eq!(b[19], (HotkeyAction::OverlayKey(14), "d".to_string()));
        for (_, s) in &b {
            assert!(parse_hotkey(s).is_ok(), "temp bind '{}' must parse", s);
        }
    }

    #[test]
    fn test_bindings_from_config_includes_overlay_toggle() {
        let mut cfg = crate::config::AppConfig::default();
        cfg.overlay_enabled = true;
        cfg.overlay_bind = "shift+F1".to_string();
        let binds = bindings_from_config(&cfg);
        assert!(binds
            .iter()
            .any(|(a, s)| *a == HotkeyAction::OverlayToggle && s == "shift+F1"));

        // Disabled → no toggle bind.
        cfg.overlay_enabled = false;
        let binds = bindings_from_config(&cfg);
        assert!(!binds.iter().any(|(a, _)| *a == HotkeyAction::OverlayToggle));
    }
}
