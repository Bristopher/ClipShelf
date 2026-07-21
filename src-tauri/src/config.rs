use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::theme::Theme;

// --- Default value functions ---

fn default_screen_capture_software() -> String { "obs".to_string() }
fn default_videos_folder() -> String { "".to_string() }
fn default_log_file_enabled() -> bool { true }
fn default_g1_bind() -> String { "ctrl+F13".to_string() }
fn default_g2_bind() -> String { "ctrl+F14".to_string() }
fn default_g3_bind() -> String { "ctrl+F15".to_string() }
fn default_rename_bind() -> String { "alt+F13".to_string() }
fn default_restart_watcher_bind() -> String { "ctrl+shift+F12".to_string() }
fn default_g1_bind_folder_name() -> String { "!! or ! (G1)".to_string() }
fn default_g2_bind_folder_name() -> String { "odd or checked (G2)".to_string() }
fn default_g3_bind_folder_name() -> String { "!!! (G3)".to_string() }
fn default_clip_save_sound_enabled() -> bool { false }
fn default_clip_save_sound_custom() -> Option<String> { None }
fn default_move_sound_enabled() -> bool { false }
fn default_error_sound_enabled() -> bool { true }
fn default_error_sound_custom() -> Option<String> { None }
fn default_timer_enabled() -> bool { true }
fn default_timer_duration_ms() -> u64 { 70000 }
fn default_auto_wipe_enabled() -> bool { true }
fn default_disable_file_movesorting() -> bool { true }
fn default_obs_websocket_enabled() -> bool { false }
fn default_obs_websocket_password() -> String { "".to_string() }
fn default_window_opacity() -> f64 { 1.0 }
fn default_hover_full_opacity() -> bool { true }
fn default_active_theme_id() -> String { "dark".to_string() }
fn default_themes() -> Vec<Theme> { Vec::new() }
fn default_save_clip_bind() -> String { "".to_string() }
fn default_timer_flash_enabled() -> bool { true }
fn default_save_clip_health_check_timeout_secs() -> u32 { 5 }
fn default_timer_flash_theme_id() -> Option<String> { None }
fn default_count_up_bind() -> String { "ctrl+shift+B".to_string() }
fn default_small_file_warn_mb() -> f64 { 6.5 }
fn default_undo_bind() -> String { "".to_string() }
fn default_autostart_enabled() -> bool { false }
fn default_remember_window_layout() -> bool { true }
fn default_monitor() -> u32 { 2 }
fn default_anchor() -> String { "top-left".to_string() }
fn default_rename_mru() -> Vec<String> { Vec::new() }
fn default_game_detection_enabled() -> bool { true }
fn default_check_updates() -> bool { true }
fn default_click_through_enabled() -> bool { true }
fn default_click_through_key() -> String { "ctrl".to_string() }
fn default_write_file_properties() -> bool { true }
fn default_day_rollover_hour() -> u8 { 4 }
fn default_game_overrides() -> Vec<GameOverride> { Vec::new() }
fn default_overlay_enabled() -> bool { true }
fn default_overlay_bind() -> String { "shift+F1".to_string() }
fn default_overlay_typing_enabled() -> bool { true }
fn default_overlay_wasd_nav() -> bool { false }
fn default_label_presets() -> Vec<String> { vec!["clutch".to_string(), "ace".to_string(), "funny".to_string(), "fail".to_string()] }
fn default_description_presets() -> Vec<String> { Vec::new() }

/// Max entries kept in the rename most-recently-used list.
pub const RENAME_MRU_MAX: usize = 8;

/// exe stem → display-name override, remembered when the user corrects a
/// wrong detection. A Vec of structs, NOT a HashMap: TOML rejects non-string
/// map keys and the list form renders cleanly in the config file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameOverride {
    pub exe: String,
    pub name: String,
}

