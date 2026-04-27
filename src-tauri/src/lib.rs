mod commands;
mod config;
mod events;
mod hotkeys;
mod logger;
mod mover;
mod obs_ws;
mod sound;
mod state;
mod theme;
mod timer;
mod tray;
mod watcher;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use regex::Regex;
use tauri::window::Color;
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tokio::sync::mpsc;

use config::AppConfig;
use events::*;
use hotkeys::HotkeyAction;
use obs_ws::{ObsWsCommand, ObsWsEvent};
use state::{AppState, AppStateInner, ChannelState, CurrentFile};
use timer::{CountUpCommand, TimerCommand};
use watcher::{WatcherCommand, WatcherEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let app_handle = app.handle().clone();

            // Resolve persistent config path: %APPDATA%/com.cbuzi.gkey-mover-v2/
            // on Windows (via Tauri's app_config_dir). Create the directory
            // if missing. Falls back to the exe-adjacent path if Tauri can't
            // resolve the dir (should never happen in practice).
            let config_path = match app_handle.path().app_config_dir() {
                Ok(dir) => {
                    if let Err(e) = std::fs::create_dir_all(&dir) {
                        log::warn!("Failed to create app config dir {:?}: {}", dir, e);
                    }
                    dir.join("gkey_config.toml")
                }
                Err(e) => {
                    log::warn!("app_config_dir unavailable ({}), falling back to exe-relative", e);
                    AppConfig::config_path()
                }
            };

            // One-time migration: if a legacy config exists next to the exe
            // and the new one doesn't, copy it over so user settings carry.
            let legacy_path = AppConfig::config_path();
            if !config_path.exists() && legacy_path.exists() && legacy_path != config_path {
                if let Err(e) = std::fs::copy(&legacy_path, &config_path) {
                    log::warn!(
                        "Failed to migrate legacy config {:?} -> {:?}: {}",
                        legacy_path,
                        config_path,
                        e
                    );
                } else {
                    log::info!(
                        "Migrated legacy config {:?} -> {:?}",
                        legacy_path,
                        config_path
                    );
                }
            }

            let config = AppConfig::load_from(&config_path).unwrap_or_else(|e| {
                log::warn!("Failed to load config, using defaults: {}", e);
                AppConfig::default()
            });
            log::info!("Config path: {:?}", config_path);

            // Create AppState
            let app_state: AppState =
                Arc::new(Mutex::new(AppStateInner::new(config.clone(), config_path.clone())));
            app.manage(app_state.clone());

            // Set up system tray
            if let Err(e) = tray::setup_tray(&app_handle) {
                log::error!("Failed to set up system tray: {}", e);
            }

            // Spawn two independent timers:
            //   - `timer_tx`: auto-wipe timer that fires when a new clip
            //     arrives (events `timer-tick` / `timer-expired`).
            //   - `user_timer_tx`: manual countdown triggered from the UI
            //     Start button (events `user-timer-tick` /
            //     `user-timer-expired`). Runs independently so a user can
            //     time their replay-buffer save without colliding with
            //     a clip-arrival countdown.
            let timer_tx =
                timer::spawn_timer(app_handle.clone(), "timer-tick", "timer-expired");
            let user_timer_tx = timer::spawn_timer(
                app_handle.clone(),
                "user-timer-tick",
                "user-timer-expired",
            );
            let count_up_tx =
                timer::spawn_count_up_timer(app_handle.clone(), "count-up-tick");

            // Spawn file watcher
            let (watcher_tx, mut watcher_rx) = watcher::spawn_watcher();

            // Create ChannelState
            let channel_state = ChannelState {
                timer_tx: timer_tx.clone(),
                user_timer_tx: user_timer_tx.clone(),
                watcher_tx: watcher_tx.clone(),
                count_up_tx: count_up_tx.clone(),
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
                let watcher_tx = watcher_tx.clone();

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
                                {
                                    let mut s = state.lock().unwrap();
                                    let entry = s.logger.log(
                                        LogLevel::Error,
                                        format!("Watcher error: {} — auto-restarting", message),
                                        LogCategory::WatcherStatus,
                                    );
                                    let _ = app_handle.emit("log-entry", &entry);
                                    let _ = app_handle.emit(
                                        "error",
                                        ErrorPayload {
                                            message: message.clone(),
                                            context: "watcher".to_string(),
                                        },
                                    );
                                }
                                // notify on Windows (ReadDirectoryChangesW) can hit
                                // transient errors — buffer overflow during a fast
                                // OBS write, antivirus scan, drive sleep — and after
                                // that the watcher is dead until respawned. Restart
                                // automatically so the user doesn't have to relaunch
                                // the app. The save-clip-bind health check
                                // (`spawn_save_clip_health_check`) handles the
                                // *silent* failure mode where notify wedges without
                                // emitting an error.
                                let _ = watcher_tx.send(WatcherCommand::Restart).await;
                            }
                        }
                    }
                });
            }

            // Spawn hotkey listener
            {
                let app_handle = app_handle.clone();
                let watcher_tx = watcher_tx.clone();
                let state = app_state.clone();
                let timer_tx = timer_tx.clone();
                let count_up_tx = count_up_tx.clone();

                let mut bindings = vec![
                    (HotkeyAction::MoveG1, config.g1_bind.clone()),
                    (HotkeyAction::MoveG2, config.g2_bind.clone()),
                    (HotkeyAction::MoveG3, config.g3_bind.clone()),
                    (HotkeyAction::Rename, config.rename_bind.clone()),
                    (
                        HotkeyAction::RestartWatcher,
                        config.restart_watcher_bind.clone(),
                    ),
                ];

                // Only register the save-clip health check if the user has
                // actually configured a bind. Note: this uses RegisterHotKey
                // which is *exclusive* — if it's the same key the capture
                // software listens to, OBS won't see it. The expected setup
                // is a Logitech G Hub / AHK macro that fires both the
                // capture-app key AND a separate dedicated key for us.
                if !config.save_clip_bind.is_empty() {
                    bindings.push((
                        HotkeyAction::SaveClipHealthCheck,
                        config.save_clip_bind.clone(),
                    ));
                }
                if !config.count_up_bind.is_empty() {
                    bindings.push((
                        HotkeyAction::CountUpToggle,
                        config.count_up_bind.clone(),
                    ));
                }

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
                                    HotkeyAction::CountUpToggle => {
                                        let _ = count_up_tx
                                            .send(CountUpCommand::Toggle)
                                            .await;
                                    }
                                    HotkeyAction::SaveClipHealthCheck => {
                                        // Arm calibration if active so the next
                                        // FileCreated can compute its delta.
                                        let calibrating = {
                                            let mut s = state.lock().unwrap();
                                            if s.calibration.active {
                                                s.calibration.pending_save_at =
                                                    Some(std::time::SystemTime::now());
                                                true
                                            } else {
                                                false
                                            }
                                        };
                                        if calibrating {
                                            let _ = app_handle.emit(
                                                "calibration-armed",
                                                serde_json::json!({}),
                                            );
                                        }
                                        spawn_save_clip_health_check(
                                            app_handle.clone(),
                                            state.clone(),
                                            watcher_tx.clone(),
                                            timer_tx.clone(),
                                        );
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

            // Position window on second monitor if available
            if let Some(window) = app_handle.get_webview_window("main") {
                let monitors = window.available_monitors().unwrap_or_default();
                if monitors.len() > 1 {
                    let second = &monitors[1];
                    let pos = second.position();
                    let _ = window.set_position(tauri::Position::Physical(
                        tauri::PhysicalPosition { x: pos.x, y: pos.y },
                    ));
                }
            }

            // Apply saved window opacity
            if config.window_opacity < 1.0 {
                if let Some(window) = app_handle.get_webview_window("main") {
                    let opacity = config.window_opacity.clamp(0.2, 1.0);
                    let alpha = (opacity * 255.0) as u8;
                    if let Ok(hwnd) = window.hwnd() {
                        unsafe {
                            use windows_sys::Win32::UI::WindowsAndMessaging::*;
                            let ex_style = GetWindowLongW(hwnd.0, GWL_EXSTYLE);
                            SetWindowLongW(hwnd.0, GWL_EXSTYLE, ex_style | WS_EX_LAYERED as i32);
                            SetLayeredWindowAttributes(hwnd.0, 0, alpha, LWA_ALPHA);
                        }
                    }
                }
            }

            // Pre-create secondary windows during startup while Tauri still
            // has its full config (incl. --config devUrl overrides). Creating
            // them later from a Tauri command produces a blank/frozen webview
            // in dev, probably because the override devUrl isn't accessible
            // at runtime the same way it is during setup.
            let _ = WebviewWindowBuilder::new(
                &app_handle,
                "settings",
                WebviewUrl::App(std::path::PathBuf::new()),
            )
            .title("GKey Mover — Settings")
            .inner_size(640.0, 720.0)
            .min_inner_size(500.0, 500.0)
            .resizable(true)
            .decorations(false)
            .center()
            .visible(false)
            .background_color(Color(10, 10, 10, 255))
            .build();

            let _ = WebviewWindowBuilder::new(
                &app_handle,
                "first-run",
                WebviewUrl::App(std::path::PathBuf::new()),
            )
            .title("GKey Mover — Setup")
            .inner_size(520.0, 560.0)
            .min_inner_size(480.0, 480.0)
            .resizable(true)
            .decorations(false)
            .center()
            .visible(false)
            .background_color(Color(10, 10, 10, 255))
            .build();

            // Window starts hidden (see tauri.conf.json). Reveal now that
            // position and opacity have been applied — avoids the flash of
            // window appearing on the primary monitor before jumping.
            if let Some(window) = app_handle.get_webview_window("main") {
                let _ = window.show();
            }

            // Log startup message
            {
                let mut s = app_state.lock().unwrap();
                let entry = s.logger.log(
                    LogLevel::Info,
                    "GKey Mover started".to_string(),
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
            commands::set_window_opacity,
            commands::reset_window,
            commands::import_theme,
            commands::export_theme,
            commands::get_system_theme_mode,
            commands::open_settings_window,
            commands::open_first_run_window,
            commands::start_user_timer,
            commands::reset_user_timer,
            commands::start_calibration,
            commands::cancel_calibration,
            commands::toggle_count_up,
            commands::full_quit,
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
    let calibration_event: Option<serde_json::Value> = {
        let mut s = state.lock().unwrap();
        s.current_file = Some(CurrentFile {
            path: path.clone(),
            moved_path: None,
            renamed: false,
        });
        s.bind_chosen = None;
        let now = std::time::SystemTime::now();
        s.last_file_created_at = Some(now);

        // Calibration recording — if a save was pending, record the delta.
        let mut emit: Option<serde_json::Value> = None;
        if s.calibration.active {
            if let Some(save_at) = s.calibration.pending_save_at.take() {
                let delta_ms = now
                    .duration_since(save_at)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                let sample = state::CalibrationSample {
                    filename: filename.clone(),
                    delta_ms,
                };
                s.calibration.samples.push(sample.clone());
                let index = s.calibration.samples.len();
                let target = s.calibration.target_samples;
                let complete = index >= target;
                if complete {
                    let avg = s
                        .calibration
                        .samples
                        .iter()
                        .map(|s| s.delta_ms)
                        .sum::<u64>()
                        / s.calibration.samples.len() as u64;
                    let worst = s
                        .calibration
                        .samples
                        .iter()
                        .map(|s| s.delta_ms)
                        .max()
                        .unwrap_or(0);
                    let best = s
                        .calibration
                        .samples
                        .iter()
                        .map(|s| s.delta_ms)
                        .min()
                        .unwrap_or(0);
                    s.calibration.active = false;
                    emit = Some(serde_json::json!({
                        "kind": "complete",
                        "filename": sample.filename,
                        "deltaMs": sample.delta_ms,
                        "index": index,
                        "target": target,
                        "averageMs": avg,
                        "worstMs": worst,
                        "bestMs": best,
                    }));
                } else {
                    emit = Some(serde_json::json!({
                        "kind": "sample",
                        "filename": sample.filename,
                        "deltaMs": sample.delta_ms,
                        "index": index,
                        "target": target,
                    }));
                }
            }
        }
        emit
    };
    if let Some(payload) = calibration_event {
        let _ = app.emit("calibration-event", payload);
    }
    {
        let mut s = state.lock().unwrap();

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

/// Watcher health check tied to the user's capture-app save-clip hotkey.
///
/// When the user hits their save-clip key we expect the capture software to
/// drop a new video file into the watched folder within a few seconds. If
/// that doesn't show up via the watcher, either (a) the capture software
/// failed to save (user error / hotkey mis-fire) or (b) the notify watcher
/// is wedged in a way that the auto-restart-on-error path didn't catch
/// (silent ReadDirectoryChangesW buffer-overflow drop, stale handle, etc.).
///
/// We can't tell (a) from (b) without looking at the disk, so we do exactly
/// that: rescan the watched folder for any video file with a modification
/// time newer than the moment the user pressed the key. If we find one,
/// it's case (b) — the watcher missed it and we recover by injecting it.
/// Otherwise we restart the watcher anyway as a defensive measure (cheap
/// and harmless) and stay quiet about case (a).
fn spawn_save_clip_health_check(
    app: tauri::AppHandle,
    state: AppState,
    watcher_tx: mpsc::Sender<WatcherCommand>,
    timer_tx: mpsc::Sender<TimerCommand>,
) {
    let save_at = std::time::SystemTime::now();
    let timeout_secs = {
        let s = state.lock().unwrap();
        s.config.save_clip_health_check_timeout_secs.max(1) as u64
    };

    tauri::async_runtime::spawn(async move {
        // Hardware-dependent — SSDs flush in <1s, slow HDDs / long replay
        // buffers can take 5-10s. User can tune via the calibration tool
        // in settings.
        tokio::time::sleep(std::time::Duration::from_secs(timeout_secs)).await;

        // Did a FileCreated land after the user pressed save?
        let (already_saw_file, videos_folder, current_path) = {
            let s = state.lock().unwrap();
            let saw = s
                .last_file_created_at
                .map(|t| t >= save_at)
                .unwrap_or(false);
            let folder = s.config.videos_folder.clone();
            let cur = s.current_file.as_ref().map(|f| f.path.clone());
            (saw, folder, cur)
        };

        if already_saw_file {
            return; // Healthy — watcher delivered, nothing to do.
        }

        if videos_folder.is_empty() {
            return; // No folder configured; nothing to scan.
        }

        // Scan for files modified since save_at. Pick the newest unseen one.
        let folder_path = std::path::PathBuf::from(&videos_folder);
        let mut newest: Option<(std::path::PathBuf, std::time::SystemTime)> = None;
        if let Ok(entries) = std::fs::read_dir(&folder_path) {
            for entry in entries.flatten() {
                let p = entry.path();
                if !watcher::is_video_file(&p) {
                    continue;
                }
                if current_path.as_ref().map(|c| c == &p).unwrap_or(false) {
                    continue; // Already the active file — not a recovery case.
                }
                if let Ok(meta) = entry.metadata() {
                    if let Ok(mtime) = meta.modified() {
                        if mtime >= save_at {
                            match &newest {
                                Some((_, prev)) if *prev >= mtime => {}
                                _ => newest = Some((p, mtime)),
                            }
                        }
                    }
                }
            }
        }

        // Restart the watcher regardless — if we got here, it missed an
        // event (or the user mis-fired their hotkey). Restart is cheap and
        // unwedges any silent failure mode.
        let _ = watcher_tx.send(WatcherCommand::Restart).await;

        if let Some((path, _)) = newest {
            // Recovery: the file is on disk but the watcher never reported
            // it. Inject it through the normal pipeline.
            {
                let mut s = state.lock().unwrap();
                let entry = s.logger.log(
                    LogLevel::Warning,
                    format!(
                        "Save-clip health check: watcher missed {} — recovered via folder rescan",
                        path.file_name().and_then(|n| n.to_str()).unwrap_or("?")
                    ),
                    LogCategory::WatcherStatus,
                );
                let _ = app.emit("log-entry", &entry);
            }
            let config = {
                let s = state.lock().unwrap();
                s.config.clone()
            };
            handle_file_created(&app, &state, &timer_tx, &config, path).await;
        } else {
            // Couldn't find a fresh file. Could be capture-software error
            // OR the clip just hasn't flushed yet. Quiet log; user will see
            // the watcher restart entry already.
            log::info!(
                "Save-clip health check: no new file found, watcher restarted defensively"
            );
        }
    });
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
