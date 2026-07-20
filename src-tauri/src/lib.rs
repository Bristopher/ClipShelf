mod clickthrough;
mod commands;
mod config;
mod events;
mod gamedetect;
mod history;
mod hotkeys;
mod keyhook;
mod logger;
mod mover;
mod obs_ws;
mod overlay;
mod props;
mod sound;
mod state;
mod stats;
mod theme;
mod thumbs;
mod timer;
mod tray;
mod updater;
mod watcher;
mod window_layout;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use regex::Regex;
use tauri::window::Color;
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tokio::sync::mpsc;

use config::AppConfig;
use events::*;
use hotkeys::HotkeyAction;
use obs_ws::ObsWsEvent;
use state::{AppState, AppStateInner, ChannelState, CurrentFile};
use timer::{CountUpCommand, TimerCommand};
use watcher::{WatcherCommand, WatcherEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Must be the first plugin. A second launch of the exe forwards to
        // this instance (we surface the existing window) and exits — no more
        // duplicate watchers or hotkey-registration fights.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
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

            // Startup update check (consent-based; config-gated).
            updater::spawn_startup_check(app_handle.clone(), config.check_updates);

            // Hold-to-click-through watcher for the main window.
            clickthrough::configure(config.click_through_enabled, &config.click_through_key);
            clickthrough::spawn(app_handle.clone());

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

            // Spawn the hotkey listener up-front so we can stash its
            // controller into ChannelState. update_config calls
            // controller.reload(...) when the user changes any bind.
            let initial_bindings = hotkeys::bindings_from_config(&config);
            let (mut hotkey_rx, mut hotkey_failure_rx, hotkey_controller) =
                hotkeys::spawn_hotkey_listener(initial_bindings)
                    .expect("Failed to spawn hotkey listener");

            // Surface hotkey registration failures — a bind another app owns
            // (Discord, Steam, ...) silently stops working otherwise.
            {
                let app_handle = app_handle.clone();
                let state = app_state.clone();
                tauri::async_runtime::spawn(async move {
                    while let Some(failures) = hotkey_failure_rx.recv().await {
                        for f in failures {
                            let msg = format!(
                                "Hotkey '{}' for {} could not be registered: {}",
                                f.binding, f.action, f.reason
                            );
                            {
                                let Ok(mut s) = state.lock() else { continue };
                                let entry = s.logger.log(
                                    LogLevel::Error,
                                    msg.clone(),
                                    LogCategory::System,
                                );
                                let _ = app_handle.emit("log-entry", &entry);
                            }
                            let _ = app_handle.emit(
                                "error",
                                ErrorPayload {
                                    message: msg,
                                    context: "hotkeys".to_string(),
                                },
                            );
                        }
                    }
                });
            }

            // Spawn the OBS WebSocket actor unconditionally — while disabled
            // it just idles waiting for a Configure from update_config, so
            // enabling it in Settings works without an app restart.
            let (obs_cmd_tx, mut obs_event_rx) = obs_ws::spawn_obs_ws(
                config.obs_websocket_enabled,
                config.obs_websocket_password.clone(),
            );

            // Clone the controller for the hotkey-action handler task (the
            // OverlayToggle/OverlayKey arms register/release the temp keys);
            // the original moves into ChannelState for the update_config path.
            let hotkey_controller_for_handler = hotkey_controller.clone();

            // Create ChannelState
            let channel_state = ChannelState {
                user_timer_tx: user_timer_tx.clone(),
                watcher_tx: watcher_tx.clone(),
                count_up_tx: count_up_tx.clone(),
                obs_cmd_tx: obs_cmd_tx.clone(),
                hotkey_controller,
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
                                // While the user has watching paused, the
                                // watcher's own "stopped" reads as "paused"
                                // in the UI.
                                let status = {
                                    let s = state.lock().unwrap();
                                    if s.watch_paused && status == "stopped" {
                                        "paused".to_string()
                                    } else {
                                        status
                                    }
                                };
                                {
                                    let mut s = state.lock().unwrap();
                                    s.watcher_restart_count = restart_count;
                                    s.last_watcher_status = status.clone();
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

            // Spawn hotkey event handler. The listener thread itself was
            // spawned earlier (so its controller could be put in
            // ChannelState); here we just consume the receiver.
            {
                let app_handle = app_handle.clone();
                let watcher_tx = watcher_tx.clone();
                let state = app_state.clone();
                let timer_tx = timer_tx.clone();
                let count_up_tx = count_up_tx.clone();
                let controller = hotkey_controller_for_handler;

                tauri::async_runtime::spawn(async move {
                    while let Some(action) = hotkey_rx.recv().await {
                        match action {
                            // G1-G3 are handled fully in Rust — no webview
                            // round-trip, so a hotkey works even while the
                            // frontend is still loading (or hung). The move
                            // does blocking IO with retry sleeps, so it runs
                            // on the blocking pool.
                            HotkeyAction::MoveG1 | HotkeyAction::MoveG2 | HotkeyAction::MoveG3 => {
                                let key = match action {
                                    HotkeyAction::MoveG1 => 1,
                                    HotkeyAction::MoveG2 => 2,
                                    _ => 3,
                                };
                                let app = app_handle.clone();
                                let st = state.clone();
                                tauri::async_runtime::spawn_blocking(move || {
                                    commands::do_press_gkey(&app, &st, key, "hotkey");
                                });
                            }
                            HotkeyAction::Rename => {
                                let _ = app_handle
                                    .emit("hotkey-triggered", serde_json::json!({"key": 4}));
                            }
                            HotkeyAction::RestartWatcher => {
                                let _ = watcher_tx.send(WatcherCommand::Restart).await;
                            }
                            HotkeyAction::CountUpToggle => {
                                let _ = count_up_tx.send(CountUpCommand::Toggle).await;
                            }
                            HotkeyAction::Undo => {
                                let app = app_handle.clone();
                                let st = state.clone();
                                tauri::async_runtime::spawn_blocking(move || {
                                    let _ = commands::do_undo(&app, &st);
                                });
                            }
                            HotkeyAction::SaveClipHealthCheck => {
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
                                    let _ = app_handle
                                        .emit("calibration-armed", serde_json::json!({}));
                                }
                                // Game detection: the user is in-game at this
                                // exact instant — snapshot the foreground app
                                // for the clip about to arrive. Win32 calls are
                                // cheap but not free; do them off the async loop.
                                {
                                    let st = state.clone();
                                    tauri::async_runtime::spawn_blocking(move || {
                                        let (enabled, overrides) = {
                                            let s = st.lock().unwrap();
                                            (s.config.game_detection_enabled, s.config.game_overrides.clone())
                                        };
                                        if enabled {
                                            let snap = gamedetect::snapshot_foreground(&overrides);
                                            let mut s = st.lock().unwrap();
                                            s.pending_game = snap;
                                        }
                                    });
                                }
                                spawn_save_clip_health_check(
                                    app_handle.clone(),
                                    state.clone(),
                                    watcher_tx.clone(),
                                    timer_tx.clone(),
                                );
                            }
                            HotkeyAction::OverlayToggle => {
                                // Toggle on visibility: if the overlay is up,
                                // close it (hide + release temp keys); else
                                // open it (show + register temp keys + emit).
                                let visible = app_handle
                                    .get_webview_window("overlay")
                                    .and_then(|w| w.is_visible().ok())
                                    .unwrap_or(false);
                                if visible {
                                    overlay::close(&app_handle, &controller);
                                } else {
                                    overlay::open(&app_handle, &controller, &state);
                                }
                            }
                            // Esc sentinel closes the overlay, symmetric with
                            // the toggle's close path.
                            HotkeyAction::OverlayKey(10) => {
                                overlay::close(&app_handle, &controller);
                            }
                            // Digit selections (1-9, 0) and arrow navigation
                            // (11 = up, 12 = down) are handed to the overlay
                            // webview to interpret.
                            HotkeyAction::OverlayKey(n) => {
                                let _ = app_handle.emit("overlay-key", n);
                            }
                        }
                    }
                });
            }

            // Handle OBS WebSocket events (actor spawned above ChannelState)
            {
                let app_handle = app_handle.clone();
                let state = app_state.clone();
                let timer_tx = timer_tx.clone();

                tauri::async_runtime::spawn(async move {
                    while let Some(event) = obs_event_rx.recv().await {
                        match event {
                            ObsWsEvent::Connected => {
                                let mut s = state.lock().unwrap();
                                s.last_obs_status = "connected".to_string();
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
                                s.last_obs_status = "disconnected".to_string();
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
                                // OBS tells us the exact saved path — inject it
                                // straight into the file pipeline. Faster than
                                // the watcher and immune to notify wedging; the
                                // dedup guard in handle_file_created swallows
                                // whichever of the two signals arrives second.
                                let config = {
                                    let mut s = state.lock().unwrap();
                                    let entry = s.logger.log(
                                        LogLevel::Info,
                                        format!("OBS replay saved: {}", path),
                                        LogCategory::ObsWebSocket,
                                    );
                                    let _ = app_handle.emit("log-entry", &entry);
                                    s.config.clone()
                                };
                                let file_path = PathBuf::from(&path);
                                if watcher::is_video_file(&file_path) {
                                    handle_file_created(
                                        &app_handle,
                                        &state,
                                        &timer_tx,
                                        &config,
                                        file_path,
                                    )
                                    .await;
                                }
                            }
                            ObsWsEvent::AuthError { message } => {
                                {
                                    let mut s = state.lock().unwrap();
                                    let entry = s.logger.log(
                                        LogLevel::Error,
                                        format!("OBS auth error: {}", message),
                                        LogCategory::ObsWebSocket,
                                    );
                                    let _ = app_handle.emit("log-entry", &entry);
                                }
                                // Wrong password is a user-fixable problem —
                                // route it to the visible error surface, not
                                // just the log.
                                let _ = app_handle.emit(
                                    "error",
                                    ErrorPayload {
                                        message: format!("OBS WebSocket auth failed: {}", message),
                                        context: "obs".to_string(),
                                    },
                                );
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
                                {
                                    let mut s = state.lock().unwrap();
                                    s.last_obs_status = status.clone();
                                }
                                let _ = app_handle.emit(
                                    "obs-ws-status",
                                    ObsWsStatusPayload { status, attempt },
                                );
                            }
                        }
                    }
                });
            }

            // Restore the remembered window layout, or fall back to the
            // configured default open position (monitor + anchor corner).
            if let Some(window) = app_handle.get_webview_window("main") {
                window_layout::apply_startup_layout(&window, &config, &config_path);
            }

            // Sync the OS autostart registration with the config toggle.
            {
                use tauri_plugin_autostart::ManagerExt;
                let autolaunch = app_handle.autolaunch();
                let result = if config.autostart_enabled {
                    autolaunch.enable()
                } else {
                    autolaunch.disable()
                };
                if let Err(e) = result {
                    // Disabling an entry that was never registered errors on
                    // some platforms — harmless, log at debug only.
                    log::debug!("autostart sync: {}", e);
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
            .title("ClipShelf — Settings")
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
            .title("ClipShelf — Setup")
            .inner_size(520.0, 560.0)
            .min_inner_size(480.0, 480.0)
            .resizable(true)
            .decorations(false)
            .center()
            .visible(false)
            .background_color(Color(10, 10, 10, 255))
            .build();

            // Overlay window: non-activating always-on-top panel shown over
            // the game via show_overlay/hide_overlay. Pre-created here like
            // settings/first-run above, for the same reason.
            overlay::init(&app_handle);

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
                    "ClipShelf started".to_string(),
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
            commands::show_main_window,
            commands::show_main_window_noactivate,
            commands::hide_main_window,
            commands::hide_tray_menu,
            updater::manual_update_check,
            updater::check_update_status,
            updater::install_update,
            updater::open_releases_page,
            commands::undo_last_action,
            commands::reveal_in_explorer,
            commands::set_watch_paused,
            commands::get_monitor_count,
            commands::get_watcher_status,
            commands::get_obs_status,
            commands::drop_files_to_gkey,
            commands::select_dropped_file,
            commands::get_gkey_stats,
            commands::get_diagnostics,
            commands::test_obs_connection,
            commands::get_history,
            commands::edit_history_game,
            overlay::show_overlay,
            overlay::hide_overlay,
            overlay::overlay_set_target,
            overlay::overlay_clear_target,
            overlay::overlay_get_context,
            overlay::overlay_history,
            overlay::overlay_sort,
            overlay::overlay_rate,
            overlay::overlay_label,
            overlay::overlay_describe,
            overlay::overlay_set_game,
            overlay::overlay_timer_toggle,
            overlay::overlay_timer_reset,
            overlay::overlay_needs_label,
            keyhook::start_type_mode,
            keyhook::stop_type_mode,
            thumbs::clip_thumbnail,
        ])
        .on_window_event(|window, event| {
            match event {
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    // Hide window instead of closing (minimize to tray). The
                    // overlay routes through overlay::close so its temporary
                    // digit/Esc hotkeys are ALWAYS released with the window.
                    if window.label() == overlay::LABEL {
                        let channels = window.app_handle().state::<ChannelState>();
                        overlay::close(window.app_handle(), &channels.hotkey_controller);
                    } else {
                        let _ = window.hide();
                    }
                    api.prevent_close();
                }
                // Remember the main window's layout (debounced).
                tauri::WindowEvent::Moved(_) | tauri::WindowEvent::Resized(_) => {
                    if window.label() == "main" {
                        window_layout::schedule_layout_save(window);
                    }
                }
                _ => {}
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Handle a newly created file from the watcher, OBS-WS, or the health-check
/// rescan. Runs the dedup gate, then classifies the file in a background
/// task: if OBS releases its exclusive write lock within a few seconds it's
/// a replay-buffer clip → normal clip flow. If the lock is held longer it's
/// a RECORDING still being written — the clip flow (current-file arming,
/// history, sound, timer) is deferred until the recording finishes, so
/// G-keys can never move/rename a file OBS holds open.
async fn handle_file_created(
    app: &tauri::AppHandle,
    state: &AppState,
    timer_tx: &mpsc::Sender<TimerCommand>,
    config: &AppConfig,
    path: PathBuf,
) {
    if !created_dedup_gate(state, &path) {
        return;
    }
    let arrival = std::time::SystemTime::now();
    let app = app.clone();
    let state = state.clone();
    let timer_tx = timer_tx.clone();
    let config = config.clone();
    // Classification probes + sleeps must not stall the caller's event loop
    // (a clip saved while a recording is being classified would wait).
    tauri::async_runtime::spawn(async move {
        classify_and_process(app, state, timer_tx, config, path, arrival).await;
    });
}

/// Dedup: the same clip can be reported twice — OBS WebSocket + folder
/// watcher (or the health-check rescan, or the recording monitor finishing).
/// First signal wins; anything re-reporting the same raw path within 10s is
/// dropped. Also the single choke point for pause: no source may inject
/// files while paused.
///
/// Check AND mark in one critical section — the reporting tasks are
/// independent, so a check-then-mark-later pattern lets two of them pass the
/// check for the same clip before either records it (double log entry,
/// double sound, timer restarted twice).
fn created_dedup_gate(state: &AppState, path: &PathBuf) -> bool {
    let mut s = state.lock().unwrap();
    if s.watch_paused {
        return false;
    }
    if let (Some(prev), Some(at)) = (&s.last_created_path, s.last_file_created_at) {
        if *prev == *path
            && at.elapsed().unwrap_or_default() < std::time::Duration::from_secs(10)
        {
            return false;
        }
    }
    s.last_created_path = Some(path.clone());
    s.last_file_created_at = Some(std::time::SystemTime::now());
    true
}

/// How long a new file may stay exclusively locked before it's classified as
/// an in-progress recording instead of a clip. Replay-buffer clips finish
/// writing (and release the lock) well within a couple of seconds.
const CLIP_CLASSIFY_ATTEMPTS: u32 = 10;
const CLIP_CLASSIFY_INTERVAL_SECS: u64 = 1;
/// Poll cadence + cap while waiting for a recording to finish.
const RECORDING_POLL_SECS: u64 = 5;
const RECORDING_MAX_SECS: u64 = 12 * 60 * 60;

/// The game snapshot for a new clip: prefer the snapshot taken at the
/// save-press instant; fall back to "what's focused right now" for files
/// that arrived without a hotkey press (watcher-only / OBS event).
async fn resolve_game_snapshot(
    state: &AppState,
    config: &AppConfig,
) -> Option<gamedetect::GameSnapshot> {
    let taken = {
        let mut s = state.lock().unwrap();
        s.take_pending_game(std::time::Duration::from_secs(30))
    };
    match taken {
        Some(snap) => Some(snap),
        None if config.game_detection_enabled => {
            let overrides = config.game_overrides.clone();
            tauri::async_runtime::spawn_blocking(move || {
                gamedetect::snapshot_foreground(&overrides)
            })
            .await
            .ok()
            .flatten()
        }
        None => None,
    }
}

/// Classify a freshly created video file (clip vs in-progress recording) and
/// run the clip flow at the right moment.
async fn classify_and_process(
    app: tauri::AppHandle,
    state: AppState,
    timer_tx: mpsc::Sender<TimerCommand>,
    config: AppConfig,
    path: PathBuf,
    arrival: std::time::SystemTime,
) {
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    for _ in 0..CLIP_CLASSIFY_ATTEMPTS {
        if !path.exists() {
            log::info!("created file vanished during classification: {}", filename);
            return;
        }
        let p = path.clone();
        let free = tauri::async_runtime::spawn_blocking(move || props::probe_exclusive(&p))
            .await
            .unwrap_or(false);
        if free {
            process_clip(&app, &state, &timer_tx, &config, path, arrival, None).await;
            return;
        }
        tokio::time::sleep(std::time::Duration::from_secs(CLIP_CLASSIFY_INTERVAL_SECS)).await;
    }

    // Exclusively held past the classification window: OBS is still writing
    // — this is a recording, not a clip. Capture the game NOW (what's being
    // recorded is focused now, not whatever happens to be focused when the
    // recording eventually stops), then wait for the lock to release.
    let game_snap = resolve_game_snapshot(&state, &config).await;
    {
        let mut s = state.lock().unwrap();
        let entry = s.logger.log_with_path(
            LogLevel::Info,
            format!(
                "Recording in progress: {} — it will be ready to sort when it finishes",
                filename
            ),
            LogCategory::FileCreated,
            Some(path.to_string_lossy().to_string()),
        );
        let _ = app.emit("log-entry", &entry);
    }

    let started = std::time::Instant::now();
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(RECORDING_POLL_SECS)).await;
        if !path.exists() {
            let mut s = state.lock().unwrap();
            let entry = s.logger.log(
                LogLevel::Warning,
                format!("Recording disappeared before finishing: {}", filename),
                LogCategory::FileCreated,
            );
            let _ = app.emit("log-entry", &entry);
            return;
        }
        let p = path.clone();
        let free = tauri::async_runtime::spawn_blocking(move || props::probe_exclusive(&p))
            .await
            .unwrap_or(false);
        if free {
            break;
        }
        if started.elapsed().as_secs() > RECORDING_MAX_SECS {
            log::warn!(
                "recording monitor: {} still locked after 12h — giving up",
                filename
            );
            return;
        }
    }

    // The OBS-WS record-stop event (or a watcher rescan) may report the file
    // again right as it unlocks — same gate as any other creation source.
    if !created_dedup_gate(&state, &path) {
        return;
    }
    // Settings may have changed during a long recording — use the live ones.
    let config = {
        let s = state.lock().unwrap();
        s.config.clone()
    };
    let mins = started.elapsed().as_secs() / 60;
    {
        let mut s = state.lock().unwrap();
        let entry = s.logger.log_with_path(
            LogLevel::Info,
            format!(
                "Recording finished: {} ({} min) — treating it as a clip now",
                filename,
                mins.max(1)
            ),
            LogCategory::FileCreated,
            Some(path.to_string_lossy().to_string()),
        );
        let _ = app.emit("log-entry", &entry);
    }
    process_clip(
        &app,
        &state,
        &timer_tx,
        &config,
        path,
        std::time::SystemTime::now(),
        Some(game_snap),
    )
    .await;
}

/// The full clip flow: current-file arming, calibration sample, game
/// detection, history + property write, log/sound/event/timer. `arrival` is
/// when the file was first reported (calibration measures against it).
/// `game_override`: `Some(snap)` uses a snapshot resolved earlier (recording
/// flow); `None` resolves one now.
async fn process_clip(
    app: &tauri::AppHandle,
    state: &AppState,
    timer_tx: &mpsc::Sender<TimerCommand>,
    config: &AppConfig,
    path: PathBuf,
    arrival: std::time::SystemTime,
    game_override: Option<Option<gamedetect::GameSnapshot>>,
) {
    let size_mb = mover::file_size_mb(&path);
    let is_warning = size_mb < config.small_file_warn_mb;

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
        });
        s.bind_chosen = None;
        // (last_created_path / last_file_created_at were already recorded in
        // the dedup gate.) Calibration measures save-press → file-appearance,
        // so it uses the ARRIVAL time — classification probes must not skew
        // the sample.
        let now = arrival;

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

    // Game detection: the recording flow passes a snapshot resolved when the
    // recording was detected; everything else resolves one now.
    let game_snap: Option<gamedetect::GameSnapshot> = match game_override {
        Some(snap) => snap,
        None => resolve_game_snapshot(state, config).await,
    };
    let game: Option<String> = game_snap.as_ref().map(|s| s.label.clone());
    let (config_path, hist_event) = {
        let mut s = state.lock().unwrap();
        if let Some(g) = &game {
            s.clip_games.insert(path.clone(), g.clone());
        }
        // Parallel exe map — kept in lockstep with clip_games at this and
        // every re-key site (via rekey_clip) so the overlay's set-game can
        // remember a per-exe override for this clip later.
        if let Some(snap) = &game_snap {
            s.clip_exes.insert(path.clone(), snap.exe_stem.clone());
        }
        let mut e = history::HistoryEvent::new("created", &path, "app");
        if let Some(g) = &game {
            e = e.with_game(g);
        }
        if let Some(snap) = &game_snap {
            e = e.with_exe(&snap.exe_stem);
        }
        (s.config_path.clone(), e)
    };
    let hist_path = history::history_path(&config_path);
    let write_props = config.write_file_properties;
    {
        let creation_path = path.clone();
        let game_for_props = game.clone();
        let state_for_props = state.clone();
        let app_for_props = app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            history::append(&hist_path, &hist_event);
            if write_props {
                if let Some(g) = game_for_props {
                    // Re-resolve the clip's current path before every probe:
                    // if the user G-key-sorts this clip while OBS still holds
                    // the file, clip_games is re-keyed to the new path and the
                    // creation path no longer exists — the write must follow
                    // the clip, not probe a dead path until it gives up. Lock
                    // ONLY to read the path, then drop before probing/sleeping.
                    let resolve = || -> Option<std::path::PathBuf> {
                        let s = state_for_props.lock().ok()?;
                        // Identity-preserving only: creation path if unmoved,
                        // else this clip's own move chain from the undo stack,
                        // else None (skip + warn). Never "the newest clip" —
                        // guessing could stamp another clip's metadata.
                        state::resolve_clip_current_path(
                            &s.clip_games,
                            &s.undo_stack,
                            &creation_path,
                        )
                    };
                    if let Err(msg) = props::write_with_retry_resolving(
                        resolve,
                        &[props::PropValue::Game(g)],
                        props::PROBE_ATTEMPTS,
                        std::time::Duration::from_millis(props::PROBE_DELAY_MS),
                    ) {
                        // Keep the dev-console line, but also surface it in the
                        // app log + UI so the skip is visible in release builds.
                        eprintln!("props: {}", msg);
                        if let Ok(mut s) = state_for_props.lock() {
                            let entry = s.logger.log(
                                LogLevel::Warning,
                                format!("Property write skipped: {}", msg),
                                LogCategory::System,
                            );
                            let _ = app_for_props.emit("log-entry", &entry);
                        }
                    }
                }
            }
        });
    }

    {
        let mut s = state.lock().unwrap();

        // Log file creation
        let level = if is_warning {
            LogLevel::Warning
        } else {
            LogLevel::Info
        };
        let mut msg = if is_warning {
            format!(
                "New file: {} ({:.1}MB - possible black screen)",
                filename, size_mb
            )
        } else {
            format!("New file: {} ({:.1}MB)", filename, size_mb)
        };
        if let Some(g) = &game {
            msg.push_str(&format!(" — {}", g));
        }
        let entry = s.logger.log_with_path(
            level,
            msg,
            LogCategory::FileCreated,
            Some(path.to_string_lossy().to_string()),
        );
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
            game: game.clone(),
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

static OBS_TIME_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(r"\d{4}-\d{2}-\d{2} (\d{2})-(\d{2})-(\d{2})").unwrap()
});
static SP_TIME_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(r"\d{4}\.\d{2}\.\d{2} - (\d{2})\.(\d{2})\.(\d{2})").unwrap()
});

/// Parse time from an OBS or ShadowPlay filename.
/// OBS: "Replay 2026-04-15 12-30-00.mp4" -> "12:30:00"
/// ShadowPlay: "Game 2026.04.15 - 12.30.00.mp4" -> "12:30:00"
fn parse_time_from_filename(filename: &str) -> String {
    if let Some(caps) = OBS_TIME_RE.captures(filename) {
        return format!("{}:{}:{}", &caps[1], &caps[2], &caps[3]);
    }
    if let Some(caps) = SP_TIME_RE.captures(filename) {
        return format!("{}:{}:{}", &caps[1], &caps[2], &caps[3]);
    }
    String::new()
}
