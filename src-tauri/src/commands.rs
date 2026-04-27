use tauri::window::Color;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};

use crate::config::AppConfig;
use crate::events::*;
use crate::mover;
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
    let prev_folder = {
        let s = state.lock().map_err(|e| e.to_string())?;
        s.config.videos_folder.clone()
    };

    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.config.merge_partial(partial);
    let path = s.config_path.clone();
    s.config.save_to(&path).map_err(|e| e.to_string())?;
    let config = s.config.clone();
    drop(s);

    // If the videos folder changed (including the first-run case where it
    // was empty at startup and is now set), (re)start the file watcher —
    // otherwise new clips are never seen until app restart.
    if config.videos_folder != prev_folder && !config.videos_folder.is_empty() {
        let path = std::path::PathBuf::from(&config.videos_folder);
        let watcher_tx = channels.watcher_tx.clone();
        tauri::async_runtime::spawn(async move {
            let _ = watcher_tx.send(WatcherCommand::Start { path }).await;
        });
    }

    let _ = app.emit("config-changed", &config);
    Ok(config)
}

#[tauri::command]
pub fn press_gkey(key: u8, state: State<'_, AppState>, app: AppHandle) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
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
            return Ok(());
        }
    };

    let file_path = current
        .moved_path
        .as_ref()
        .unwrap_or(&current.path)
        .clone();
    let config = s.config.clone();
    let bind = s.bind_chosen.clone();
    drop(s); // Release lock before file operations

    match mover::move_or_rename_file(&file_path, key, &config) {
        Ok(result) => {
            let mut s = state.lock().map_err(|e| e.to_string())?;
            s.current_file = Some(CurrentFile {
                path: result.new_path.clone(),
                moved_path: None,
                renamed: false,
            });
            let mode = if config.disable_file_movesorting {
                "renamed"
            } else {
                "moved"
            };
            let msg = format!("File {} to {}", mode, result.tag_applied);
            let entry = s
                .logger
                .log(LogLevel::Success, msg, LogCategory::FileMoved);
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
                if let Some(ref bind) = bind {
                    let basename = file_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown");
                    s.logger
                        .write_to_file(&format!("{} | {}", bind, basename));
                }
            }
            if config.move_sound_enabled {
                let resource_dir = app.path().resource_dir().unwrap_or_default();
                sound::play_move_beep(&resource_dir);
            }
        }
        Err(e) => {
            let mut s = state.lock().map_err(|e| e.to_string())?;
            let entry = s.logger.log(
                LogLevel::Error,
                format!("Move failed: {}", e),
                LogCategory::System,
            );
            let _ = app.emit("log-entry", &entry);
        }
    }
    Ok(())
}

#[tauri::command]
pub fn rename_file(text: String, state: State<'_, AppState>, app: AppHandle) -> Result<(), String> {
    let (file_path, log_enabled) = {
        let s = state.lock().map_err(|e| e.to_string())?;
        let current = match &s.current_file {
            Some(cf) => cf.clone(),
            None => {
                // Need mutable borrow for logging - drop and re-acquire
                drop(s);
                let mut s = state.lock().map_err(|e| e.to_string())?;
                let entry = s
                    .logger
                    .log(LogLevel::Error, "No current_file".into(), LogCategory::System);
                let _ = app.emit("log-entry", &entry);
                return Ok(());
            }
        };
        let file_path = current
            .moved_path
            .as_ref()
            .unwrap_or(&current.path)
            .clone();
        (file_path, s.config.log_file_enabled)
    };

    match mover::rename_file_with_text(&file_path, &text) {
        Ok(result) => {
            let mut s = state.lock().map_err(|e| e.to_string())?;
            s.current_file = Some(CurrentFile {
                path: result.new_path.clone(),
                moved_path: Some(result.new_path.clone()),
                renamed: true,
            });
            let new_name = result
                .new_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();
            let entry = s.logger.log(
                LogLevel::Success,
                format!("File renamed to: {}", new_name),
                LogCategory::FileRenamed,
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
        }
        Err(e) => {
            let mut s = state.lock().map_err(|e| e.to_string())?;
            let entry = s.logger.log(
                LogLevel::Error,
                format!("Rename failed: {}", e),
                LogCategory::System,
            );
            let _ = app.emit("log-entry", &entry);
        }
    }
    Ok(())
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
    let duration = duration_secs.unwrap_or_else(|| {
        let s = state.lock().expect("state lock poisoned");
        s.config.timer_duration_secs() as u32
    });
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
    let duration = duration_secs.unwrap_or_else(|| {
        let s = state.lock().expect("state lock poisoned");
        s.config.timer_duration_secs() as u32
    });
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
pub fn reset_window(window: tauri::Window) -> Result<(), String> {
    // Default size matches tauri.conf.json window config.
    const DEFAULT_W: u32 = 900;
    const DEFAULT_H: u32 = 260;

    let monitors = window.available_monitors().map_err(|e| e.to_string())?;
    let target = if monitors.len() > 1 {
        &monitors[1]
    } else {
        monitors.first().ok_or("no monitors available")?
    };
    let pos = target.position();

    window
        .set_position(tauri::Position::Physical(tauri::PhysicalPosition {
            x: pos.x,
            y: pos.y,
        }))
        .map_err(|e| e.to_string())?;
    window
        .set_size(tauri::Size::Physical(tauri::PhysicalSize {
            width: DEFAULT_W,
            height: DEFAULT_H,
        }))
        .map_err(|e| e.to_string())?;
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
