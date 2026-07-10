use std::collections::HashMap;
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

/// How many recent clips to remember per G-key for the sidebar flyout.
pub const GKEY_RECENT_MAX: usize = 5;

/// Session-only per-G-key move stats: how many clips were sorted with this
/// key and where the last few ended up. Answers "did that sort land where I
/// think it did?" without reading the log. Resets on launch.
#[derive(Debug, Clone, Default)]
pub struct GKeyStat {
    pub count: u32,
    /// Newest first, capped at GKEY_RECENT_MAX.
    pub recent: Vec<PathBuf>,
}

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

    /// Last watcher status string emitted ("running"/"stopped"/"paused").
    /// Status events often fire before the webview finishes loading, so the
    /// frontend fetches this on mount instead of guessing.
    pub last_watcher_status: String,

    /// Last OBS WebSocket status string emitted (see obs_ws.rs statuses).
    /// Same rationale as `last_watcher_status`.
    pub last_obs_status: String,

    /// Session-only per-G-key move stats, keyed 1-3.
    pub gkey_stats: HashMap<u8, GKeyStat>,
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
            last_watcher_status: "stopped".to_string(),
            last_obs_status: "disabled".to_string(),
            gkey_stats: HashMap::new(),
        }
    }

    /// Push an undo entry, evicting the oldest past the cap.
    pub fn push_undo(&mut self, entry: UndoEntry) {
        if self.undo_stack.len() >= UNDO_STACK_MAX {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(entry);
    }

    /// Record a successful G-key move for the session stats.
    pub fn record_gkey_move(&mut self, key: u8, dest: PathBuf) {
        let stat = self.gkey_stats.entry(key).or_default();
        stat.count += 1;
        stat.recent.retain(|p| p != &dest);
        stat.recent.insert(0, dest);
        stat.recent.truncate(GKEY_RECENT_MAX);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;

    #[test]
    fn test_record_gkey_move_counts_and_caps_recent() {
        let mut s = AppStateInner::new(AppConfig::default(), PathBuf::new());

        for i in 0..7 {
            s.record_gkey_move(1, PathBuf::from(format!("C:/clips/clip{}.mp4", i)));
        }
        s.record_gkey_move(2, PathBuf::from("C:/clips/other.mp4"));

        let g1 = s.gkey_stats.get(&1).unwrap();
        assert_eq!(g1.count, 7);
        assert_eq!(g1.recent.len(), GKEY_RECENT_MAX);
        assert_eq!(g1.recent[0], PathBuf::from("C:/clips/clip6.mp4"));

        // Re-recording the same destination moves it to the front instead
        // of duplicating it (count still increments — it was a real move).
        s.record_gkey_move(1, PathBuf::from("C:/clips/clip3.mp4"));
        let g1 = s.gkey_stats.get(&1).unwrap();
        assert_eq!(g1.count, 8);
        assert_eq!(g1.recent[0], PathBuf::from("C:/clips/clip3.mp4"));
        assert_eq!(g1.recent.len(), GKEY_RECENT_MAX);

        assert_eq!(s.gkey_stats.get(&2).unwrap().count, 1);
    }
}
