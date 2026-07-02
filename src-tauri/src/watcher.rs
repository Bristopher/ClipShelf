use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use tauri;
use tokio::sync::mpsc;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum WatcherCommand {
    Start { path: PathBuf },
    Stop,
    Restart,
}

#[derive(Debug)]
pub enum WatcherEvent {
    FileCreated { path: PathBuf },
    StatusChanged { status: String, restart_count: u32, message: Option<String> },
    Error { message: String },
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Spawns the watcher actor and returns a command sender + event receiver.
pub fn spawn_watcher() -> (mpsc::Sender<WatcherCommand>, mpsc::Receiver<WatcherEvent>) {
    let (cmd_tx, cmd_rx) = mpsc::channel::<WatcherCommand>(32);
    let (evt_tx, evt_rx) = mpsc::channel::<WatcherEvent>(64);

    let cmd_tx_clone = cmd_tx.clone();
    tauri::async_runtime::spawn(watcher_actor(cmd_rx, cmd_tx_clone, evt_tx));

    (cmd_tx, evt_rx)
}

// ---------------------------------------------------------------------------
// Internal actor
// ---------------------------------------------------------------------------

async fn watcher_actor(
    mut cmd_rx: mpsc::Receiver<WatcherCommand>,
    cmd_tx: mpsc::Sender<WatcherCommand>,
    evt_tx: mpsc::Sender<WatcherEvent>,
) {
    // Active watcher lives here; dropping it stops the watch.
    let active_watcher: Arc<Mutex<Option<RecommendedWatcher>>> = Arc::new(Mutex::new(None));
    let mut restart_count: u32 = 0;
    let mut current_path: Option<PathBuf> = None;

    // Grab the current runtime handle so the notify callback (sync context)
    // can schedule async sends.
    let rt_handle = tokio::runtime::Handle::current();

    // --- Sleep/resume detector -------------------------------------------
    // Every second we tick; if the wall-clock delta exceeds 10 s we send
    // Restart to ourselves, which handles the case where the OS suspends and
    // the notify backend stops delivering events. Uses SystemTime, not
    // Instant — Instant is monotonic and may not advance across a Windows
    // suspend, which would make the gap invisible to this check.
    {
        let cmd_tx_sleep = cmd_tx.clone();
        tauri::async_runtime::spawn(async move {
            let mut last_tick = SystemTime::now();
            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;
                let now = SystemTime::now();
                let delta = now
                    .duration_since(last_tick)
                    .unwrap_or(Duration::ZERO);
                if delta > Duration::from_secs(10) {
                    // System likely woke from sleep; restart the watcher.
                    let _ = cmd_tx_sleep.send(WatcherCommand::Restart).await;
                }
                last_tick = now;
            }
        });
    }

    // --- Main command loop -----------------------------------------------
    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            WatcherCommand::Start { path } => {
                current_path = Some(path.clone());

                // Build a new RecommendedWatcher with the notify v8 API.
                let evt_tx_cb = evt_tx.clone();
                let rt_cb = rt_handle.clone();

                let result = RecommendedWatcher::new(
                    move |res: notify::Result<Event>| {
                        match res {
                            Ok(event) => {
                                if matches!(event.kind, EventKind::Create(_)) {
                                    for file_path in event.paths {
                                        if is_video_file(&file_path) {
                                            let evt_tx_inner = evt_tx_cb.clone();
                                            rt_cb.spawn(async move {
                                                // 200 ms debounce
                                                tokio::time::sleep(Duration::from_millis(200))
                                                    .await;
                                                let _ = evt_tx_inner
                                                    .send(WatcherEvent::FileCreated {
                                                        path: file_path,
                                                    })
                                                    .await;
                                            });
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                let evt_tx_inner = evt_tx_cb.clone();
                                let rt_inner = rt_cb.clone();
                                rt_inner.spawn(async move {
                                    let _ = evt_tx_inner
                                        .send(WatcherEvent::Error {
                                            message: e.to_string(),
                                        })
                                        .await;
                                });
                            }
                        }
                    },
                    Config::default(),
                );

                match result {
                    Ok(mut w) => {
                        match w.watch(&path, RecursiveMode::NonRecursive) {
                            Ok(()) => {
                                *active_watcher.lock().unwrap() = Some(w);
                                let _ = evt_tx
                                    .send(WatcherEvent::StatusChanged {
                                        status: "running".to_string(),
                                        restart_count,
                                        message: None,
                                    })
                                    .await;
                            }
                            Err(e) => {
                                let _ = evt_tx
                                    .send(WatcherEvent::Error {
                                        message: format!("Failed to watch path: {e}"),
                                    })
                                    .await;
                            }
                        }
                    }
                    Err(e) => {
                        let _ = evt_tx
                            .send(WatcherEvent::Error {
                                message: format!("Failed to create watcher: {e}"),
                            })
                            .await;
                    }
                }
            }

            WatcherCommand::Stop => {
                // Drop the watcher to cease watching.
                *active_watcher.lock().unwrap() = None;
                current_path = None;
                let _ = evt_tx
                    .send(WatcherEvent::StatusChanged {
                        status: "stopped".to_string(),
                        restart_count,
                        message: None,
                    })
                    .await;
            }

            WatcherCommand::Restart => {
                restart_count += 1;
                // Drop existing watcher.
                *active_watcher.lock().unwrap() = None;
                // Re-send Start if we had a path.
                if let Some(path) = current_path.clone() {
                    let _ = cmd_tx.send(WatcherCommand::Start { path }).await;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

pub fn is_video_file(path: &Path) -> bool {
    match path.extension() {
        Some(ext) => {
            let lower = ext.to_string_lossy().to_lowercase();
            matches!(lower.as_str(), "mp4" | "mov" | "avi" | "mkv")
        }
        None => false,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_video_file() {
        // Should return true
        assert!(is_video_file(Path::new("clip.mp4")));
        assert!(is_video_file(Path::new("clip.MP4")));
        assert!(is_video_file(Path::new("clip.mov")));
        assert!(is_video_file(Path::new("clip.avi")));
        assert!(is_video_file(Path::new("clip.mkv")));

        // Should return false
        assert!(!is_video_file(Path::new("notes.txt")));
        assert!(!is_video_file(Path::new("photo.jpg")));
        assert!(!is_video_file(Path::new("no_extension")));
    }
}
