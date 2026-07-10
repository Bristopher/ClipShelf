use tauri::{AppHandle, Emitter, Manager, State};

use crate::config::AppConfig;
use crate::events::*;
use crate::mover;
use crate::obs_ws::ObsWsCommand;
use crate::sound;
use crate::state::{AppState, ChannelState, CurrentFile};
use crate::theme::{Theme, ThemeExport, THEME_SCHEMA};
use crate::timer::{CountUpCommand, TimerCommand};
use crate::watcher::WatcherCommand;

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    Ok(s.config.clone())
}

#[tauri::command]
pub fn update_config(
    partial: serde_json::Value,
    state: State<'_, AppState>,
    channels: State<'_, ChannelState>,
    app: AppHandle,
) -> Result<AppConfig, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let prev_folder = s.config.videos_folder.clone();
    let prev_log_enabled = s.config.log_file_enabled;
    let prev_obs = (
        s.config.obs_websocket_enabled,
        s.config.obs_websocket_password.clone(),
    );
    // A merge failure means the update was discarded — error out instead of
    // saving/emitting as if it succeeded.
    s.config.merge_partial(partial)?;
    let path = s.config_path.clone();
    let config = s.config.clone();

    // Repoint the file logger — it captured the videos folder at startup and
    // would otherwise keep writing daily logs to the old location.
    if config.videos_folder != prev_folder || config.log_file_enabled != prev_log_enabled {
        s.logger
            .reconfigure(&config.videos_folder, config.log_file_enabled);
    }
    // Write to disk outside the critical section — a G-key press shouldn't
    // block on file IO behind the state lock.
    drop(s);
    config.save_to(&path).map_err(|e| e.to_string())?;

    // If the videos folder changed (including the first-run case where it
    // was empty at startup and is now set), (re)start the file watcher —
    // otherwise new clips are never seen until app restart. A cleared
    // folder stops the watcher instead of leaving it on the old path.
    if config.videos_folder != prev_folder {
        let watcher_tx = channels.watcher_tx.clone();
        let cmd = if config.videos_folder.is_empty() {
            WatcherCommand::Stop
        } else {
            WatcherCommand::Start {
                path: std::path::PathBuf::from(&config.videos_folder),
            }
        };
        tauri::async_runtime::spawn(async move {
            let _ = watcher_tx.send(cmd).await;
        });
    }

    // Hot-reload global hotkeys so a bind change in Settings takes effect
    // immediately — no app restart needed.
    channels
        .hotkey_controller
        .reload(crate::hotkeys::bindings_from_config(&config));

    // Keep the OS autostart registration in sync with the toggle.
    {
        use tauri_plugin_autostart::ManagerExt;
        let autolaunch = app.autolaunch();
        let result = if config.autostart_enabled {
            autolaunch.enable()
        } else {
            autolaunch.disable()
        };
        if let Err(e) = result {
            log::debug!("autostart sync: {}", e);
        }
    }

    // Hot-apply OBS WebSocket settings — the actor connects/disconnects/
    // reconnects with new credentials without an app restart.
    let new_obs = (
        config.obs_websocket_enabled,
        config.obs_websocket_password.clone(),
    );
    if new_obs != prev_obs {
        let obs_tx = channels.obs_cmd_tx.clone();
        tauri::async_runtime::spawn(async move {
            let _ = obs_tx
                .send(ObsWsCommand::Configure {
                    enabled: new_obs.0,
                    password: new_obs.1,
                })
                .await;
        });
    }

    let _ = app.emit("config-changed", &config);
    Ok(config)
}

#[tauri::command]
pub fn press_gkey(key: u8, state: State<'_, AppState>, app: AppHandle) -> Result<(), String> {
    do_press_gkey(&app, state.inner(), key);
    Ok(())
}

