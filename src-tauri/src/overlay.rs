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
        .title("ClipShelf — Overlay")
        .inner_size(420.0, 480.0)
        .resizable(false)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        // Never focusable at the windowing level — WS_EX_NOACTIVATE alone
        // doesn't stop WebView2 from grabbing focus when it becomes visible
        // (which activates the window and minimizes exclusive-fullscreen
        // games). With focusable(false), tao refuses activation entirely.
        .focusable(false)
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
    // Remember who owns the foreground BEFORE the show, so the guard below
    // can hand it back if showing the webview activates us anyway.
    let prev_foreground = current_foreground();
    let _ = window.show();
    guard_foreground(prev_foreground);

    let _ = app.emit("overlay-visible", serde_json::json!({ "visible": true }));
}

/// The current foreground window as a raw handle value (0 if none).
fn current_foreground() -> isize {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    unsafe { GetForegroundWindow() as isize }
}

/// Watchdog for the "overlay show minimized my fullscreen game" failure:
/// WS_EX_NOACTIVATE + focusable(false) should prevent activation, but
/// WebView2 has been observed grabbing focus when its window becomes
/// visible, which yanks the foreground off the game (exclusive-fullscreen
/// games minimize on that). For a short window after show, if the foreground
/// moves to any window of OUR process while the game held it before, hand it
/// straight back. Restores only when the thief is us — a genuine user
/// alt-tab to another app is left alone.
fn guard_foreground(prev: isize) {
    if prev == 0 {
        return;
    }
    std::thread::spawn(move || {
        use windows_sys::Win32::System::Threading::GetCurrentProcessId;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetForegroundWindow, GetWindowThreadProcessId, IsWindow, SetForegroundWindow,
        };
        let own_pid = unsafe { GetCurrentProcessId() };
        // ~300ms of vigilance: WebView2's focus grab lands within the first
        // few frames after the window becomes visible.
        for _ in 0..10 {
            std::thread::sleep(std::time::Duration::from_millis(30));
            unsafe {
                let fg = GetForegroundWindow();
                if fg as isize == prev {
                    continue;
                }
                let mut pid = 0u32;
                GetWindowThreadProcessId(fg, &mut pid);
                if pid == own_pid && IsWindow(prev as _) != 0 {
                    // We stole it — give it back. Allowed because our
                    // process currently owns the foreground.
                    SetForegroundWindow(prev as _);
                }
            }
        }
    });
}

/// Hide the overlay. Emits `overlay-visible` `{visible: false}` app-wide and
/// clears any explicit acting target — the next overlay open goes back to
/// the most recent clip default.
pub fn hide(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.hide();
    }
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut s) = state.lock() {
            s.overlay_target = None;
        }
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
    // If the overlay window doesn't exist there is nothing to reveal — bail
    // before arming the temporary keys or emitting overlay-open, so we never
    // register overlay-only binds against a window that can't handle them.
    if app.get_webview_window(LABEL).is_none() {
        log::warn!("overlay: open called but window is missing — skipping");
        return;
    }

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

/// Pure decision core for `acting_clip`: given an optional explicit target
/// (path + whether it still exists on disk) and the current-clip fallback,
/// decide which path to act on. Returns `(acting, dropped)` — `dropped` is
/// true when a target was set but has vanished from disk, signaling the
/// caller to clear `overlay_target` and fall back to `current`.
fn resolve_acting(
    target: Option<(PathBuf, bool)>,
    current: Option<PathBuf>,
) -> (Option<PathBuf>, bool) {
    match target {
        Some((t, true)) => (Some(t), false),
        Some((_, false)) => (current, true), // vanished → fall back + signal drop
        None => (current, false),
    }
}

/// The clip every overlay action operates on: the explicit `overlay_target`
/// when one is set and still exists on disk, otherwise the most recent
/// clip's CURRENT acting path — `moved_path` if it's already been
/// sorted/renamed this session, else its original `path`. All action
/// commands share this guard so they fail uniformly with `"No recent clip"`
/// when there's nothing to act on. Takes `&mut` because a vanished target
/// clears `overlay_target` as a side effect.
fn acting_clip(s: &mut AppStateInner) -> Result<PathBuf, String> {
    let target = s.overlay_target.clone().map(|t| {
        let exists = t.exists();
        (t, exists)
    });
    let current = s
        .current_file
        .as_ref()
        .map(|cf| cf.moved_path.as_ref().unwrap_or(&cf.path).clone());
    let (acting, dropped) = resolve_acting(target, current);
    if dropped {
        s.overlay_target = None;
    }
    acting.ok_or_else(|| "No recent clip".to_string())
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
    /// True when the overlay is acting on an explicit target (e.g. a clip
    /// picked from history) rather than the most recent clip.
    pub from_history: bool,
    /// The acting clip's most recent history-event time, formatted
    /// "%I:%M %p" (e.g. "3:42 PM"). `None` when no history event was found
    /// for it or its timestamp failed to parse.
    pub target_time: Option<String>,
}

