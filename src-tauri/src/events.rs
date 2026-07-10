use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Info,
    Warning,
    Error,
    Success,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LogCategory {
    FileCreated,
    FileMoved,
    FileRenamed,
    HotkeyPressed,
    WatcherStatus,
    ObsWebSocket,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntryPayload {
    pub timestamp: String,
    pub level: LogLevel,
    pub message: String,
    pub category: LogCategory,
    /// File this entry refers to (created/moved/renamed clips). Present so
    /// the UI can offer click-to-reveal / ctrl+click-to-play on the entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileCreatedPayload {
    pub path: String,
    pub filename: String,
    pub timestamp: String,
    pub size_mb: f64,
    pub is_warning: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileMovedPayload {
    pub original: String,
    pub destination: String,
    pub tag: String,
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileRenamedPayload {
    pub original: String,
    pub new_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimerTickPayload {
    pub remaining_secs: u32,
    pub total_secs: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyPressedPayload {
    pub key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatcherStatusPayload {
    pub status: String,
    pub restart_count: Option<u32>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObsWsStatusPayload {
    pub status: String,
    pub attempt: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorPayload {
    pub message: String,
    pub context: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentClipPayload {
    pub name: String,
    pub path: String,
}

/// Session move stats for one G-key (sidebar badge + recent-clips flyout).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GKeyStatPayload {
    pub key: u8,
    pub count: u32,
    pub recent: Vec<RecentClipPayload>,
}

/// Snapshot for the diagnostics popover. Fetched on open — no polling.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsPayload {
    pub version: String,
    pub config_path: String,
    pub videos_folder: String,
    pub watcher_status: String,
    pub watcher_restart_count: u32,
    pub watch_paused: bool,
    pub obs_enabled: bool,
    pub obs_status: String,
}
