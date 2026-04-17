use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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
fn default_windows_notification_enabled() -> bool { false }
fn default_timer_enabled() -> bool { true }
fn default_timer_duration_ms() -> u64 { 70000 }
fn default_auto_wipe_enabled() -> bool { true }
fn default_disable_file_movesorting() -> bool { true }
fn default_obs_websocket_enabled() -> bool { false }
fn default_obs_websocket_password() -> String { "".to_string() }
fn default_shadowplay_folder() -> Option<String> { None }
fn default_prompt_capture_software() -> bool { false }
fn default_window_opacity() -> f64 { 1.0 }

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

    #[serde(default = "default_windows_notification_enabled")]
    pub windows_notification_enabled: bool,

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

    #[serde(default = "default_shadowplay_folder")]
    pub shadowplay_folder: Option<String>,

    #[serde(default = "default_prompt_capture_software")]
    pub prompt_capture_software: bool,

    #[serde(default = "default_window_opacity")]
    pub window_opacity: f64,
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
            windows_notification_enabled: default_windows_notification_enabled(),
            timer_enabled: default_timer_enabled(),
            timer_duration_ms: default_timer_duration_ms(),
            auto_wipe_enabled: default_auto_wipe_enabled(),
            disable_file_movesorting: default_disable_file_movesorting(),
            obs_websocket_enabled: default_obs_websocket_enabled(),
            obs_websocket_password: default_obs_websocket_password(),
            shadowplay_folder: default_shadowplay_folder(),
            prompt_capture_software: default_prompt_capture_software(),
            window_opacity: default_window_opacity(),
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
    pub fn save_to(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let contents = toml::to_string_pretty(self)?;
        std::fs::write(path, contents)?;
        Ok(())
    }

    /// Merges a JSON object into the config, updating only the fields present in the JSON.
    pub fn merge_partial(&mut self, partial: serde_json::Value) {
        let obj = match partial.as_object() {
            Some(o) => o,
            None => return,
        };

        // Serialize current config to a JSON Value, apply partial overrides, then deserialize back.
        let mut current = serde_json::to_value(&*self).unwrap_or(serde_json::Value::Object(Default::default()));
        if let Some(current_obj) = current.as_object_mut() {
            for (k, v) in obj {
                current_obj.insert(k.clone(), v.clone());
            }
        }
        if let Ok(updated) = serde_json::from_value::<AppConfig>(current) {
            *self = updated;
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
        assert!(!cfg.windows_notification_enabled);
        assert!(cfg.timer_enabled);
        assert_eq!(cfg.timer_duration_ms, 70000);
        assert!(cfg.auto_wipe_enabled);
        assert!(cfg.disable_file_movesorting);
        assert!(!cfg.obs_websocket_enabled);
        assert_eq!(cfg.obs_websocket_password, "");
        assert!(cfg.shadowplay_folder.is_none());
        assert!(!cfg.prompt_capture_software);
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
        original.shadowplay_folder = Some("C:/Shadowplay".to_string());

        original.save_to(path).expect("save_to failed");

        let loaded = AppConfig::load_from(path).expect("load_from failed");
        assert_eq!(loaded.screen_capture_software, "shadowplay");
        assert_eq!(loaded.videos_folder, "C:/Videos");
        assert_eq!(loaded.timer_duration_ms, 30000);
        assert_eq!(loaded.obs_websocket_password, "secret123");
        assert_eq!(loaded.shadowplay_folder, Some("C:/Shadowplay".to_string()));
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

        cfg.merge_partial(partial);

        assert_eq!(cfg.screen_capture_software, "shadowplay");
        assert_eq!(cfg.timer_duration_ms, 90000);
        assert_eq!(cfg.obs_websocket_password, "newpass");
        // Untouched fields remain at defaults
        assert_eq!(cfg.g1_bind, "ctrl+F13");
        assert!(cfg.auto_wipe_enabled);
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
