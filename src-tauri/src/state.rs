use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::events::LogEntryPayload;

#[derive(Debug, Clone)]
pub struct CurrentFile {
    pub path: PathBuf,
    pub moved_path: Option<PathBuf>,
    pub renamed: bool,
}

#[derive(Debug, Clone)]
pub enum WatcherStatus {
    Running,
    Stopped,
    Error(String),
}

#[derive(Debug, Clone)]
pub enum ObsWsStatus {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting { attempt: u32 },
    FailedOver,
}

#[derive(Debug)]
pub struct AppStateInner {
    pub current_file: Option<CurrentFile>,
    pub bind_chosen: Option<String>,
    pub log_history: Vec<LogEntryPayload>,
    pub watcher_status: WatcherStatus,
    pub watcher_restart_count: u32,
    pub obs_ws_status: ObsWsStatus,
    pub timer_running: bool,
}

impl AppStateInner {
    pub fn new() -> Self {
        Self {
            current_file: None,
            bind_chosen: None,
            log_history: Vec::new(),
            watcher_status: WatcherStatus::Stopped,
            watcher_restart_count: 0,
            obs_ws_status: ObsWsStatus::Disconnected,
            timer_running: false,
        }
    }
}

pub type AppState = Arc<Mutex<AppStateInner>>;

pub fn create_app_state() -> AppState {
    Arc::new(Mutex::new(AppStateInner::new()))
}