// --- AppConfig struct ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_screen_capture_software")]
    pub screen_capture_software: String,

    #[serde(default = "default_videos_folder")]
    pub videos_folder: String,

    #[serde(default = "default_log_file_enabled")]
    pub log_file_enabled: bool,

    #[serde(default = "default_g1_bind")]
    pub g1_bind: String,

    #[serde(default = "default_g2_bind")]
    pub g2_bind: String,

    #[serde(default = "default_g3_bind")]
    pub g3_bind: String,

    #[serde(default = "default_rename_bind")]
    pub rename_bind: String,

    #[serde(default = "default_restart_watcher_bind")]
    pub restart_watcher_bind: String,

    #[serde(default = "default_g1_bind_folder_name")]
    pub g1_bind_folder_name: String,

    #[serde(default = "default_g2_bind_folder_name")]
    pub g2_bind_folder_name: String,

    #[serde(default = "default_g3_bind_folder_name")]
    pub g3_bind_folder_name: String,

    #[serde(default = "default_clip_save_sound_enabled")]
    pub clip_save_sound_enabled: bool,

    #[serde(default = "default_clip_save_sound_custom")]
    pub clip_save_sound_custom: Option<String>,

    #[serde(default = "default_move_sound_enabled")]
    pub move_sound_enabled: bool,

    #[serde(default = "default_error_sound_enabled")]
    pub error_sound_enabled: bool,

    #[serde(default = "default_error_sound_custom")]
    pub error_sound_custom: Option<String>,

    #[serde(default = "default_timer_enabled")]
    pub timer_enabled: bool,

    #[serde(default = "default_timer_duration_ms")]
    pub timer_duration_ms: u64,

    #[serde(default = "default_auto_wipe_enabled")]
    pub auto_wipe_enabled: bool,

    #[serde(default = "default_disable_file_movesorting")]
    pub disable_file_movesorting: bool,

    #[serde(default = "default_obs_websocket_enabled")]
    pub obs_websocket_enabled: bool,

    #[serde(default = "default_obs_websocket_password")]
    pub obs_websocket_password: String,

    #[serde(default = "default_window_opacity")]
    pub window_opacity: f64,

    #[serde(default = "default_hover_full_opacity")]
    pub hover_full_opacity: bool,

    #[serde(default = "default_active_theme_id")]
    pub active_theme_id: String,

    #[serde(default = "default_themes")]
    pub themes: Vec<Theme>,

    /// Keybind the user presses in their capture app (OBS / ShadowPlay) to
    /// save a clip. Not currently listened to by ClipShelf — informational
    /// for the user, and hooked in future phases to surface "no clip
    /// arrived" errors in the event log.
    #[serde(default = "default_save_clip_bind")]
    pub save_clip_bind: String,

    /// When true, flash the whole window with inverted colors every second
    /// once the countdown drops to 5 or fewer seconds — very hard to miss.
    #[serde(default = "default_timer_flash_enabled")]
    pub timer_flash_enabled: bool,

    /// How long to wait after the user presses `save_clip_bind` before
    /// declaring the watcher unhealthy and rescanning the folder. Hardware-
    /// dependent: SSDs flush in ~1s, slow HDDs or long replay buffers can
    /// take 5-10s. Use the calibration tool in settings to measure yours.
    #[serde(default = "default_save_clip_health_check_timeout_secs")]
    pub save_clip_health_check_timeout_secs: u32,

    /// When the ≤5s timer flash fires, swap to this theme instead of the
    /// CSS-invert effect. `None` = auto-pick: light active theme → dark, dark
    /// → light. Setting an id here overrides the auto-pick with the user's
    /// explicit choice (any built-in or custom theme id).
    #[serde(default = "default_timer_flash_theme_id")]
    pub timer_flash_theme_id: Option<String>,

    /// Hotkey for the count-up stopwatch. First press starts at 0, second
    /// press resets to 0 and stops, third press starts again. Empty values
    /// are migrated to the default on load — use `disabled_binds` to turn
    /// the hotkey off instead of clearing it.
    #[serde(default = "default_count_up_bind")]
    pub count_up_bind: String,

    /// Bind-field names (e.g. "count_up_bind") whose global hotkey is
    /// individually toggled off in Settings. Kept as a list so the master
    /// toggle below can flip everything off and back WITHOUT losing which
    /// individual binds the user had disabled.
    #[serde(default)]
    pub disabled_binds: Vec<String>,

    /// Master hotkey kill-switch: when true, NO global hotkeys register at
    /// all. Independent of `disabled_binds`, so toggling this back on
    /// restores exactly the per-bind states from before.
    #[serde(default)]
    pub hotkeys_disabled: bool,

    /// Clips smaller than this (MB) get a "possible black screen" warning +
    /// error sound. Depends on bitrate and replay-buffer length, so it's
    /// tunable rather than the old hardcoded 6.5.
    #[serde(default = "default_small_file_warn_mb")]
    pub small_file_warn_mb: f64,

    /// Hotkey to undo the last move/rename. Empty = not registered. Users
    /// should pick something rare — plain Ctrl+Z is registered GLOBALLY and
    /// would swallow undo in every other app.
    #[serde(default = "default_undo_bind")]
    pub undo_bind: String,

    /// Launch ClipShelf automatically at Windows login.
    #[serde(default = "default_autostart_enabled")]
    pub autostart_enabled: bool,

    /// Restore the last window position/size on launch. The layout itself is
    /// persisted separately in window_layout.toml so the Settings draft/save
    /// model can't clobber a fresher auto-saved layout.
    #[serde(default = "default_remember_window_layout")]
    pub remember_window_layout: bool,

    /// 1-based monitor for the default open position (clamped to the number
    /// of connected monitors at runtime).
    #[serde(default = "default_monitor")]
    pub default_monitor: u32,

    /// Corner anchor for the default open position: "top-left", "top-right",
    /// "bottom-left", "bottom-right", or "center".
    #[serde(default = "default_anchor")]
    pub default_anchor: String,

    /// Most-recently-used rename texts, newest first (cap RENAME_MRU_MAX).
    /// Maintained by the backend on every successful rename; the rename
    /// dialog shows these as one-click chips.
    #[serde(default = "default_rename_mru")]
    pub rename_mru: Vec<String>,

    /// Detect the focused game/app when a clip is saved and record it in
    /// history + file properties. Master switch for the whole feature.
    #[serde(default = "default_game_detection_enabled")]
    pub game_detection_enabled: bool,

    /// Check GitHub for a newer release on launch and offer it (never
    /// installs silently — the user always confirms via dialog).
    #[serde(default = "default_check_updates")]
    pub check_updates: bool,

    /// Hold-to-click-through: while `click_through_key` is physically held,
    /// mouse clicks pass straight through the (semi-transparent) main window
    /// to whatever is underneath, without minimizing or unfocusing it.
    #[serde(default = "default_click_through_enabled")]
    pub click_through_enabled: bool,

    /// Which modifier activates click-through while held: "ctrl", "alt", or
    /// "shift". NOTE: while held, in-app <mod>+Click actions are unreachable
    /// (the click never lands on the window) — pick a modifier you don't use
    /// inside the app.
    #[serde(default = "default_click_through_key")]
    pub click_through_key: String,

    /// Mirror game/rating/description into Windows file properties
    /// (Explorer Tags/Rating/Comments). history.jsonl is written regardless.
    #[serde(default = "default_write_file_properties")]
    pub write_file_properties: bool,

    /// Hour (0-23) at which "today" starts for history and daily stats —
    /// default 4 AM so a late-night session doesn't split at midnight.
    #[serde(default = "default_day_rollover_hour")]
    pub day_rollover_hour: u8,

    /// User corrections for misdetected games, checked before heuristics.
    #[serde(default = "default_game_overrides")]
    pub game_overrides: Vec<GameOverride>,

    /// Master switch for the in-game overlay.
    #[serde(default = "default_overlay_enabled")]
    pub overlay_enabled: bool,

    /// Global hotkey that toggles the overlay. Note: game sees the Shift keydown
    /// (modifier leak).
    #[serde(default = "default_overlay_bind")]
    pub overlay_bind: String,

    /// Allow the LL-hook type mode (captures the keyboard while a text field
    /// is open).
    #[serde(default = "default_overlay_typing_enabled")]
    pub overlay_typing_enabled: bool,

    /// Also bind W/S/A/D as arrow-key aliases while the overlay is open.
    /// Off by default — the keys are only ever registered while the overlay
    /// panel is up, but some players still don't want movement keys touched.
    #[serde(default = "default_overlay_wasd_nav")]
    pub overlay_wasd_nav: bool,

    /// One-keypress label chips in the overlay.
    #[serde(default = "default_label_presets")]
    pub label_presets: Vec<String>,

    /// One-keypress description chips in the overlay.
    #[serde(default = "default_description_presets")]
    pub description_presets: Vec<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            screen_capture_software: default_screen_capture_software(),
            videos_folder: default_videos_folder(),
            log_file_enabled: default_log_file_enabled(),
            g1_bind: default_g1_bind(),
            g2_bind: default_g2_bind(),
            g3_bind: default_g3_bind(),
            rename_bind: default_rename_bind(),
            restart_watcher_bind: default_restart_watcher_bind(),
            g1_bind_folder_name: default_g1_bind_folder_name(),
            g2_bind_folder_name: default_g2_bind_folder_name(),
            g3_bind_folder_name: default_g3_bind_folder_name(),
            clip_save_sound_enabled: default_clip_save_sound_enabled(),
            clip_save_sound_custom: default_clip_save_sound_custom(),
            move_sound_enabled: default_move_sound_enabled(),
            error_sound_enabled: default_error_sound_enabled(),
            error_sound_custom: default_error_sound_custom(),
            timer_enabled: default_timer_enabled(),
            timer_duration_ms: default_timer_duration_ms(),
            auto_wipe_enabled: default_auto_wipe_enabled(),
            disable_file_movesorting: default_disable_file_movesorting(),
            obs_websocket_enabled: default_obs_websocket_enabled(),
            obs_websocket_password: default_obs_websocket_password(),
            window_opacity: default_window_opacity(),
            hover_full_opacity: default_hover_full_opacity(),
            active_theme_id: default_active_theme_id(),
            themes: default_themes(),
            save_clip_bind: default_save_clip_bind(),
            timer_flash_enabled: default_timer_flash_enabled(),
            save_clip_health_check_timeout_secs: default_save_clip_health_check_timeout_secs(),
            timer_flash_theme_id: default_timer_flash_theme_id(),
            count_up_bind: default_count_up_bind(),
            disabled_binds: Vec::new(),
            hotkeys_disabled: false,
            small_file_warn_mb: default_small_file_warn_mb(),
            undo_bind: default_undo_bind(),
            autostart_enabled: default_autostart_enabled(),
            remember_window_layout: default_remember_window_layout(),
            default_monitor: default_monitor(),
            default_anchor: default_anchor(),
            rename_mru: default_rename_mru(),
            game_detection_enabled: default_game_detection_enabled(),
            check_updates: default_check_updates(),
            click_through_enabled: default_click_through_enabled(),
            click_through_key: default_click_through_key(),
            write_file_properties: default_write_file_properties(),
            day_rollover_hour: default_day_rollover_hour(),
            game_overrides: default_game_overrides(),
            overlay_enabled: default_overlay_enabled(),
            overlay_bind: default_overlay_bind(),
            overlay_typing_enabled: default_overlay_typing_enabled(),
            overlay_wasd_nav: default_overlay_wasd_nav(),
            label_presets: default_label_presets(),
            description_presets: default_description_presets(),
        }
    }
}

