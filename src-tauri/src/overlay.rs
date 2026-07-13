//! In-game overlay window: a small always-on-top, non-activating panel that
//! shows over a fullscreen/borderless game without stealing input focus.
//!
//! Creation happens once at startup (see `init`, called from `lib.rs`
//! `setup`, mirroring the settings/first-run pre-creation pattern — building
//! it later from a command produces a blank/frozen webview in dev). The
//! window then stays hidden until `show`/`hide` toggle it. Task 6 wires the
//! actual G-key feedback UI into `OverlayApp.tsx`; this task only builds the
//! window plumbing.

use std::path::PathBuf;

use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, State, WebviewUrl, WebviewWindowBuilder,
};

use crate::events::{ErrorPayload, FileRenamedPayload, LogCategory, LogLevel};
use crate::hotkeys::HotkeyController;
use crate::state::{AppState, AppStateInner, ChannelState, CurrentFile};
use crate::timer::CountUpCommand;

/// Window label for the overlay webview.
pub const LABEL: &str = "overlay";

/// Pre-create the overlay window during startup. Called from `lib.rs`
/// `setup` right after the settings/first-run windows are built.
pub fn init(app: &AppHandle) {
    let window = match WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App(std::path::PathBuf::new()))
        .title("GKey Mover — Overlay")
        .inner_size(420.0, 480.0)
        .resizable(false)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .visible(false)
        .build()
    {
        Ok(w) => w,
        Err(e) => {
            log::error!("Failed to create overlay window: {}", e);
            return;
        }
    };

    apply_noactivate(&window);
}

/// Set WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW on the overlay's HWND so clicking
/// it (or showing it) never steals foreground/input focus from whatever the
/// user has focused (the game). Without this, `show()` alone can still
/// activate the window on some window managers/games.
fn apply_noactivate(window: &tauri::WebviewWindow) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    };

    let hwnd = match window.hwnd() {
        Ok(h) => h,
        Err(e) => {
            log::error!("overlay: failed to get HWND: {}", e);
            return;
        }
    };

    unsafe {
        let prev = GetWindowLongPtrW(hwnd.0, GWL_EXSTYLE);
        SetWindowLongPtrW(
            hwnd.0,
            GWL_EXSTYLE,
            prev | (WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW) as isize,
        );
    }
}

/// Show the overlay positioned at the bottom-center of the monitor
/// containing the cursor, without taking focus. Emits `overlay-visible`
/// `{visible: true}` app-wide once shown.
pub fn show(app: &AppHandle) {
    let Some(window) = app.get_webview_window(LABEL) else {
        log::error!("overlay: show called before window was created");
        return;
    };

    if let Some((x, y)) = target_position(&window) {
        let _ = window.set_position(PhysicalPosition::new(x, y));
    }

    // Deliberately no `set_focus()` — that's the whole point of this window.
    let _ = window.show();

    let _ = app.emit("overlay-visible", serde_json::json!({ "visible": true }));
}

/// Hide the overlay. Emits `overlay-visible` `{visible: false}` app-wide.
pub fn hide(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.hide();
    }
    let _ = app.emit("overlay-visible", serde_json::json!({ "visible": false }));
}

/// Compute the bottom-center physical-pixel position for the overlay on the
/// monitor currently under the cursor (falls back to the window's current
/// monitor, then the primary monitor).
fn target_position(window: &tauri::WebviewWindow) -> Option<(i32, i32)> {
    let monitor = window
        .cursor_position()
        .ok()
        .and_then(|pos| window.monitor_from_point(pos.x, pos.y).ok().flatten())
        .or_else(|| window.current_monitor().ok().flatten())
        .or_else(|| window.primary_monitor().ok().flatten())?;

    let win_size = window.inner_size().ok()?;
    let mon_pos = monitor.position();
    let mon_size = monitor.size();

    let x = mon_pos.x + (mon_size.width as i32 - win_size.width as i32) / 2;
    let y = mon_pos.y + mon_size.height as i32 - win_size.height as i32 - 80;
    Some((x, y))
}