/// Everything the overlay needs to render, from one short lock. Errors with
/// `"No recent clip"` when there's nothing to act on.
#[tauri::command]
pub fn overlay_get_context(state: State<'_, AppState>) -> Result<OverlayContext, String> {
    let (acting, filename, game, exe, from_history, label_presets, description_presets, typing_enabled, binds, config_path) = {
        let mut s = state.lock().map_err(|e| e.to_string())?;
        let acting = acting_clip(&mut s)?;
        let filename = acting
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        let game = s.clip_games.get(&acting).cloned();
        let exe = s.clip_exes.get(&acting).cloned();
        let from_history = s.overlay_target.is_some();
        let c = &s.config;
        let binds = OverlayBinds {
            g1: c.g1_bind.clone(),
            g2: c.g2_bind.clone(),
            g3: c.g3_bind.clone(),
            g1_name: c.g1_bind_folder_name.clone(),
            g2_name: c.g2_bind_folder_name.clone(),
            g3_name: c.g3_bind_folder_name.clone(),
            overlay: c.overlay_bind.clone(),
        };
        (
            acting,
            filename,
            game,
            exe,
            from_history,
            c.label_presets.clone(),
            c.description_presets.clone(),
            c.overlay_typing_enabled,
            binds,
            s.config_path.clone(),
        )
    };

    // History read happens OUTSIDE the lock.
    let acting_str = acting.to_string_lossy().to_string();
    let target_time = crate::history::read_all(&crate::history::history_path(&config_path))
        .into_iter()
        .filter(|ev| ev.path == acting_str)
        .filter_map(|ev| {
            chrono::DateTime::parse_from_rfc3339(&ev.ts)
                .ok()
                .map(|dt| dt.format("%I:%M %p").to_string())
        })
        .last();

    Ok(OverlayContext {
        path: acting.to_string_lossy().to_string(),
        filename,
        game,
        exe,
        label_presets,
        description_presets,
        typing_enabled,
        binds,
        from_history,
        target_time,
    })
}

/// Set the overlay's explicit acting target — e.g. the user picked a clip
/// from history. Rejects a target that no longer exists on disk.
#[tauri::command]
pub fn overlay_set_target(state: State<'_, AppState>, path: String) -> Result<(), String> {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return Err("Clip no longer exists".into());
    }
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.overlay_target = Some(p);
    Ok(())
}

/// Clear the overlay's explicit acting target, returning to the most recent
/// clip default.
#[tauri::command]
pub fn overlay_clear_target(state: State<'_, AppState>) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.overlay_target = None;
    Ok(())
}

/// Sort with a G-key. When no explicit target is set (`overlay_target` is
/// `None`), reuses the exact hotkey move path (`do_press_gkey`) acting on the
/// most recent clip — same collision-safe move, log, sound, and undo push,
/// only tagging the history event source "overlay". When a target IS set
/// (e.g. a clip picked from the overlay's history list), routes that ONE
/// path through the same move core the drag-drop path uses
/// (`commands::move_file_with_key`) instead — dropping onto a G-key must be
/// able to sort a clip other than the most recent one. Sync like
/// `press_gkey`: both paths block on move retries and Tauri runs sync
/// commands on a worker thread.
#[tauri::command]
pub fn overlay_sort(app: AppHandle, state: State<'_, AppState>, key: u8) -> Result<(), String> {
    if !(1..=3).contains(&key) {
        return Err(format!("Invalid gkey: {}. Must be 1, 2, or 3.", key));
    }

    let target = {
        let s = state.lock().map_err(|e| e.to_string())?;
        s.overlay_target.clone()
    };

    match target {
        None => {
            crate::commands::do_press_gkey(&app, state.inner(), key, "overlay");
        }
        Some(t) => {
            let config = {
                let s = state.lock().map_err(|e| e.to_string())?;
                s.config.clone()
            };
            let label = format!("G{} (overlay)", key);
            if let Some(mv) =
                crate::commands::move_file_with_key(&app, state.inner(), &t, key, &label, &config, "overlay")
            {
                let mut s = state.lock().map_err(|e| e.to_string())?;
                s.push_undo(crate::state::UndoEntry { moves: vec![mv] });
            }
        }
    }
    Ok(())
}