/// Core G-key move/rename logic. A free function (not just a command) so the
/// global hotkey handler can invoke it directly in Rust — a hotkey must work
/// even while the webview is still loading or wedged.
pub fn do_press_gkey(app: &AppHandle, state: &AppState, key: u8) {
    let Ok(mut s) = state.lock() else { return };
    let gkey_label = format!("G{}", key);
    s.bind_chosen = Some(gkey_label.clone());

    let entry = s.logger.log(
        LogLevel::Info,
        format!("{} Key", gkey_label),
        LogCategory::HotkeyPressed,
    );
    let _ = app.emit("log-entry", &entry);
    let _ = app.emit(
        "hotkey-pressed",
        HotkeyPressedPayload {
            key: gkey_label.clone(),
        },
    );

    // Check current file
    let current = match &s.current_file {
        Some(cf) => cf.clone(),
        None => {
            let msg = if s.config.videos_folder.is_empty() {
                "No videos folder configured — open Settings and pick your OBS/ShadowPlay clips folder.".to_string()
            } else {
                format!(
                    "No clip detected for {} — is OBS/ShadowPlay running? Check that clips save to: {}",
                    gkey_label, s.config.videos_folder
                )
            };
            let entry = s.logger.log(LogLevel::Error, msg, LogCategory::System);
            let _ = app.emit("log-entry", &entry);
            if s.config.error_sound_enabled {
                let resource_dir = app.path().resource_dir().unwrap_or_default();
                sound::play_error(&s.config.error_sound_custom, &resource_dir);
            }
            return;
        }
    };

    let file_path = current
        .moved_path
        .as_ref()
        .unwrap_or(&current.path)
        .clone();
    let config = s.config.clone();
    drop(s); // Release lock before file operations

    if let Some(mv) = move_file_with_key(app, state, &file_path, key, &gkey_label, &config) {
        if let Ok(mut s) = state.lock() {
            s.push_undo(crate::state::UndoEntry { moves: vec![mv] });
        }
    }
}

/// Shared move handling for a G-key action: the collision-safe move itself,
/// then state/current-file/stats bookkeeping, log + events, sound.
/// Used by key presses (current clip) and drag-drops (explicit path).
/// Blocking (retry sleeps) — callers run it off the async workers.
/// Returns the performed move on success so the CALLER records undo — a
/// single press pushes one entry, a batch drop groups all its moves into
/// one entry so one undo press reverses the whole drop.
fn move_file_with_key(
    app: &AppHandle,
    state: &AppState,
    file_path: &std::path::Path,
    key: u8,
    log_label: &str,
    config: &AppConfig,
) -> Option<crate::state::UndoMove> {
    match mover::move_or_rename_file(file_path, key, config) {
        Ok(result) => {
            let Ok(mut s) = state.lock() else { return None };
            s.current_file = Some(CurrentFile {
                path: result.new_path.clone(),
                moved_path: None,
                renamed: false,
            });
            s.record_gkey_move(key, result.new_path.clone());
            let mode = if config.disable_file_movesorting {
                "renamed"
            } else {
                "moved"
            };
            let msg = format!("File {} to {}", mode, result.tag_applied);
            let entry = s.logger.log_with_path(
                LogLevel::Success,
                msg,
                LogCategory::FileMoved,
                Some(result.new_path.to_string_lossy().to_string()),
            );
            let _ = app.emit("log-entry", &entry);
            let _ = app.emit(
                "file-moved",
                FileMovedPayload {
                    original: file_path.to_string_lossy().to_string(),
                    destination: result.new_path.to_string_lossy().to_string(),
                    tag: result.tag_applied.clone(),
                    mode: mode.to_string(),
                },
            );
            if config.log_file_enabled {
                let basename = file_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");
                s.logger
                    .write_to_file(&format!("{} | {}", log_label, basename));
            }
            // Persist the bumped daily count — disk write outside the lock.
            let daily = s.daily_stats.clone();
            let stats_path = crate::stats::stats_path(&s.config_path);
            drop(s);
            crate::stats::save(&stats_path, &daily);
            if config.move_sound_enabled {
                let resource_dir = app.path().resource_dir().unwrap_or_default();
                sound::play_move_beep(&resource_dir);
            }
            Some(crate::state::UndoMove {
                from: result.original_path,
                to: result.new_path,
            })
        }
        Err(e) => {
            let Ok(mut s) = state.lock() else { return None };
            let entry = s.logger.log(
                LogLevel::Error,
                format!("Move failed: {}", e),
                LogCategory::System,
            );
            let _ = app.emit("log-entry", &entry);
            let _ = app.emit(
                "error",
                ErrorPayload {
                    message: format!("Move failed: {}", e),
                    context: "move".to_string(),
                },
            );
            None
        }
    }
}

