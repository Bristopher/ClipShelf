use tauri::{AppHandle, Emitter, Manager, State};

use crate::config::AppConfig;
use crate::events::*;
use crate::mover;
use crate::sound;
use crate::state::{AppState, ChannelState, CurrentFile};
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
    app: AppHandle,
) -> Result<AppConfig, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.config.merge_partial(partial);
    s.config.save().map_err(|e| e.to_string())?;
    let config = s.config.clone();
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
            let entry = s
                .logger
                .log(LogLevel::Error, "No current_file".into(), LogCategory::System);
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

#[tauri::command]
pub fn restart_watcher(channels: State<'_, ChannelState>) -> Result<(), String> {
    channels
        .watcher_tx
        .try_send(WatcherCommand::Restart)
        .map_err(|e| format!("Failed to send restart command: {}", e))
}

#[tauri::command]
pub fn open_folder(path: String) -> Result<(), String> {
    opener::open(&path).map_err(|e| e.to_string())
}