/// One row in the overlay's "today's clips" history list — a subset of
/// `HistoryEntryPayload` reduced to one row per distinct clip.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayHistoryRow {
    pub filename: String,
    pub path: String,
    pub game: Option<String>,
    /// The clip's latest event time, formatted "%I:%M %p" (falls back to the
    /// raw `ts` if it fails to parse).
    pub time: String,
    /// Whether the file still exists at `path` — filled by the caller
    /// (`overlay_history`) since a disk probe would make this fn impure.
    pub exists: bool,
}

/// Pure reducer: from the day's reconciled history events, produce one row
/// per distinct clip (`clip_id`) at the CURRENT logical day, showing each
/// clip's latest event (so a clip that was created then labeled shows its
/// labeled path/filename, not its created one). Newest-first by event time,
/// truncated to `cap`. `exists` is always `false` here — the caller fills it
/// in with a disk probe so this stays testable without touching the
/// filesystem.
fn history_rows(
    events: &[crate::events::HistoryEntryPayload],
    today: &str,
    cap: usize,
) -> Vec<OverlayHistoryRow> {
    use std::collections::HashMap;

    // Latest event per clip, among today's events only. `ts` is written by
    // `chrono::Local::now().to_rfc3339_opts(...)` — a consistent offset
    // within a session — so plain string comparison sorts chronologically.
    let mut latest: HashMap<usize, &crate::events::HistoryEntryPayload> = HashMap::new();
    for e in events.iter().filter(|e| e.day == today) {
        latest
            .entry(e.clip_id)
            .and_modify(|cur| {
                if e.ts > cur.ts {
                    *cur = e;
                }
            })
            .or_insert(e);
    }

    let mut rows: Vec<&crate::events::HistoryEntryPayload> = latest.into_values().collect();
    rows.sort_by(|a, b| b.ts.cmp(&a.ts));

    rows.into_iter()
        .take(cap)
        .map(|e| OverlayHistoryRow {
            filename: e.filename.clone(),
            path: e.path.clone(),
            game: e.game.clone(),
            time: chrono::DateTime::parse_from_rfc3339(&e.ts)
                .map(|dt| dt.format("%I:%M %p").to_string())
                .unwrap_or_else(|_| e.ts.clone()),
            exists: false,
        })
        .collect()
}

/// Max rows returned by `overlay_history` — a command-center list, not the
/// full History panel.
const OVERLAY_HISTORY_CAP: usize = 30;