/// Outcome of a (possibly multi-file) drag-drop sort.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchDropResult {
    pub moved: u32,
    pub failed: u32,
}

/// Files dragged from Explorer and dropped onto a G-key button — sort each
/// through the normal move path. Manual fallback for clips the watcher
/// missed, or for bulk-sorting old clips. Per-file failures are logged and
/// emitted as error events by the move path; the summary comes back here.
#[tauri::command]
pub async fn drop_files_to_gkey(
    paths: Vec<String>,
    key: u8,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<BatchDropResult, String> {
    if !(1..=3).contains(&key) {
        return Err(format!("Invalid gkey: {}. Must be 1, 2, or 3.", key));
    }
    if paths.is_empty() {
        return Err("No files dropped".to_string());
    }
    let st = state.inner().clone();
    // Blocking pool — each move retries with sleeps, same as the hotkey path.
    tauri::async_runtime::spawn_blocking(move || {
        let config = {
            let Ok(s) = st.lock() else {
                return BatchDropResult { moved: 0, failed: paths.len() as u32 };
            };
            s.config.clone()
        };
        let label = format!("G{} (drop)", key);
        let mut moves: Vec<crate::state::UndoMove> = Vec::new();
        let mut failed = 0u32;
        for path in &paths {
            let file_path = std::path::PathBuf::from(path);
            if !crate::watcher::is_video_file(&file_path) || !file_path.exists() {
                failed += 1;
                continue;
            }
            match move_file_with_key(&app, &st, &file_path, key, &label, &config) {
                Some(mv) => moves.push(mv),
                None => failed += 1,
            }
        }
        let moved = moves.len() as u32;
        // The whole drop is ONE undoable action — a single undo press
        // restores every file it moved.
        if !moves.is_empty() {
            if let Ok(mut s) = st.lock() {
                s.push_undo(crate::state::UndoEntry { moves });
            }
        }
        BatchDropResult { moved, failed }
    })
    .await
    .map_err(|e| e.to_string())
}

/// A file dropped onto the log/rename area — make it the current clip (so
/// rename/G-keys operate on it) and return its filename for the dialog.
#[tauri::command]
pub fn select_dropped_file(
    path: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<String, String> {
    let file_path = std::path::PathBuf::from(&path);
    if !crate::watcher::is_video_file(&file_path) {
        return Err("Not a video file".to_string());
    }
    if !file_path.exists() {
        return Err("File no longer exists at this location".to_string());
    }
    let filename = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.current_file = Some(CurrentFile {
        path: file_path.clone(),
        moved_path: None,
        renamed: false,
    });
    s.bind_chosen = None;
    let entry = s.logger.log_with_path(
        LogLevel::Info,
        format!("Selected clip: {}", filename),
        LogCategory::FileCreated,
        Some(file_path.to_string_lossy().to_string()),
    );
    let _ = app.emit("log-entry", &entry);
    Ok(filename)
}

/// Move stats for all G-keys (sidebar badges + flyouts): persistent "today"
/// counts + session-only recent destinations.
#[tauri::command]
pub fn get_gkey_stats(state: State<'_, AppState>) -> Result<Vec<GKeyStatPayload>, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    Ok((1u8..=3)
        .map(|key| GKeyStatPayload {
            key,
            count: s.daily_stats.count(key),
            recent: s
                .gkey_recent
                .get(&key)
                .map(|list| {
                    list.iter()
                        .map(|p| RecentClipPayload {
                            name: p
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("unknown")
                                .to_string(),
                            path: p.to_string_lossy().to_string(),
                        })
                        .collect()
                })
                .unwrap_or_default(),
        })
        .collect())
}

/// One-shot OBS WebSocket connection test (first-run setup). Bounded at 5s
/// so a black-holed port can't leave the button spinning forever.
#[tauri::command]
pub async fn test_obs_connection(password: String) -> Result<(), String> {
    match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        crate::obs_ws::test_connection(&password),
    )
    .await
    {
        Ok(result) => result,
        Err(_) => Err("Timed out waiting for OBS".to_string()),
    }
}