impl AppConfig {
    /// Returns the config file path next to the executable.
    pub fn config_path() -> PathBuf {
        let exe = std::env::current_exe().expect("failed to get current exe path");
        exe.parent()
            .expect("exe has no parent directory")
            .join("gkey_config.toml")
    }

    /// Loads config from a specific path. Returns defaults if file doesn't exist.
    pub fn load_from(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = std::fs::read_to_string(path)?;
        let mut config: Self = toml::from_str(&contents)?;
        // Migration (2026-07-20): count-up used to default to "" (= not
        // registered); it now defaults to Ctrl+Shift+B, and turning a
        // hotkey off is done via `disabled_binds` instead of clearing the
        // combo. Configs saved before this carry the old empty string —
        // upgrade them in place.
        if config.count_up_bind.trim().is_empty() {
            config.count_up_bind = default_count_up_bind();
        }
        Ok(config)
    }

    /// Saves config to a specific path.
    ///
    /// Atomic: writes a sibling temp file then renames it over the real one,
    /// so a crash or power cut mid-save can never leave a truncated TOML
    /// (which the next launch would silently replace with defaults, wiping
    /// every user setting).
    pub fn save_to(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let contents = toml::to_string_pretty(self)?;
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, contents)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    /// Merges a JSON object into the config, updating only the fields present in the JSON.
    /// Errors (leaving the config unchanged) if the merged result doesn't
    /// deserialize back into a valid AppConfig — callers must not report a
    /// successful save when the update was actually discarded.
    pub fn merge_partial(&mut self, partial: serde_json::Value) -> Result<(), String> {
        let obj = match partial.as_object() {
            Some(o) => o,
            None => return Err("partial config update is not a JSON object".to_string()),
        };

        // Serialize current config to a JSON Value, apply partial overrides, then deserialize back.
        let mut current = serde_json::to_value(&*self).unwrap_or(serde_json::Value::Object(Default::default()));
        if let Some(current_obj) = current.as_object_mut() {
            for (k, v) in obj {
                current_obj.insert(k.clone(), v.clone());
            }
        }
        match serde_json::from_value::<AppConfig>(current) {
            Ok(updated) => {
                *self = updated;
                Ok(())
            }
            Err(e) => Err(format!("invalid config update: {}", e)),
        }
    }

