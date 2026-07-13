use std::collections::HashMap;
use std::path::{Path, PathBuf};
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
}

/// One file's reversible move: `from` is where the file was, `to` is where
/// it ended up. Undo renames `to` back to `from`.
#[derive(Debug, Clone)]
pub struct UndoMove {
    pub from: PathBuf,
    pub to: PathBuf,
}

/// One undoable ACTION — a single move/rename, or a whole multi-file batch
/// drop. One undo press reverses the entire action (moves restored in
/// reverse order).
#[derive(Debug, Clone)]
pub struct UndoEntry {
    pub moves: Vec<UndoMove>,
}

/// Cap on the undo history — enough for a whole session of mis-presses
/// without growing unbounded.
pub const UNDO_STACK_MAX: usize = 20;

/// How many recent clips to remember per G-key for the sidebar flyout.
pub const GKEY_RECENT_MAX: usize = 5;

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

    /// Session-only recent destinations per G-key (1-3), newest first,
    /// capped at GKEY_RECENT_MAX — feeds the sidebar flyout.
    pub gkey_recent: HashMap<u8, Vec<PathBuf>>,

    /// Persistent "today" move counts per G-key (gkey_stats.toml, rolls
    /// over at local midnight). Saved after each move, outside the lock.
    pub daily_stats: crate::stats::DailyStats,

    /// Game snapshot captured at the save-clip-bind press, consumed by the
    /// next FileCreated. Age-gated so a stale press can't mislabel a later
    /// clip that arrived by other means.
    pub pending_game: Option<crate::gamedetect::GameSnapshot>,

    /// Session map: clip's CURRENT path → detected game. Kept in sync on
    /// move/rename so rate/label events after sorting still carry the game.
    pub clip_games: HashMap<PathBuf, String>,

    /// Session map parallel to `clip_games`: clip's CURRENT path → detected
    /// exe stem. Maintained at the exact same re-key sites (create/move/
    /// rename/undo) so the overlay's "set game" can remember a per-exe
    /// detection override for a clip whose game was never auto-detected.
    pub clip_exes: HashMap<PathBuf, String>,
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
        let daily_stats = crate::stats::load(
            &crate::stats::stats_path(&config_path),
            config.day_rollover_hour,
        );
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
            gkey_recent: HashMap::new(),
            daily_stats,
            pending_game: None,
            clip_games: HashMap::new(),
            clip_exes: HashMap::new(),
        }
    }

    /// Re-key both session maps when a clip moves `from` → `to`. Keeps
    /// `clip_games` and `clip_exes` in lockstep at every move/rename/undo
    /// site — the single place that transfers a clip's identity so no site
    /// can update one map and forget the other. Returns the carried game so
    /// the caller can attach it to the history event it's already building.
    pub fn rekey_clip(&mut self, from: &Path, to: PathBuf) -> Option<String> {
        let game = self.clip_games.remove(from);
        if let Some(g) = &game {
            self.clip_games.insert(to.clone(), g.clone());
        }
        if let Some(exe) = self.clip_exes.remove(from) {
            self.clip_exes.insert(to, exe);
        }
        game
    }

    /// Consume the pending game snapshot if it's fresh enough. A snapshot
    /// older than `max_age` is dropped (a stale save-press must not mislabel
    /// a clip that arrived much later by another path).
    pub fn take_pending_game(
        &mut self,
        max_age: std::time::Duration,
    ) -> Option<crate::gamedetect::GameSnapshot> {
        let snap = self.pending_game.take()?;
        if snap.taken_at.elapsed().unwrap_or_default() <= max_age {
            Some(snap)
        } else {
            None
        }
    }

    /// Push an undo entry, evicting the oldest past the cap.
    pub fn push_undo(&mut self, entry: UndoEntry) {
        if self.undo_stack.len() >= UNDO_STACK_MAX {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(entry);
    }

    /// Record a successful G-key move: bumps the persistent daily count and
    /// the session recent list. Caller saves `daily_stats` to disk OUTSIDE
    /// the state lock.
    pub fn record_gkey_move(&mut self, key: u8, dest: PathBuf) {
        self.daily_stats.increment(key, self.config.day_rollover_hour);
        let recent = self.gkey_recent.entry(key).or_default();
        recent.retain(|p| p != &dest);
        recent.insert(0, dest);
        recent.truncate(GKEY_RECENT_MAX);
    }
}