/// Snapshot for the diagnostics popover.
#[tauri::command]
pub fn get_diagnostics(state: State<'_, AppState>) -> Result<DiagnosticsPayload, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    Ok(DiagnosticsPayload {
        version: env!("CARGO_PKG_VERSION").to_string(),
        config_path: s.config_path.to_string_lossy().to_string(),
        videos_folder: s.config.videos_folder.clone(),
        watcher_status: s.last_watcher_status.clone(),
        watcher_restart_count: s.watcher_restart_count,
        watch_paused: s.watch_paused,
        obs_enabled: s.config.obs_websocket_enabled,
        obs_status: s.last_obs_status.clone(),
    })
}

#[tauri::command]
pub async fn rename_file(
    text: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    // The rename retries with sleeps (up to ~1.7s) — keep that off the async
    // runtime workers, same as the hotkey paths.
    let st = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || do_rename_file(&app, &st, &text))
        .await
        .map_err(|e| e.to_string())
}

fn do_rename_file(app: &AppHandle, state: &AppState, text: &str) {
    let (file_path, log_enabled) = {
        let Ok(s) = state.lock() else { return };
        let current = match &s.current_file {
            Some(cf) => cf.clone(),
            None => {
                // Need mutable borrow for logging - drop and re-acquire
                drop(s);
                let Ok(mut s) = state.lock() else { return };
                let entry = s
                    .logger
                    .log(LogLevel::Error, "No current_file".into(), LogCategory::System);
                let _ = app.emit("log-entry", &entry);
                return;
            }
        };
        let file_path = current
            .moved_path
            .as_ref()
            .unwrap_or(&current.path)
            .clone();
        (file_path, s.config.log_file_enabled)
    };

    // {date}/{time} tokens expand into the filename; the MRU keeps the raw
    // text so templates stay reusable.
    let expanded = mover::expand_rename_tokens(text);
    match mover::rename_file_with_text(&file_path, &expanded) {
        Ok(result) => {
            let Ok(mut s) = state.lock() else { return };
            s.current_file = Some(CurrentFile {
                path: result.new_path.clone(),
                moved_path: Some(result.new_path.clone()),
                renamed: true,
            });
            s.push_undo(crate::state::UndoEntry {
                moves: vec![crate::state::UndoMove {
                    from: result.original_path.clone(),
                    to: result.new_path.clone(),
                }],
            });
            let new_name = result
                .new_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();
            let entry = s.logger.log_with_path(
                LogLevel::Success,
                format!("File renamed to: {}", new_name),
                LogCategory::FileRenamed,
                Some(result.new_path.to_string_lossy().to_string()),
            );
            let _ = app.emit("log-entry", &entry);
            let _ = app.emit(
                "file-renamed",
                FileRenamedPayload {
                    original: file_path.to_string_lossy().to_string(),
                    new_name: new_name.clone(),
                },
            );
            if log_enabled {
                let old_name = file_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");
                s.logger
                    .write_to_file(&format!("Renamed: {} ---------> {}", old_name, new_name));
            }
            // Remember the text in the rename MRU so the dialog can offer it
            // as a one-click chip next time. Persisted in config; disk write
            // happens outside the state lock.
            s.config.push_rename_mru(text);
            let (cfg, cfg_path) = (s.config.clone(), s.config_path.clone());
            drop(s);
            if let Err(e) = cfg.save_to(&cfg_path) {
                log::warn!("Failed to persist rename MRU: {}", e);
            }
            let _ = app.emit("config-changed", &cfg);
        }
        Err(e) => {
            let Ok(mut s) = state.lock() else { return };
            let entry = s.logger.log(
                LogLevel::Error,
                format!("Rename failed: {}", e),
                LogCategory::System,
            );
            let _ = app.emit("log-entry", &entry);
            let _ = app.emit(
                "error",
                ErrorPayload {
                    message: format!("Rename failed: {}", e),
                    context: "rename".to_string(),
                },
            );
        }
    }
}