/// Open the overlay: show the window, register the temporary digit/Esc keys,
/// and emit `overlay-open` with the current clip's filename + detected game
/// (read under a short state lock). Every path that reveals the overlay goes
/// through here so key registration and the open event stay symmetric.
pub fn open(app: &AppHandle, controller: &HotkeyController, state: &AppState) {
    show(app);
    controller.set_overlay_keys(true);

    let (filename, game) = {
        let s = state.lock().unwrap();
        let filename = s
            .current_file
            .as_ref()
            .and_then(|f| f.path.file_name())
            .and_then(|n| n.to_str())
            .map(|n| n.to_string());
        let game = s
            .current_file
            .as_ref()
            .and_then(|f| s.clip_games.get(&f.path).cloned());
        (filename, game)
    };

    let _ = app.emit(
        "overlay-open",
        serde_json::json!({ "filename": filename, "game": game }),
    );
}

/// Close the overlay: hide the window and release the temporary keys. Used by
/// the toggle arm, the Esc sentinel arm, and the `hide_overlay` command so
/// the temp keys are ALWAYS released whenever the overlay goes away.
pub fn close(app: &AppHandle, controller: &HotkeyController) {
    hide(app);
    controller.set_overlay_keys(false);
    // Closing the overlay ALWAYS ends type mode — the LL keyboard hook must
    // never keep swallowing the game's keystrokes after the overlay is gone.
    crate::keyhook::stop();
}

/// Dev/testing command to show the overlay on demand.
#[tauri::command]
pub fn show_overlay(
    app: AppHandle,
    channels: State<'_, ChannelState>,
    state: State<'_, AppState>,
) {
    open(&app, &channels.hotkey_controller, state.inner());
}

/// Dev/testing command to hide the overlay on demand. Routed through `close`
/// so it releases the temporary overlay keys too.
#[tauri::command]
pub fn hide_overlay(app: AppHandle, channels: State<'_, ChannelState>) {
    close(&app, &channels.hotkey_controller);
}

// --- Overlay action commands (Task 4) ---

/// The clip every overlay action operates on: the most recent clip's CURRENT
/// acting path — `moved_path` if it's already been sorted/renamed this
/// session, else its original `path`. All action commands share this guard so
/// they fail uniformly with `"No recent clip"` when there's nothing to act on.
fn acting_clip(s: &AppStateInner) -> Result<PathBuf, String> {
    match &s.current_file {
        Some(cf) => Ok(cf.moved_path.as_ref().unwrap_or(&cf.path).clone()),
        None => Err("No recent clip".to_string()),
    }
}

/// G-key binds + folder names + the overlay toggle bind, so the overlay can
/// label its three sort buttons and show the active shortcuts.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayBinds {
    pub g1: String,
    pub g2: String,
    pub g3: String,
    pub g1_name: String,
    pub g2_name: String,
    pub g3_name: String,
    pub overlay: String,
}

/// One snapshot the overlay UI renders from — the current clip plus the
/// preset/bind config it needs, all read under a single short state lock.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayContext {
    pub filename: String,
    pub path: String,
    pub game: Option<String>,
    pub exe: Option<String>,
    pub label_presets: Vec<String>,
    pub description_presets: Vec<String>,
    pub typing_enabled: bool,
    pub binds: OverlayBinds,
}

/// Everything the overlay needs to render, from one short lock. Errors with
/// `"No recent clip"` when there's nothing to act on.
#[tauri::command]
pub fn overlay_get_context(state: State<'_, AppState>) -> Result<OverlayContext, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    let acting = acting_clip(&s)?;
    let filename = acting
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();
    let game = s.clip_games.get(&acting).cloned();
    let exe = s.clip_exes.get(&acting).cloned();
    let c = &s.config;
    Ok(OverlayContext {
        path: acting.to_string_lossy().to_string(),
        filename,
        game,
        exe,
        label_presets: c.label_presets.clone(),
        description_presets: c.description_presets.clone(),
        typing_enabled: c.overlay_typing_enabled,
        binds: OverlayBinds {
            g1: c.g1_bind.clone(),
            g2: c.g2_bind.clone(),
            g3: c.g3_bind.clone(),
            g1_name: c.g1_bind_folder_name.clone(),
            g2_name: c.g2_bind_folder_name.clone(),
            g3_name: c.g3_bind_folder_name.clone(),
            overlay: c.overlay_bind.clone(),
        },
    })
}