    /// Returns timer duration in whole seconds.
    pub fn timer_duration_secs(&self) -> u64 {
        self.timer_duration_ms / 1000
    }

    /// Returns timer duration formatted as "MM:SS". (Currently only tests
    /// use it — cfg(test) rather than deleted because it documents the
    /// duration format.)
    #[cfg(test)]
    pub fn timer_display(&self) -> String {
        let total_secs = self.timer_duration_secs();
        let mins = total_secs / 60;
        let secs = total_secs % 60;
        format!("{:02}:{:02}", mins, secs)
    }

    /// Record a rename text in the MRU list: newest first, case-insensitive
    /// dedupe, capped at RENAME_MRU_MAX.
    pub fn push_rename_mru(&mut self, text: &str) {
        let lower = text.to_lowercase();
        self.rename_mru.retain(|t| t.to_lowercase() != lower);
        self.rename_mru.insert(0, text.to_string());
        self.rename_mru.truncate(RENAME_MRU_MAX);
    }

    /// Upsert a detection override (case-insensitive on exe stem).
    pub fn remember_game_override(&mut self, exe: &str, name: &str) {
        let lower = exe.to_lowercase();
        self.game_overrides.retain(|o| o.exe.to_lowercase() != lower);
        self.game_overrides.push(GameOverride { exe: exe.to_string(), name: name.to_string() });
    }