#[tauri::command]
pub async fn undo_last_action(state: State<'_, AppState>, app: AppHandle) -> Result<(), String> {
    // restore_file retries with sleeps — run on the blocking pool, same as
    // the undo hotkey path in lib.rs.
    let st = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || do_undo(&app, &st))
        .await
        .map_err(|e| e.to_string())
}

/// Reverse the most recent action (a single move/rename, or every file of a
/// batch drop — restored in reverse order). Free function so the global
/// undo hotkey can call it directly in Rust, same as do_press_gkey.
pub fn do_undo(app: &AppHandle, state: &AppState) {
    let popped = {
        let Ok(mut s) = state.lock() else { return };
        s.undo_stack.pop()
    };
    let Some(entry) = popped else {
        let Ok(mut s) = state.lock() else { return };
        let log = s.logger.log(
            LogLevel::Info,
            "Nothing to undo".to_string(),
            LogCategory::System,
        );
        let _ = app.emit("log-entry", &log);
        return;
    };

    let total = entry.moves.len();
    let mut restored_count = 0usize;
    for mv in entry.moves.iter().rev() {
        match mover::restore_file(&mv.to, &mv.from) {
            Ok(restored) => {
                restored_count += 1;
                let Ok(mut s) = state.lock() else { return };
                s.current_file = Some(CurrentFile {
                    path: restored.clone(),
                    moved_path: None,
                    renamed: false,
                });
                let undone_name = mv
                    .to
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("?")
                    .to_string();
                let restored_name = restored
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("?")
                    .to_string();
                let log = s.logger.log_with_path(
                    LogLevel::Success,
                    format!("Undo: {} → {}", undone_name, restored_name),
                    LogCategory::FileMoved,
                    Some(restored.to_string_lossy().to_string()),
                );
                let _ = app.emit("log-entry", &log);
                if s.config.log_file_enabled {
                    s.logger.write_to_file(&format!(
                        "Undo: {} ---------> {}",
                        undone_name, restored_name
                    ));
                }
            }
            Err(e) => {
                let Ok(mut s) = state.lock() else { return };
                let log = s.logger.log(
                    LogLevel::Error,
                    format!("Undo failed: {}", e),
                    LogCategory::System,
                );
                let _ = app.emit("log-entry", &log);
                let _ = app.emit(
                    "error",
                    ErrorPayload {
                        message: format!("Undo failed: {}", e),
                        context: "undo".to_string(),
                    },
                );
            }
        }
    }

    // Batch summary so the user sees one line for the whole action.
    if total > 1 {
        let Ok(mut s) = state.lock() else { return };
        let log = s.logger.log(
            LogLevel::Info,
            format!("Undo batch: restored {}/{} files", restored_count, total),
            LogCategory::System,
        );
        let _ = app.emit("log-entry", &log);
    }
}

/// Open Windows Explorer with the given file selected/highlighted.
#[tauri::command]
pub fn reveal_in_explorer(path: String) -> Result<(), String> {
    let p = std::path::Path::new(&path);
    if !p.exists() {
        return Err("File no longer exists at this location".to_string());
    }
    opener::reveal(p).map_err(|e| e.to_string())
}