/// Today's distinct clips for the overlay's history list. Reuses the exact
/// payload pipeline `get_history` uses (same rollover-bucketed reconciliation
/// via `commands::history_payloads`), then reduces to one row per clip for
/// the current logical day.
#[tauri::command]
pub async fn overlay_history(state: State<'_, AppState>) -> Result<Vec<OverlayHistoryRow>, String> {
    let (config_path, rollover_hour) = {
        let s = state.lock().map_err(|e| e.to_string())?;
        (s.config_path.clone(), s.config.day_rollover_hour)
    };
    tauri::async_runtime::spawn_blocking(move || {
        let events = crate::history::read_all(&crate::history::history_path(&config_path));
        let today = crate::stats::logical_today(rollover_hour);
        // full=true: history_payloads must not pre-filter by day — the
        // reducer needs the WHOLE reconciliation chain (a clip created
        // yesterday-side of the rollover but labeled today still needs its
        // earlier events to resolve clip identity) even though only today's
        // rows are returned.
        let payloads = crate::commands::history_payloads(events, rollover_hour, true, &today);
        let mut rows = history_rows(&payloads, &today, OVERLAY_HISTORY_CAP);
        for row in &mut rows {
            row.exists = std::path::Path::new(&row.path).exists();
        }
        rows
    })
    .await
    .map_err(|e| e.to_string())
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
        let acting = acting_clip(&mut s)?;
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
        let mut s = state.lock().map_err(|e| e.to_string())?;
        (acting_clip(&mut s)?, s.config.log_file_enabled)
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
            let msg = format!("Label failed: {}", e);
            {
                let mut s = state.lock().map_err(|e| e.to_string())?;
                let entry = s
                    .logger
                    .log(LogLevel::Error, msg.clone(), LogCategory::System);
                let _ = app.emit("log-entry", &entry);
            }
            let _ = app.emit(
                "error",
                ErrorPayload {
                    message: msg.clone(),
                    context: "label".to_string(),
                },
            );
            // Propagate the failure so the overlay's label action rejects and
            // flashes red — previously this returned Ok(()) and told the overlay
            // the label succeeded even though the rename had failed.
            Err(msg)
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
        let acting = acting_clip(&mut s)?;
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
        let mut s = state.lock().map_err(|e| e.to_string())?;
        let acting = acting_clip(&mut s)?;
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
    let acting = acting_clip(&mut s)?;
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
    use crate::events::HistoryEntryPayload;

    fn payload(
        ts: &str,
        path: &str,
        filename: &str,
        game: Option<&str>,
        day: &str,
        clip_id: usize,
    ) -> HistoryEntryPayload {
        HistoryEntryPayload {
            ts: ts.to_string(),
            event: "moved".to_string(),
            path: path.to_string(),
            old_path: None,
            game: game.map(|g| g.to_string()),
            exe: None,
            key: None,
            rating: None,
            label: None,
            description: None,
            source: "hotkey".to_string(),
            day: day.to_string(),
            filename: filename.to_string(),
            clip_id,
        }
    }

    #[test]
    fn test_overlay_history_rows_dedupe_to_latest_event_per_clip() {
        // Clip A: created then labeled (new path/filename) — same clip_id.
        // Clip B: created only. Both events fall on "today".
        let events = vec![
            payload(
                "2026-07-19T10:00:00-04:00",
                "C:/clips/a.mp4",
                "a.mp4",
                Some("Valorant"),
                "2026-07-19",
                0,
            ),
            payload(
                "2026-07-19T10:05:00-04:00",
                "C:/clips/a - clutch.mp4",
                "a - clutch.mp4",
                Some("Valorant"),
                "2026-07-19",
                0,
            ),
            payload(
                "2026-07-19T10:02:00-04:00",
                "C:/clips/b.mp4",
                "b.mp4",
                Some("Apex"),
                "2026-07-19",
                1,
            ),
        ];

        let rows = history_rows(&events, "2026-07-19", 30);

        assert_eq!(rows.len(), 2);
        // Newest first: clip A's labeled event (10:05) before clip B's (10:02).
        assert_eq!(rows[0].filename, "a - clutch.mp4");
        assert_eq!(rows[0].path, "C:/clips/a - clutch.mp4");
        assert_eq!(rows[0].game.as_deref(), Some("Valorant"));
        assert_eq!(rows[1].filename, "b.mp4");
    }

    #[test]
    fn test_overlay_history_rows_filters_to_today_and_caps() {
        let mut events = vec![payload(
            "2026-07-18T09:00:00-04:00",
            "C:/clips/yesterday.mp4",
            "yesterday.mp4",
            None,
            "2026-07-18",
            0,
        )];
        for i in 0..5 {
            events.push(payload(
                &format!("2026-07-19T10:0{}:00-04:00", i),
                &format!("C:/clips/{}.mp4", i),
                &format!("{}.mp4", i),
                None,
                "2026-07-19",
                i + 1,
            ));
        }

        let rows = history_rows(&events, "2026-07-19", 3);
        assert_eq!(rows.len(), 3);
        // Newest first among today's clips.
        assert_eq!(rows[0].filename, "4.mp4");
    }

    #[test]
    fn test_resolve_acting_prefers_existing_target() {
        let t = PathBuf::from("C:/clips/old.mp4");
        let c = PathBuf::from("C:/clips/new.mp4");
        assert_eq!(resolve_acting(Some((t.clone(), true)), Some(c.clone())), (Some(t), false));
    }
    #[test]
    fn test_resolve_acting_falls_back_and_flags_drop_when_target_gone() {
        let t = PathBuf::from("C:/clips/gone.mp4");
        let c = PathBuf::from("C:/clips/new.mp4");
        assert_eq!(resolve_acting(Some((t, false)), Some(c.clone())), (Some(c), true));
    }
    #[test]
    fn test_resolve_acting_no_target_uses_current() {
        let c = PathBuf::from("C:/clips/new.mp4");
        assert_eq!(resolve_acting(None, Some(c.clone())), (Some(c), false));
        assert_eq!(resolve_acting(None, None), (None, false));
    }

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