    /// Returns the sort folder path for a given G-key number (1, 2, or 3).
    /// Path: videos_folder/sort/AHK sort/{folder_name}
    pub fn sort_folder_path(&self, gkey: u8) -> PathBuf {
        let folder_name = match gkey {
            1 => &self.g1_bind_folder_name,
            2 => &self.g2_bind_folder_name,
            3 => &self.g3_bind_folder_name,
            _ => panic!("gkey must be 1, 2, or 3"),
        };
        PathBuf::from(&self.videos_folder)
            .join("sort")
            .join("AHK sort")
            .join(folder_name)
    }
}

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_default_config_values() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.screen_capture_software, "obs");
        assert_eq!(cfg.videos_folder, "");
        assert!(cfg.log_file_enabled);
        assert_eq!(cfg.g1_bind, "ctrl+F13");
        assert_eq!(cfg.g2_bind, "ctrl+F14");
        assert_eq!(cfg.g3_bind, "ctrl+F15");
        assert_eq!(cfg.rename_bind, "alt+F13");
        assert_eq!(cfg.restart_watcher_bind, "ctrl+shift+F12");
        assert_eq!(cfg.g1_bind_folder_name, "!! or ! (G1)");
        assert_eq!(cfg.g2_bind_folder_name, "odd or checked (G2)");
        assert_eq!(cfg.g3_bind_folder_name, "!!! (G3)");
        assert!(!cfg.clip_save_sound_enabled);
        assert!(cfg.clip_save_sound_custom.is_none());
        assert!(!cfg.move_sound_enabled);
        assert!(cfg.error_sound_enabled);
        assert!(cfg.error_sound_custom.is_none());
        assert!(cfg.timer_enabled);
        assert_eq!(cfg.timer_duration_ms, 70000);
        assert!(cfg.auto_wipe_enabled);
        assert!(cfg.disable_file_movesorting);
        assert!(!cfg.obs_websocket_enabled);
        assert_eq!(cfg.obs_websocket_password, "");
        assert_eq!(cfg.small_file_warn_mb, 6.5);
    }

    #[test]
    fn test_load_missing_file_returns_defaults() {
        let path = Path::new("/nonexistent/path/that/does/not/exist/gkey_config.toml");
        let cfg = AppConfig::load_from(path).expect("load_from should succeed for missing file");
        // Should be default values
        assert_eq!(cfg.screen_capture_software, "obs");
        assert_eq!(cfg.timer_duration_ms, 70000);
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let tmp = NamedTempFile::new().expect("failed to create temp file");
        let path = tmp.path();

        let mut original = AppConfig::default();
        original.screen_capture_software = "shadowplay".to_string();
        original.videos_folder = "C:/Videos".to_string();
        original.timer_duration_ms = 30000;
        original.obs_websocket_password = "secret123".to_string();

        original.save_to(path).expect("save_to failed");

        let loaded = AppConfig::load_from(path).expect("load_from failed");
        assert_eq!(loaded.screen_capture_software, "shadowplay");
        assert_eq!(loaded.videos_folder, "C:/Videos");
        assert_eq!(loaded.timer_duration_ms, 30000);
        assert_eq!(loaded.obs_websocket_password, "secret123");
        // Untouched defaults should still be correct
        assert_eq!(loaded.g1_bind, "ctrl+F13");
        assert!(loaded.auto_wipe_enabled);
    }

    #[test]
    fn test_load_partial_toml_uses_defaults_for_missing() {
        let tmp = NamedTempFile::new().expect("failed to create temp file");
        let path = tmp.path();

        // Write only a subset of fields
        let partial_toml = r#"
screen_capture_software = "shadowplay"
timer_duration_ms = 45000
"#;
        std::fs::write(path, partial_toml).expect("write failed");

        let cfg = AppConfig::load_from(path).expect("load_from failed");
        assert_eq!(cfg.screen_capture_software, "shadowplay");
        assert_eq!(cfg.timer_duration_ms, 45000);
        // Missing fields should be defaults
        assert_eq!(cfg.videos_folder, "");
        assert_eq!(cfg.g1_bind, "ctrl+F13");
        assert!(cfg.log_file_enabled);
        assert!(cfg.error_sound_enabled);
    }

    #[test]
    fn test_merge_partial() {
        let mut cfg = AppConfig::default();
        assert_eq!(cfg.screen_capture_software, "obs");
        assert_eq!(cfg.timer_duration_ms, 70000);

        let partial = serde_json::json!({
            "screen_capture_software": "shadowplay",
            "timer_duration_ms": 90000u64,
            "obs_websocket_password": "newpass"
        });

        cfg.merge_partial(partial).expect("merge should succeed");

        assert_eq!(cfg.screen_capture_software, "shadowplay");
        assert_eq!(cfg.timer_duration_ms, 90000);
        assert_eq!(cfg.obs_websocket_password, "newpass");
        // Untouched fields remain at defaults
        assert_eq!(cfg.g1_bind, "ctrl+F13");
        assert!(cfg.auto_wipe_enabled);
    }

    #[test]
    fn test_merge_partial_rejects_bad_types_and_leaves_config_unchanged() {
        let mut cfg = AppConfig::default();
        let partial = serde_json::json!({ "timer_duration_ms": "not a number" });

        let result = cfg.merge_partial(partial);

        assert!(result.is_err());
        assert_eq!(cfg.timer_duration_ms, 70000);
    }

    #[test]
    fn test_push_rename_mru_dedupes_and_caps() {
        let mut cfg = AppConfig::default();
        assert!(cfg.rename_mru.is_empty());

        cfg.push_rename_mru("clutch");
        cfg.push_rename_mru("ace");
        // Case-insensitive dedupe: re-adding moves it to the front with the
        // newest casing.
        cfg.push_rename_mru("Clutch");
        assert_eq!(cfg.rename_mru, vec!["Clutch", "ace"]);

        for i in 0..10 {
            cfg.push_rename_mru(&format!("entry{}", i));
        }
        assert_eq!(cfg.rename_mru.len(), RENAME_MRU_MAX);
        assert_eq!(cfg.rename_mru[0], "entry9");
    }

    #[test]
    fn test_timer_display() {
        let mut cfg = AppConfig::default();

        cfg.timer_duration_ms = 70000;
        assert_eq!(cfg.timer_display(), "01:10");

        cfg.timer_duration_ms = 30000;
        assert_eq!(cfg.timer_display(), "00:30");

        cfg.timer_duration_ms = 120000;
        assert_eq!(cfg.timer_display(), "02:00");
    }

    #[test]
    fn test_sort_folder_path() {
        let mut cfg = AppConfig::default();
        cfg.videos_folder = "C:/Videos".to_string();

        let p1 = cfg.sort_folder_path(1);
        assert_eq!(p1, PathBuf::from("C:/Videos/sort/AHK sort/!! or ! (G1)"));

        let p2 = cfg.sort_folder_path(2);
        assert_eq!(p2, PathBuf::from("C:/Videos/sort/AHK sort/odd or checked (G2)"));

        let p3 = cfg.sort_folder_path(3);
        assert_eq!(p3, PathBuf::from("C:/Videos/sort/AHK sort/!!! (G3)"));
    }

    #[test]
    fn test_game_detection_defaults() {
        let cfg = AppConfig::default();
        assert!(cfg.game_detection_enabled);
        assert!(cfg.write_file_properties);
        assert_eq!(cfg.day_rollover_hour, 4);
        assert!(cfg.game_overrides.is_empty());
    }

    #[test]
    fn test_game_overrides_toml_roundtrip() {
        let tmp = NamedTempFile::new().unwrap();
        let mut cfg = AppConfig::default();
        cfg.remember_game_override("cs2", "Counter-Strike 2");
        cfg.save_to(tmp.path()).expect("save");
        let loaded = AppConfig::load_from(tmp.path()).expect("load");
        assert_eq!(loaded.game_overrides.len(), 1);
        assert_eq!(loaded.game_overrides[0].exe, "cs2");
        assert_eq!(loaded.game_overrides[0].name, "Counter-Strike 2");
    }

    #[test]
    fn test_remember_game_override_upserts_case_insensitive() {
        let mut cfg = AppConfig::default();
        cfg.remember_game_override("CS2", "Wrong Name");
        cfg.remember_game_override("cs2", "Counter-Strike 2");
        assert_eq!(cfg.game_overrides.len(), 1);
        assert_eq!(cfg.game_overrides[0].name, "Counter-Strike 2");
    }

    #[test]
    fn test_hotkey_toggle_defaults_and_count_up_migration() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.count_up_bind, "ctrl+shift+B");
        assert!(cfg.disabled_binds.is_empty());
        assert!(!cfg.hotkeys_disabled);

        // Pre-migration configs stored "" for "no count-up hotkey" — load
        // must upgrade that to the new default (disabling now goes through
        // disabled_binds instead).
        let tmp = NamedTempFile::new().expect("failed to create temp file");
        let mut original = AppConfig::default();
        original.count_up_bind = String::new();
        original.disabled_binds = vec!["g2_bind".to_string()];
        original.hotkeys_disabled = true;
        original.save_to(tmp.path()).expect("save_to failed");
        let loaded = AppConfig::load_from(tmp.path()).expect("load_from failed");
        assert_eq!(loaded.count_up_bind, "ctrl+shift+B");
        assert_eq!(loaded.disabled_binds, vec!["g2_bind".to_string()]);
        assert!(loaded.hotkeys_disabled);
    }

    #[test]
    fn test_overlay_config_defaults() {
        let cfg = AppConfig::default();
        assert!(cfg.overlay_enabled);
        assert_eq!(cfg.overlay_bind, "shift+F1");
        assert!(cfg.overlay_typing_enabled);
        assert!(!cfg.overlay_wasd_nav);
        assert_eq!(cfg.label_presets, vec!["clutch", "ace", "funny", "fail"]);
        assert!(cfg.description_presets.is_empty());
    }

    #[test]
    fn test_overlay_presets_toml_roundtrip() {
        let tmp = NamedTempFile::new().expect("failed to create temp file");
        let path = tmp.path();

        let mut original = AppConfig::default();
        original.overlay_enabled = false;
        original.overlay_bind = "ctrl+shift+O".to_string();
        original.overlay_typing_enabled = false;
        original.label_presets = vec!["custom1".to_string(), "custom2".to_string()];
        original.description_presets = vec!["desc1".to_string(), "desc2".to_string(), "desc3".to_string()];

        original.save_to(path).expect("save_to failed");

        let loaded = AppConfig::load_from(path).expect("load_from failed");
        assert!(!loaded.overlay_enabled);
        assert_eq!(loaded.overlay_bind, "ctrl+shift+O");
        assert!(!loaded.overlay_typing_enabled);
        assert_eq!(loaded.label_presets, vec!["custom1", "custom2"]);
        assert_eq!(loaded.description_presets, vec!["desc1", "desc2", "desc3"]);
    }
}