/// Pause/resume clip watching. Paused = watcher stopped AND file events
/// from any source (OBS WebSocket, health-check rescan) ignored, so the
/// user can reorganize the clips folder without the app grabbing files.
#[tauri::command]
pub fn set_watch_paused(
    paused: bool,
    state: State<'_, AppState>,
    channels: State<'_, ChannelState>,
    app: AppHandle,
) -> Result<(), String> {
    let videos_folder = {
        let mut s = state.lock().map_err(|e| e.to_string())?;
        s.watch_paused = paused;
        if paused {
            s.last_watcher_status = "paused".to_string();
        }
        let msg = if paused {
            "Watching paused — new clips are ignored"
        } else {
            "Watching resumed"
        };
        let entry = s.logger.log(
            if paused { LogLevel::Warning } else { LogLevel::Info },
            msg.to_string(),
            LogCategory::WatcherStatus,
        );
        let _ = app.emit("log-entry", &entry);
        s.config.videos_folder.clone()
    };

    // Keep the tray checkbox in sync when toggled from the UI.
    if let Some(tray) = app.try_state::<crate::tray::TrayItems>() {
        let _ = tray.pause_item.set_checked(paused);
    }

    let watcher_tx = channels.watcher_tx.clone();
    let cmd = if paused {
        WatcherCommand::Stop
    } else if !videos_folder.is_empty() {
        WatcherCommand::Start {
            path: std::path::PathBuf::from(videos_folder),
        }
    } else {
        // Resumed but no folder configured — nothing to start, but the UI
        // still needs to leave the "paused" state.
        if let Ok(mut s) = state.lock() {
            s.last_watcher_status = "stopped".to_string();
        }
        let _ = app.emit(
            "watcher-status",
            WatcherStatusPayload {
                status: "stopped".to_string(),
                restart_count: None,
                message: None,
            },
        );
        return Ok(());
    };
    tauri::async_runtime::spawn(async move {
        let _ = watcher_tx.send(cmd).await;
    });
    Ok(())
}

/// Current watcher status — fetched by the UI on mount because the status
/// events usually fire before the webview has loaded its listeners.
#[tauri::command]
pub fn get_watcher_status(state: State<'_, AppState>) -> Result<WatcherStatusPayload, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    Ok(WatcherStatusPayload {
        status: s.last_watcher_status.clone(),
        restart_count: Some(s.watcher_restart_count),
        message: None,
    })
}

/// Current OBS WebSocket status — same on-mount rationale as above.
#[tauri::command]
pub fn get_obs_status(state: State<'_, AppState>) -> Result<ObsWsStatusPayload, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    Ok(ObsWsStatusPayload {
        status: s.last_obs_status.clone(),
        attempt: None,
    })
}

/// Number of connected monitors — used by the Settings default-position picker.
#[tauri::command]
pub fn get_monitor_count(app: AppHandle) -> usize {
    app.get_webview_window("main")
        .and_then(|w| w.available_monitors().ok())
        .map(|m| m.len())
        .unwrap_or(1)
}

#[tauri::command]
pub fn wipe_log(state: State<'_, AppState>) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.logger.wipe_display();
    Ok(())
}

#[tauri::command]
pub fn restore_log(state: State<'_, AppState>) -> Result<Vec<LogEntryPayload>, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    Ok(s.logger.restore_display())
}

/// Starts the user-triggered countdown timer. Independent from the
/// auto-wipe timer — this one exists so users can time their OBS "Save
/// Replay Buffer" press without interfering with file-arrival wiping.
#[tauri::command]
pub fn start_user_timer(
    duration_secs: Option<u32>,
    state: State<'_, AppState>,
    channels: State<'_, ChannelState>,
) -> Result<(), String> {
    let duration = match duration_secs {
        Some(d) => d,
        None => {
            let s = state.lock().map_err(|e| e.to_string())?;
            s.config.timer_duration_secs() as u32
        }
    };
    channels
        .user_timer_tx
        .try_send(TimerCommand::Start { duration_secs: duration })
        .map_err(|e| format!("Failed to start user timer: {}", e))
}

