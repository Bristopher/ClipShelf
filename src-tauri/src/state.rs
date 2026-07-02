use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use tokio::sync::mpsc;

use crate::config::AppConfig;
use crate::logger::AppLogger;
use crate::timer::{CountUpCommand, TimerCommand};
use crate::watcher::WatcherCommand;

#[derive(Debug, Clone)]
pub struct CurrentFile {
    pub path: PathBuf,
    pub moved_path: Option<PathBuf>,
    pub renamed: bool,
}

/// One reversible move/rename: `from` is where the file was, `to` is where
/// it ended up. Undo renames `to` back to `from`.
#[derive(Debug, Clone)]
pub struct UndoEntry {
    pub from: PathBuf,
    pub to: PathBuf,
}

/// Cap on the undo history — enough for a whole session of mis-presses
/// without growing unbounded.
pub const UNDO_STACK_MAX: usize = 20;

pub struct AppStateInner {
    pub current_file: Option<CurrentFile>,
    pub bind_chosen: Option<String>,
    pub config: AppConfig,
    pub config_path: PathBuf,
    pub logger: AppLogger,
    pub watcher_restart_count: u32,
    pub timer_running: bool,
    /// Wall-clock time of the most recent `WatcherEvent::FileCreated` we
    /// processed. The save-clip health check compares this against the time
    /// the user pressed their capture-app hotkey to decide whether the
    /// watcher is alive.
    pub last_file_created_at: Option<SystemTime>,

    /// Raw path of that most recent created file (pre-move). Used to dedup
    /// the same clip being reported by both the OBS WebSocket and the folder
    /// watcher — unlike `current_file.path` this never changes on move.
    pub last_created_path: Option<PathBuf>,

    /// Calibration mode: when active, each `save_clip_bind` press records
    /// the next `FileCreated` arrival time and emits a sample to the UI so
    /// the user can pick a sensible `save_clip_health_check_timeout_secs`.
    pub calibration: CalibrationState,

    /// History of moves/renames for undo, newest last.
    pub undo_stack: Vec<UndoEntry>,

    /// While true the folder watcher is stopped and file events (including
    /// OBS WebSocket injection) are ignored — for reorganizing the clips
    /// folder without the app grabbing files. Runtime-only, resets on launch.
    pub watch_paused: bool,
}

#[derive(Debug, Clone, Default)]
pub struct CalibrationState {
    pub active: bool,
    pub target_samples: usize,
    pub pending_save_at: Option<SystemTime>,
    pub samples: Vec<CalibrationSample>,
}

#[derive(Debug, Clone)]
pub struct CalibrationSample {
    pub filename: String,
    pub delta_ms: u64,
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
            last_file_created_at: None,
            last_created_path: None,
            calibration: CalibrationState::default(),
            undo_stack: Vec::new(),
            watch_paused: false,
        }
    }

    /// Push an undo entry, evicting the oldest past the cap.
    pub fn push_undo(&mut self, entry: UndoEntry) {
        if self.undo_stack.len() >= UNDO_STACK_MAX {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(entry);
    }
}

pub type AppState = Arc<Mutex<AppStateInner>>;

/// Holds the tokio channel senders for background tasks.
pub struct ChannelState {
    pub timer_tx: mpsc::Sender<TimerCommand>,
    pub user_timer_tx: mpsc::Sender<TimerCommand>,
    pub watcher_tx: mpsc::Sender<WatcherCommand>,
    pub count_up_tx: mpsc::Sender<CountUpCommand>,
    pub obs_cmd_tx: mpsc::Sender<crate::obs_ws::ObsWsCommand>,
    pub hotkey_controller: crate::hotkeys::HotkeyController,
}