/// Sort the current clip with a G-key. Reuses the exact hotkey move path
/// (`do_press_gkey`) — same collision-safe move, log, sound, and undo push —
/// only tagging the history event source "overlay". Sync like `press_gkey`:
/// `do_press_gkey` blocks on move retries and Tauri runs it on a worker.
#[tauri::command]
pub fn overlay_sort(app: AppHandle, state: State<'_, AppState>, key: u8) -> Result<(), String> {
    if !(1..=3).contains(&key) {
        return Err(format!("Invalid gkey: {}. Must be 1, 2, or 3.", key));
    }
    crate::commands::do_press_gkey(&app, state.inner(), key, "overlay");
    Ok(())
}

/// Rate the current clip 1-5 stars. Appends a `rated` history event and, when
/// `write_file_properties` is on, mirrors System.Rating onto the file with the
/// same identity-resolving retry closure the create path uses. All disk work
/// happens after the lock drops.
#[tauri::command]
pub async fn overlay_rate(
    app: AppHandle,
    state: State<'_, AppState>,
    stars: u8,
) -> Result<(), String> {
    let stars = stars.clamp(1, 5);
    let (acting, game, write_props, config_path, entry) = {
        let mut s = state.lock().map_err(|e| e.to_string())?;
        let acting = acting_clip(&s)?;
        let game = s.clip_games.get(&acting).cloned();
        let write_props = s.config.write_file_properties;
        let config_path = s.config_path.clone();
        let filename = acting
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        let entry = s.logger.log_with_path(
            LogLevel::Success,
            format!("Rated \u{2605}{}: {}", stars, filename),
            LogCategory::System,
            Some(acting.to_string_lossy().to_string()),
        );
        (acting, game, write_props, config_path, entry)
    };
    let _ = app.emit("log-entry", &entry);

    let mut ev = crate::history::HistoryEvent::new("rated", &acting, "overlay").with_rating(stars);
    if let Some(g) = &game {
        ev = ev.with_game(g);
    }

    let state_for_props = state.inner().clone();
    let app_for_props = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        crate::history::append(&crate::history::history_path(&config_path), &ev);
        if write_props {
            write_prop_resolving(
                &app_for_props,
                &state_for_props,
                &acting,
                crate::props::PropValue::Stars(stars),
            );
        }
    });
    Ok(())
}

/// Add a one-keypress label to the current clip: collision-safe rename to
/// `{stem} - {label}{ext}` via the mover, then the same state bookkeeping the
/// rename dialog does (current_file/moved_path, re-key game+exe, undo push,
/// history `labeled` event, file-renamed emit). Runs the blocking rename on
/// the blocking pool like `rename_file`.
#[tauri::command]
pub async fn overlay_label(
    app: AppHandle,
    state: State<'_, AppState>,
    label: String,
) -> Result<(), String> {
    let label = label.trim().to_string();
    if label.is_empty() {
        return Err("Label cannot be empty".to_string());
    }
    let st = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || do_overlay_label(&app, &st, &label))
        .await
        .map_err(|e| e.to_string())?
}