/// Resets the user-triggered countdown back to its full duration and
/// stops it. Emits a final `user-timer-tick` so the UI updates to the
/// reset value. Leaves the auto-wipe timer untouched.
#[tauri::command]
pub fn reset_user_timer(
    duration_secs: Option<u32>,
    state: State<'_, AppState>,
    channels: State<'_, ChannelState>,
) -> Result<(), String> {
    let duration = match duration_secs {
        Some(d) => d,
        None => {
            let s = state.lock().map_err(|e| e.to_string())?;
            s.config.timer_duration_secs() as u32
        }
    };
    channels
        .user_timer_tx
        .try_send(TimerCommand::Reset { duration_secs: duration })
        .map_err(|e| format!("Failed to reset user timer: {}", e))
}

#[tauri::command]
pub fn restart_watcher(channels: State<'_, ChannelState>) -> Result<(), String> {
    channels
        .watcher_tx
        .try_send(WatcherCommand::Restart)
        .map_err(|e| format!("Failed to send restart command: {}", e))
}

/// Begin a save-clip latency calibration session. The user will press their
/// `save_clip_bind` `target_samples` times; each press → file-arrival delta
/// is recorded and emitted as a `calibration-event` with kind `"sample"`.
/// After the final sample, kind flips to `"complete"` with average / worst
/// / best in milliseconds.
#[tauri::command]
pub fn start_calibration(
    target_samples: usize,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    if s.config.save_clip_bind.is_empty() {
        return Err("Set a save-clip hotkey first".to_string());
    }
    s.calibration.active = true;
    s.calibration.target_samples = target_samples.clamp(1, 50);
    s.calibration.pending_save_at = None;
    s.calibration.samples.clear();
    Ok(())
}

#[tauri::command]
pub fn full_quit(app: AppHandle) {
    // Ctrl+click on the X — bypass the hide-to-tray behavior and exit the
    // process. Used by the title-bar close button when the user wants to
    // stop the app entirely (e.g. before running an installer upgrade).
    app.exit(0);
}

#[tauri::command]
pub fn toggle_count_up(channels: State<'_, ChannelState>) -> Result<(), String> {
    channels
        .count_up_tx
        .try_send(CountUpCommand::Toggle)
        .map_err(|e| format!("Failed to send count-up toggle: {}", e))
}

#[tauri::command]
pub fn cancel_calibration(state: State<'_, AppState>) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.calibration.active = false;
    s.calibration.pending_save_at = None;
    s.calibration.samples.clear();
    Ok(())
}

#[tauri::command]
pub fn open_folder(path: String) -> Result<(), String> {
    opener::open(&path).map_err(|e| e.to_string())
}

/// Returns the Windows apps theme as "light" or "dark", or `None` if the
/// platform isn't Windows or the registry read fails/times out.
///
/// Uses `spawn_blocking` + 500ms timeout so a hung read can't lock the UI.
#[tauri::command]
pub async fn get_system_theme_mode() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        let handle = tauri::async_runtime::spawn_blocking(read_windows_apps_theme);
        match tokio::time::timeout(std::time::Duration::from_millis(500), handle).await {
            Ok(Ok(mode)) => mode,
            _ => None,
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

#[cfg(target_os = "windows")]
fn read_windows_apps_theme() -> Option<String> {
    use std::ptr::null_mut;
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ,
    };

    let subkey: Vec<u16> =
        "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize\0"
            .encode_utf16()
            .collect();
    let value_name: Vec<u16> = "AppsUseLightTheme\0".encode_utf16().collect();

    unsafe {
        let mut hkey: HKEY = null_mut();
        if RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_READ, &mut hkey)
            != ERROR_SUCCESS
        {
            return None;
        }
        let mut value: u32 = 0;
        let mut size: u32 = std::mem::size_of::<u32>() as u32;
        let mut ty: u32 = 0;
        let status = RegQueryValueExW(
            hkey,
            value_name.as_ptr(),
            std::ptr::null_mut(),
            &mut ty,
            &mut value as *mut u32 as *mut u8,
            &mut size,
        );
        RegCloseKey(hkey);
        if status != ERROR_SUCCESS {
            return None;
        }
        if value == 1 {
            Some("light".into())
        } else {
            Some("dark".into())
        }
    }
}

