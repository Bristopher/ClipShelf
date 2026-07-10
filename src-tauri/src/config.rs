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
fn default_count_up_bind() -> String { "".to_string() }
fn default_small_file_warn_mb() -> f64 { 6.5 }
fn default_undo_bind() -> String { "".to_string() }
fn default_autostart_enabled() -> bool { false }
fn default_remember_window_layout() -> bool { true }
fn default_monitor() -> u32 { 2 }
fn default_anchor() -> String { "top-left".to_string() }
fn default_rename_mru() -> Vec<String> { Vec::new() }

/// Max entries kept in the rename most-recently-used list.
pub const RENAME_MRU_MAX: usize = 8;

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
    /// save a clip. Not currently listened to by GKey Mover — informational
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
    /// press resets to 0 and stops, third press starts again. Empty = no
    /// hotkey registered.
    #[serde(default = "default_count_up_bind")]
    pub count_up_bind: String,

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

    /// Launch GKey Mover automatically at Windows login.
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
            small_file_warn_mb: default_small_file_warn_mb(),
            undo_bind: default_undo_bind(),
            autostart_enabled: default_autostart_enabled(),
            remember_window_layout: default_remember_window_layout(),
            default_monitor: default_monitor(),
            default_anchor: default_anchor(),
            rename_mru: default_rename_mru(),
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

    /// Loads config from the default path next to the executable.
    /// Returns defaults if the file does not exist.
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        Self::load_from(&Self::config_path())
    }

    /// Loads config from a specific path. Returns defaults if file doesn't exist.
    pub fn load_from(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&contents)?;
        Ok(config)
    }

    /// Saves config to the default path next to the executable.
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.save_to(&Self::config_path())
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

    /// Returns timer duration formatted as "MM:SS".
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
}