fn do_overlay_label(app: &AppHandle, state: &AppState, label: &str) -> Result<(), String> {
    let (file_path, log_enabled) = {
        let s = state.lock().map_err(|e| e.to_string())?;
        (acting_clip(&s)?, s.config.log_file_enabled)
    };

    let target = crate::mover::labeled_name(&file_path, label);
    match crate::mover::rename_file_at(&file_path, &target) {
        Ok(result) => {
            let (config_path, game) = {
                let mut s = state.lock().map_err(|e| e.to_string())?;
                s.current_file = Some(CurrentFile {
                    path: result.new_path.clone(),
                    moved_path: Some(result.new_path.clone()),
                });
                let game = s.rekey_clip(&file_path, result.new_path.clone());
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
                    format!("Labeled: {}", new_name),
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
                        .write_to_file(&format!("Labeled: {} ---------> {}", old_name, new_name));
                }
                (s.config_path.clone(), game)
            };
            let mut ev = crate::history::HistoryEvent::new("labeled", &result.new_path, "overlay")
                .with_label(label)
                .with_old_path(&file_path);
            if let Some(g) = &game {
                ev = ev.with_game(g);
            }
            crate::history::append(&crate::history::history_path(&config_path), &ev);
            Ok(())
        }
        Err(e) => {
            let mut s = state.lock().map_err(|e| e.to_string())?;
            let entry = s.logger.log(
                LogLevel::Error,
                format!("Label failed: {}", e),
                LogCategory::System,
            );
            let _ = app.emit("log-entry", &entry);
            let _ = app.emit(
                "error",
                ErrorPayload {
                    message: format!("Label failed: {}", e),
                    context: "label".to_string(),
                },
            );
            Ok(())
        }
    }
}

/// Attach a free-text description to the current clip: a `described` history
/// event and, when `write_file_properties` is on, System.Comment mirrored onto
/// the file. No rename — description lives in metadata only.
#[tauri::command]
pub async fn overlay_describe(
    app: AppHandle,
    state: State<'_, AppState>,
    text: String,
) -> Result<(), String> {
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err("Description cannot be empty".to_string());
    }
    let (acting, game, write_props, config_path, entry) = {
        let mut s = state.lock().map_err(|e| e.to_string())?;
        let acting = acting_clip(&s)?;
        let game = s.clip_games.get(&acting).cloned();
        let write_props = s.config.write_file_properties;
        let config_path = s.config_path.clone();
        let filename = acting
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        let entry = s.logger.log_with_path(
            LogLevel::Success,
            format!("Description saved: {}", filename),
            LogCategory::System,
            Some(acting.to_string_lossy().to_string()),
        );
        (acting, game, write_props, config_path, entry)
    };
    let _ = app.emit("log-entry", &entry);

    let mut ev =
        crate::history::HistoryEvent::new("described", &acting, "overlay").with_description(&text);
    if let Some(g) = &game {
        ev = ev.with_game(g);
    }

    let state_for_props = state.inner().clone();
    let app_for_props = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        crate::history::append(&crate::history::history_path(&config_path), &ev);
        if write_props {
            write_prop_resolving(
                &app_for_props,
                &state_for_props,
                &acting,
                crate::props::PropValue::Description(text),
            );
        }
    });
    Ok(())
}

/// Set/correct the current clip's game from the overlay. Reuses
/// `edit_game_core` (the History panel's set-game logic) on the acting path +
/// the clip's exe from `clip_exes`, so a remembered override is keyed to the
/// right exe. Tagged source "overlay".
#[tauri::command]
pub async fn overlay_set_game(
    app: AppHandle,
    state: State<'_, AppState>,
    game: String,
    remember: bool,
) -> Result<(), String> {
    let (path, exe) = {
        let s = state.lock().map_err(|e| e.to_string())?;
        let acting = acting_clip(&s)?;
        let exe = s.clip_exes.get(&acting).cloned();
        (acting.to_string_lossy().to_string(), exe)
    };
    crate::commands::edit_game_core(&app, state.inner(), path, game, exe, remember, "overlay").await
}

/// Toggle the count-up stopwatch — same action as the count-up hotkey.
#[tauri::command]
pub async fn overlay_timer_toggle(channels: State<'_, ChannelState>) -> Result<(), String> {
    let tx = channels.count_up_tx.clone();
    tx.send(CountUpCommand::Toggle)
        .await
        .map_err(|e| format!("Failed to send count-up toggle: {}", e))
}