pub type AppState = Arc<Mutex<AppStateInner>>;

/// Resolve where the clip created at `creation_path` lives RIGHT NOW, by
/// identity — never by "most recent clip" guesswork. Used by the deferred
/// property write so it follows a clip that was G-key-sorted while OBS still
/// held the file.
///
/// 1. If `clip_games` still keys the creation path, the clip hasn't moved.
/// 2. Otherwise walk the undo stack's move records — the only state that ties
///    an old path to its new one — following this clip's `from == cur` links
///    (newest first) until no link matches. At least one hop → that's the clip.
/// 3. No chain at all → `None`: the caller skips the write and warns.
///    Skipping is safe; writing another clip's metadata is not.
pub fn resolve_clip_current_path(
    clip_games: &HashMap<PathBuf, String>,
    undo_stack: &[UndoEntry],
    creation_path: &std::path::Path,
) -> Option<PathBuf> {
    if clip_games.contains_key(creation_path) {
        return Some(creation_path.to_path_buf());
    }
    let mut cur = creation_path.to_path_buf();
    let mut hops = 0usize;
    // Bound the walk so a pathological from/to cycle can't spin forever.
    let max_hops: usize = undo_stack.iter().map(|e| e.moves.len()).sum();
    while hops < max_hops {
        let next = undo_stack
            .iter()
            .rev()
            .flat_map(|e| e.moves.iter().rev())
            .find(|m| m.from == cur)
            .map(|m| m.to.clone());
        match next {
            Some(to) => {
                cur = to;
                hops += 1;
            }
            None => break,
        }
    }
    if hops > 0 {
        Some(cur)
    } else {
        None
    }
}

/// Holds the tokio channel senders for background tasks. (The auto-wipe
/// timer's sender isn't here — no command needs it; the lib.rs event
/// handlers hold their own clones.)
pub struct ChannelState {
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

        let g1 = s.gkey_recent.get(&1).unwrap();
        assert_eq!(s.daily_stats.count(1), 7);
        assert_eq!(g1.len(), GKEY_RECENT_MAX);
        assert_eq!(g1[0], PathBuf::from("C:/clips/clip6.mp4"));

        // Re-recording the same destination moves it to the front instead
        // of duplicating it (count still increments — it was a real move).
        s.record_gkey_move(1, PathBuf::from("C:/clips/clip3.mp4"));
        assert_eq!(s.daily_stats.count(1), 8);
        let g1 = s.gkey_recent.get(&1).unwrap();
        assert_eq!(g1[0], PathBuf::from("C:/clips/clip3.mp4"));
        assert_eq!(g1.len(), GKEY_RECENT_MAX);

