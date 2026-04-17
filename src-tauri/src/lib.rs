mod commands;
mod config;
mod events;
mod hotkeys;
mod logger;
mod mover;
mod obs_ws;
mod sound;
mod state;
mod timer;
mod tray;
mod watcher;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use regex::Regex;
use tauri::{Emitter, Manager};
use tokio::sync::mpsc;

use config::AppConfig;
use events::*;
use hotkeys::HotkeyAction;
use obs_ws::{ObsWsCommand, ObsWsEvent};
use state::{AppState, AppStateInner, ChannelState, CurrentFile};
use timer::TimerCommand;
use watcher::{WatcherCommand, WatcherEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_handle = app.handle().clone();

            // Load config
            let config = AppConfig::load().unwrap_or_else(|e| {
                log::warn!("Failed to load config, using defaults: {}", e);
                AppConfig::default()
            });

            // Create AppState
            let app_state: AppState = Arc::new(Mutex::new(AppStateInner::new(config.clone())));
            app.manage(app_state.clone());

            // Set up system tray
            if let Err(e) = tray::setup_tray(&app_handle) {
                log::error!("Failed to set up system tray: {}", e);
            }

            // Spawn timer
            let timer_tx = timer::spawn_timer(app_handle.clone());

            // Spawn file watcher
            let (watcher_tx, mut watcher_rx) = watcher::spawn_watcher();

            // Create ChannelState
            let channel_state = ChannelState {
                timer_tx: timer_tx.clone(),
                watcher_tx: watcher_tx.clone(),
            };
            app.manage(channel_state);

            // Start watching if videos_folder is set
            if !config.videos_folder.is_empty() {
                let path = PathBuf::from(&config.videos_folder);
                let watcher_tx_start = watcher_tx.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = watcher_tx_start
                        .send(WatcherCommand::Start { path })
                        .await;
                });
            }

            // Spawn watcher event handler
            {
                let app_handle = app_handle.clone();
                let state = app_state.clone();
                let timer_tx = timer_tx.clone();

                tauri::async_runtime::spawn(async move {
                    while let Some(event) = watcher_rx.recv().await {
                        match event {
                            WatcherEvent::FileCreated { path } => {
                                let config = {
                                    let s = state.lock().unwrap();
                                    s.config.clone()
                                };
                                handle_file_created(
                                    &app_handle,
                                    &state,
                                    &timer_tx,
                                    &config,
                                    path,
                                )
                                .await;
                            }
                            WatcherEvent::StatusChanged {
                                status,
                                restart_count,
                                message,
                            } => {
                                {
                                    let mut s = state.lock().unwrap();
                                    s.watcher_restart_count = restart_count;
                                    let msg = match &message {
                                        Some(m) => format!("Watcher {}: {}", status, m),
                                        None => format!("Watcher {}", status),
                                    };
                                    let entry = s.logger.log(
                                        LogLevel::Info,
                                        msg,
                                        LogCategory::WatcherStatus,
                                    );
                                    let _ = app_handle.emit("log-entry", &entry);
                                }
                                let _ = app_handle.emit(
                                    "watcher-status",
                                    WatcherStatusPayload {
                                        status,
                                        restart_count: Some(restart_count),
                                        message,
                                    },
                                );
                            }
                            WatcherEvent::Error { message } => {
                                let mut s = state.lock().unwrap();
                                let entry = s.logger.log(
                                    LogLevel::Error,
                                    format!("Watcher error: {}", message),
                                    LogCategory::WatcherStatus,
                                );
                                let _ = app_handle.emit("log-entry", &entry);
                                let _ = app_handle.emit(
                                    "error",
                                    ErrorPayload {
                                        message,
                                        context: "watcher".to_string(),
                                    },
                                );
                            }
                        }
                    }
                });
            }

            // Spawn hotkey listener
            {
                let app_handle = app_handle.clone();
                let watcher_tx = watcher_tx.clone();

                let bindings = vec![
                    (HotkeyAction::MoveG1, config.g1_bind.clone()),
                    (HotkeyAction::MoveG2, config.g2_bind.clone()),
                    (HotkeyAction::MoveG3, config.g3_bind.clone()),
                    (HotkeyAction::Rename, config.rename_bind.clone()),
                    (
                        HotkeyAction::RestartWatcher,
                        config.restart_watcher_bind.clone(),
                    ),
                ];

                match hotkeys::spawn_hotkey_listener(bindings) {
                    Ok(mut hotkey_rx) => {
                        tauri::async_runtime::spawn(async move {
                            while let Some(action) = hotkey_rx.recv().await {
                                match action {
                                    HotkeyAction::MoveG1 => {
                                        let _ = app_handle
                                            .emit("hotkey-triggered", serde_json::json!({"key": 1}));
                                    }
                                    HotkeyAction::MoveG2 => {
                                        let _ = app_handle
                                            .emit("hotkey-triggered", serde_json::json!({"key": 2}));
                                    }
                                    HotkeyAction::MoveG3 => {
                                        let _ = app_handle
                                            .emit("hotkey-triggered", serde_json::json!({"key": 3}));
                                    }
                                    HotkeyAction::Rename => {
                                        let _ = app_handle
                                            .emit("hotkey-triggered", serde_json::json!({"key": 4}));
                                    }
                                    HotkeyAction::RestartWatcher => {
                                        let _ = watcher_tx
                                            .send(WatcherCommand::Restart)
                                            .await;
                                    }
                                }
                            }
                        });
                    }
                    Err(e) => {
                        log::error!("Failed to spawn hotkey listener: {}", e);
                    }
                }
            }

            // Spawn OBS WebSocket if enabled
            if config.obs_websocket_enabled && !config.obs_websocket_password.is_empty() {
                let app_handle = app_handle.clone();
                let state = app_state.clone();
                let (obs_cmd_tx, mut obs_event_rx) =
                    obs_ws::spawn_obs_ws(config.obs_websocket_password.clone(), 5);

                // Send initial connect command
                let obs_cmd_tx_clone = obs_cmd_tx.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = obs_cmd_tx_clone.send(ObsWsCommand::Connect).await;
                });

                // Handle OBS events
                tauri::async_runtime::spawn(async move {
                    while let Some(event) = obs_event_rx.recv().await {
                        match event {
                            ObsWsEvent::Connected => {
                                let mut s = state.lock().unwrap();
                                let entry = s.logger.log(
                                    LogLevel::Success,
                                    "OBS WebSocket connected".to_string(),
                                    LogCategory::ObsWebSocket,
                                );
                                let _ = app_handle.emit("log-entry", &entry);
                                let _ = app_handle.emit(
                                    "obs-ws-status",
                                    ObsWsStatusPayload {
                                        status: "connected".to_string(),
                                        attempt: None,
                                    },
                                );
                            }
                            ObsWsEvent::Disconnected { code, reason } => {
                                let mut s = state.lock().unwrap();
                                let msg = format!(
                                    "OBS WebSocket disconnected: {} (code: {:?})",
                                    reason, code
                                );
                                let entry = s.logger.log(
                                    LogLevel::Warning,
                                    msg,
                                    LogCategory::ObsWebSocket,
                                );
                                let _ = app_handle.emit("log-entry", &entry);
                                let _ = app_handle.emit(
                                    "obs-ws-status",
                                    ObsWsStatusPayload {
                                        status: "disconnected".to_string(),
                                        attempt: None,
                                    },
                                );
                            }
                            ObsWsEvent::ReplayBufferSaved { path } => {
                                // OBS reports file saved - the watcher should also
                                // pick this up as a FileCreated event.
                                let mut s = state.lock().unwrap();
                                let entry = s.logger.log(
                                    LogLevel::Info,
                                    format!("OBS replay saved: {}", path),
                                    LogCategory::ObsWebSocket,
                                );
                                let _ = app_handle.emit("log-entry", &entry);
                            }
                            ObsWsEvent::AuthError { message } => {
                                let mut s = state.lock().unwrap();
                                let entry = s.logger.log(
                                    LogLevel::Error,
                                    format!("OBS auth error: {}", message),
                                    LogCategory::ObsWebSocket,
                                );
                                let _ = app_handle.emit("log-entry", &entry);
                            }
                            ObsWsEvent::Error { message } => {
                                let mut s = state.lock().unwrap();
                                let entry = s.logger.log(
                                    LogLevel::Error,
                                    format!("OBS error: {}", message),
                                    LogCategory::ObsWebSocket,
                                );
                                let _ = app_handle.emit("log-entry", &entry);
                            }
                            ObsWsEvent::StatusChanged { status, attempt } => {
                                let _ = app_handle.emit(
                                    "obs-ws-status",
                                    ObsWsStatusPayload { status, attempt },
                                );
                            }
                        }
                    }
                });
            }

            // Log startup message
            {
                let mut s = app_state.lock().unwrap();
                let entry = s.logger.log(
                    LogLevel::Info,
                    "Gkey Mover v2 started".to_string(),
                    LogCategory::System,
                );
                let _ = app_handle.emit("log-entry", &entry);
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::update_config,
            commands::press_gkey,
            commands::rename_file,
            commands::wipe_log,
            commands::restore_log,
            commands::restart_watcher,
            commands::open_folder,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Hide window instead of closing (minimize to tray)
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Handle a newly created file from the watcher.
async fn handle_file_created(
    app: &tauri::AppHandle,
    state: &AppState,
    timer_tx: &mpsc::Sender<TimerCommand>,
    config: &AppConfig,
    path: PathBuf,
) {
    let size_mb = mover::file_size_mb(&path);
    let is_warning = size_mb < 6.5;

    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    // Parse timestamp from filename
    let timestamp = parse_time_from_filename(&filename);

    // Update state
    {
        let mut s = state.lock().unwrap();
        s.current_file = Some(CurrentFile {
            path: path.clone(),
            moved_path: None,
            renamed: false,
        });
        s.bind_chosen = None;

        // Log file creation
        let level = if is_warning {
            LogLevel::Warning
        } else {
            LogLevel::Info
        };
        let msg = if is_warning {
            format!(
                "New file: {} ({:.1}MB - possible black screen)",
                filename, size_mb
            )
        } else {
            format!("New file: {} ({:.1}MB)", filename, size_mb)
        };
        let entry = s.logger.log(level, msg, LogCategory::FileCreated);
        let _ = app.emit("log-entry", &entry);

        // Log to file
        if config.log_file_enabled {
            s.logger
                .write_to_file(&format!("--- {} ({:.1}MB) ---", filename, size_mb));
        }
    }

    // Emit file-created event
    let _ = app.emit(
        "file-created",
        FileCreatedPayload {
            path: path.to_string_lossy().to_string(),
            filename: filename.clone(),
            timestamp,
            size_mb,
            is_warning,
        },
    );

    // Play clip saved sound
    if config.clip_save_sound_enabled {
        let resource_dir = app.path().resource_dir().unwrap_or_default();
        sound::play_clip_saved(&config.clip_save_sound_custom, &resource_dir);
    }

    // Play error sound for small files
    if is_warning && config.error_sound_enabled {
        let resource_dir = app.path().resource_dir().unwrap_or_default();
        sound::play_error(&config.error_sound_custom, &resource_dir);
    }

    // Start timer if enabled
    if config.timer_enabled {
        let duration_secs = config.timer_duration_secs() as u32;
        let _ = timer_tx
            .send(TimerCommand::Start { duration_secs })
            .await;
        let mut s = state.lock().unwrap();
        s.timer_running = true;
    }
}

/// Parse time from an OBS or ShadowPlay filename.
/// OBS: "Replay 2026-04-15 12-30-00.mp4" -> "12:30:00"
/// ShadowPlay: "Game 2026.04.15 - 12.30.00.mp4" -> "12:30:00"
fn parse_time_from_filename(filename: &str) -> String {
    // OBS format: YYYY-MM-DD HH-MM-SS
    let obs_re = Regex::new(r"\d{4}-\d{2}-\d{2} (\d{2})-(\d{2})-(\d{2})").unwrap();
    if let Some(caps) = obs_re.captures(filename) {
        return format!("{}:{}:{}", &caps[1], &caps[2], &caps[3]);
    }

    // ShadowPlay format: YYYY.MM.DD - HH.MM.SS
    let sp_re = Regex::new(r"\d{4}\.\d{2}\.\d{2} - (\d{2})\.(\d{2})\.(\d{2})").unwrap();
    if let Some(caps) = sp_re.captures(filename) {
        return format!("{}:{}:{}", &caps[1], &caps[2], &caps[3]);
    }

    String::new()
}