/// The user picked "custom label" while typing is disabled — surface a visible
/// reminder in the event log so they know to enable typing or pick a preset.
/// No history vocabulary; purely a log nudge.
#[tauri::command]
pub fn overlay_needs_label(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let acting = acting_clip(&s)?;
    let filename = acting
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();
    let entry = s.logger.log_with_path(
        LogLevel::Warning,
        format!("Clip needs a label: {}", filename),
        LogCategory::System,
        Some(acting.to_string_lossy().to_string()),
    );
    let _ = app.emit("log-entry", &entry);
    Ok(())
}

/// Combine identity resolution with the acting-path fallback for the overlay's
/// rate/describe property mirror. The identity chain (clip_games/undo_stack)
/// stays PRIMARY — it follows a clip sorted mid-write. But a clip with no
/// detected game never enters clip_games, so identity resolution returns None
/// even though the file sits untouched at the acting path; stars/description
/// must still mirror onto it. Fall back to the acting path only when it still
/// exists on disk (a moved-but-identity-less clip stays a skip — never guess).
fn with_acting_fallback(resolved: Option<PathBuf>, acting: &std::path::Path) -> Option<PathBuf> {
    resolved.or_else(|| acting.exists().then(|| acting.to_path_buf()))
}

/// Shared property-mirror helper for rate/describe: probe-then-write with the
/// identity-resolving closure (follows a clip sorted mid-write, never guesses
/// the newest clip), surfacing a skip warning to the UI exactly like the
/// create path. Blocking — call only from the blocking pool.
fn write_prop_resolving(
    app: &AppHandle,
    state: &AppState,
    acting: &std::path::Path,
    value: crate::props::PropValue,
) {
    let acting = acting.to_path_buf();
    let resolve = || -> Option<PathBuf> {
        let resolved = {
            let s = state.lock().ok()?;
            crate::state::resolve_clip_current_path(&s.clip_games, &s.undo_stack, &acting)
        };
        // Lock dropped before the fallback's exists() disk probe.
        with_acting_fallback(resolved, &acting)
    };
    if let Err(msg) = crate::props::write_with_retry_resolving(
        resolve,
        &[value],
        crate::props::PROBE_ATTEMPTS,
        std::time::Duration::from_millis(crate::props::PROBE_DELAY_MS),
    ) {
        eprintln!("props: {}", msg);
        if let Ok(mut s) = state.lock() {
            let entry = s.logger.log(
                LogLevel::Warning,
                format!("Property write skipped: {}", msg),
                LogCategory::System,
            );
            let _ = app.emit("log-entry", &entry);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_with_acting_fallback_identity_wins() {
        // Identity resolution succeeded — its answer is used even if the
        // acting path also exists (the chain follows a mid-write sort).
        let dir = tempfile::tempdir().unwrap();
        let acting = dir.path().join("acting.mp4");
        std::fs::write(&acting, b"stub").unwrap();
        let resolved = PathBuf::from("C:/clips/sorted/acting !!.mp4");
        assert_eq!(
            with_acting_fallback(Some(resolved.clone()), &acting),
            Some(resolved)
        );
    }

    #[test]
    fn test_with_acting_fallback_identityless_existing_clip() {
        // No identity (game never detected) but the file is still there —
        // rate/describe must mirror onto the acting path.
        let dir = tempfile::tempdir().unwrap();
        let acting = dir.path().join("no-game.mp4");
        std::fs::write(&acting, b"stub").unwrap();
        assert_eq!(with_acting_fallback(None, &acting), Some(acting));
    }

    #[test]
    fn test_with_acting_fallback_missing_file_stays_skip() {
        // No identity AND the acting path is gone (clip moved away without a
        // chain) — skip, never guess another file.
        assert_eq!(
            with_acting_fallback(None, std::path::Path::new("C:/nope/gone.mp4")),
            None
        );
    }
}
