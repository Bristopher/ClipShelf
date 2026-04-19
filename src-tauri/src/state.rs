use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

use crate::config::AppConfig;
use crate::logger::AppLogger;
use crate::timer::TimerCommand;
use crate::watcher::WatcherCommand;

#[derive(Debug, Clone)]
pub struct CurrentFile {
    pub path: PathBuf,
    pub moved_path: Option<PathBuf>,
    pub renamed: bool,
}

pub struct AppStateInner {
    pub current_file: Option<CurrentFile>,
    pub bind_chosen: Option<String>,
    pub config: AppConfig,
    pub config_path: PathBuf,
    pub logger: AppLogger,
    pub watcher_restart_count: u32,
    pub timer_running: bool,
}

impl AppStateInner {
    pub fn new(config: AppConfig, config_path: PathBuf) -> Self {
        let logger = AppLogger::new(&config.videos_folder, config.log_file_enabled);
        Self {
            current_file: None,
            bind_chosen: None,
            config,
            config_path,
            logger,
            watcher_restart_count: 0,
            timer_running: false,
        }
    }
}

pub type AppState = Arc<Mutex<AppStateInner>>;

/// Holds the tokio channel senders for background tasks.
pub struct ChannelState {
    pub timer_tx: mpsc::Sender<TimerCommand>,
    pub user_timer_tx: mpsc::Sender<TimerCommand>,
    pub watcher_tx: mpsc::Sender<WatcherCommand>,
}