#[tauri::command]
pub fn import_theme(path: String) -> Result<Theme, String> {
    let contents = std::fs::read_to_string(&path)
        .map_err(|e| format!("io: {}", e))?;
    let envelope: ThemeExport = serde_json::from_str(&contents)
        .map_err(|e| format!("invalid JSON: {}", e))?;
    if envelope.schema != THEME_SCHEMA {
        return Err(format!(
            "invalid schema: expected {}, got {}",
            THEME_SCHEMA, envelope.schema
        ));
    }
    let name = envelope.name.trim();
    if name.is_empty() {
        return Err("invalid name: empty".into());
    }
    let id = slugify(name);
    Ok(Theme {
        id,
        name: name.to_string(),
        builtin: false,
        tokens: envelope.tokens,
    })
}

#[tauri::command]
pub fn export_theme(
    path: String,
    theme_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    let builtins = crate::theme::builtin_themes();
    let theme = builtins
        .iter()
        .chain(s.config.themes.iter())
        .find(|t| t.id == theme_id)
        .ok_or_else(|| format!("theme not found: {}", theme_id))?;
    let envelope = ThemeExport {
        schema: THEME_SCHEMA.into(),
        name: theme.name.clone(),
        tokens: theme.tokens.clone(),
    };
    let json = serde_json::to_string_pretty(&envelope).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("io: {}", e))?;
    Ok(())
}

fn slugify(name: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    let trimmed = out.trim_end_matches('-').to_string();
    if trimmed.is_empty() {
        "custom".to_string()
    } else {
        trimmed
    }
}

#[tauri::command]
pub fn open_settings_window(app: AppHandle) -> Result<(), String> {
    // The settings window is pre-created at startup (see lib.rs) so runtime
    // URL resolution can't go wrong. This command just reveals it.
    if let Some(existing) = app.get_webview_window("settings") {
        let _ = existing.unminimize();
        let _ = existing.show();
        let _ = existing.set_focus();
        return Ok(());
    }
    Err("settings window was not pre-created at startup".into())
}

#[tauri::command]
pub fn open_first_run_window(app: AppHandle) -> Result<(), String> {
    // First-run window is also pre-created; reveal + center on call.
    if let Some(existing) = app.get_webview_window("first-run") {
        let _ = existing.unminimize();
        let _ = existing.center();
        let _ = existing.show();
        let _ = existing.set_focus();
        // Flash the taskbar entry so users realize setup is pending.
        let _ = existing.request_user_attention(Some(tauri::UserAttentionType::Critical));
        return Ok(());
    }
    Err("first-run window was not pre-created at startup".into())
}

#[tauri::command]
pub fn reset_window(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    // Back to the configured default open position (Settings > Window) at
    // the default size, and forget the remembered layout.
    let (config, config_path) = {
        let s = state.lock().map_err(|e| e.to_string())?;
        (s.config.clone(), s.config_path.clone())
    };
    let window = app
        .get_webview_window("main")
        .ok_or("main window not found")?;
    crate::window_layout::clear(&crate::window_layout::layout_path(&config_path));
    crate::window_layout::apply_default_position(&window, &config, true);
    Ok(())
}

#[tauri::command]
pub fn set_window_opacity(opacity: f64, window: tauri::Window) -> Result<(), String> {
    let clamped = opacity.clamp(0.2, 1.0);
    let alpha = (clamped * 255.0) as u8;

    let hwnd = window.hwnd().map_err(|e| e.to_string())?.0;

    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::*;
        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
        SetWindowLongW(hwnd, GWL_EXSTYLE, ex_style | WS_EX_LAYERED as i32);
        SetLayeredWindowAttributes(hwnd, 0, alpha, LWA_ALPHA);
    }

    Ok(())
}