        assert_eq!(s.daily_stats.count(2), 1);
    }

    // The burst-clipping guard: clip A's creation path was re-keyed away and a
    // SECOND clip (B) is the newest thing in clip_games — resolution must
    // follow A's undo-chain identity, or return None. It must NEVER return
    // clip B's path (that would write A's game into B's metadata).
    #[test]
    fn test_resolve_clip_current_path_identity_never_newest_clip() {
        let a_created = PathBuf::from("C:/clips/clipA.mp4");
        let a_sorted = PathBuf::from("C:/clips/wins/clipA - clutch.mp4");
        let b_created = PathBuf::from("C:/clips/clipB.mp4");

        // clip A was sorted (re-keyed to its new path); clip B just arrived.
        let mut clip_games: HashMap<PathBuf, String> = HashMap::new();
        clip_games.insert(a_sorted.clone(), "Game A".into());
        clip_games.insert(b_created.clone(), "Game B".into());

        // Undo stack records A's move by identity.
        let undo_stack = vec![UndoEntry {
            moves: vec![UndoMove {
                from: a_created.clone(),
                to: a_sorted.clone(),
            }],
        }];

        // Follows A's chain — not clip B.
        let resolved = resolve_clip_current_path(&clip_games, &undo_stack, &a_created);
        assert_eq!(resolved, Some(a_sorted.clone()));

        // Unmoved clip resolves to its own creation path.
        assert_eq!(
            resolve_clip_current_path(&clip_games, &undo_stack, &b_created),
            Some(b_created.clone())
        );

        // No clip_games key and no chain link → None (skip, never guess),
        // even though clip B exists and is "newest".
        let orphan = PathBuf::from("C:/clips/clipC.mp4");
        assert_eq!(
            resolve_clip_current_path(&clip_games, &undo_stack, &orphan),
            None
        );

        // Multi-hop: A gets moved again; the chain follows to the final path.
        let a_final = PathBuf::from("C:/clips/best/clipA - clutch.mp4");
        let undo_stack2 = vec![
            UndoEntry {
                moves: vec![UndoMove {
                    from: a_created.clone(),
                    to: a_sorted.clone(),
                }],
            },
            UndoEntry {
                moves: vec![UndoMove {
                    from: a_sorted.clone(),
                    to: a_final.clone(),
                }],
            },
        ];
        let mut cg2: HashMap<PathBuf, String> = HashMap::new();
        cg2.insert(a_final.clone(), "Game A".into());
        cg2.insert(b_created.clone(), "Game B".into());
        assert_eq!(
            resolve_clip_current_path(&cg2, &undo_stack2, &a_created),
            Some(a_final)
        );
    }

    #[test]
    fn test_rekey_clip_moves_game_and_exe_in_lockstep() {
        let mut s = AppStateInner::new(AppConfig::default(), PathBuf::new());
        let a = PathBuf::from("C:/clips/a.mp4");
        let b = PathBuf::from("C:/clips/sorted/a !!.mp4");
        s.clip_games.insert(a.clone(), "Halo".into());
        s.clip_exes.insert(a.clone(), "halo".into());

        // Move A → B: both maps re-key together, old keys gone.
        let carried = s.rekey_clip(&a, b.clone());
        assert_eq!(carried.as_deref(), Some("Halo"));
        assert!(!s.clip_games.contains_key(&a));
        assert!(!s.clip_exes.contains_key(&a));
        assert_eq!(s.clip_games.get(&b).map(String::as_str), Some("Halo"));
        assert_eq!(s.clip_exes.get(&b).map(String::as_str), Some("halo"));

        // Re-key a clip with an exe but no game: exe still transfers, game None.
        let c = PathBuf::from("C:/clips/c.mp4");
        let d = PathBuf::from("C:/clips/c - clutch.mp4");
        s.clip_exes.insert(c.clone(), "cs2".into());
        let carried = s.rekey_clip(&c, d.clone());
        assert_eq!(carried, None);
        assert!(!s.clip_exes.contains_key(&c));
        assert_eq!(s.clip_exes.get(&d).map(String::as_str), Some("cs2"));

        // Re-key an untracked path: no-op, no panic.
        let ghost = PathBuf::from("C:/clips/ghost.mp4");
        assert_eq!(s.rekey_clip(&ghost, PathBuf::from("C:/x.mp4")), None);
    }

    #[test]
    fn test_take_pending_game_respects_age() {
        use std::time::{Duration, SystemTime};
        let mut s = AppStateInner::new(AppConfig::default(), PathBuf::new());
        assert!(s.take_pending_game(Duration::from_secs(30)).is_none());

        s.pending_game = Some(crate::gamedetect::GameSnapshot {
            label: "Counter-Strike 2".into(),
            exe_stem: "cs2".into(),
            taken_at: SystemTime::now(),
        });
        let snap = s.take_pending_game(Duration::from_secs(30)).expect("fresh snapshot");
        assert_eq!(snap.label, "Counter-Strike 2");
        assert!(s.pending_game.is_none(), "take consumes");

        s.pending_game = Some(crate::gamedetect::GameSnapshot {
            label: "Old".into(),
            exe_stem: "old".into(),
            taken_at: SystemTime::now() - Duration::from_secs(120),
        });
        assert!(s.take_pending_game(Duration::from_secs(30)).is_none(), "stale is discarded");
    }
}
