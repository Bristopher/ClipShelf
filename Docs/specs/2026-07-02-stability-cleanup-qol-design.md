# Stability, Cleanup & QoL Features — 2026-07-02

Approved scope from audit session: stability pass → cleanup → five QoL features.

## Phase A — Stability

1. **OBS WS always-on actor with infinite reconnect.** `spawn_obs_ws(enabled, password)`
   is spawned unconditionally at startup. Single command `Configure { enabled, password }`
   hot-applies settings changes (sent from `update_config`). Reconnect forever with
   backoff (3s after a clean drop, then 5/10/20/30s cap). Empty password allowed
   (auth-less OBS). Statuses: `disabled | connecting | reconnecting | connected | disconnected`.
2. **Collision-safe file destinations.** `unique_destination()` appends ` (2)`, ` (3)`…
   before the extension when the target exists. Used by move, rename, and undo.
3. **G1–G3 hotkeys handled fully in Rust.** `do_press_gkey()` free function called
   directly from the hotkey handler — no webview round-trip, hotkeys work even if the
   webview is loading/hung. Key 4 (rename) still emits `hotkey-triggered` for the dialog.
   Also fixes Sidebar G4 button which incorrectly called `press_gkey(4)`.
4. **Single instance.** `tauri-plugin-single-instance`; second launch shows/focuses main window.
5. **Sleep detector uses SystemTime** (Instant may not advance during suspend on Windows).
6. **Logger repoints on config change.** `AppLogger::reconfigure()` keeps buffers, updates
   log dir + enabled flag from `update_config`.
7. **`ReplayBufferSaved` injects into the file pipeline.** OBS-reported path goes through
   `handle_file_created` (faster + immune to watcher wedging). Dedup guard at the top of
   `handle_file_created`: same path seen within 5s → skip (covers watcher/OBS double-fire).

## Phase B — Cleanup

- Delete dead config fields `windows_notification_enabled`, `shadowplay_folder`,
  `prompt_capture_software` (Rust + TS) and the unused `tauri-plugin-notification` plugin.
- Drop unused `notify-debouncer-mini` dependency.
- Black-screen warning threshold `6.5` MB → config field `small_file_warn_mb` + settings input.
- Regexes in `parse_time_from_filename` / `insert_tag_in_filename` → `LazyLock`.
- `rename_file_with_text` gets the same locked-file retry as move (shared helper).

## Phase C — Features

1. **Undo last action.** `undo_stack: Vec<UndoEntry{from,to}>` (cap 20) in state, pushed on
   every successful move/rename. `undo_last_action` command reverses the top entry
   (collision-safe + retry), resets `current_file` to the restored path. New configurable
   hotkey `undo_bind` (default empty — user picks something rare, NOT ctrl+z) handled
   Rust-side; Undo button in BottomBar.
2. **Clickable log entries.** `LogEntryPayload.path: Option<String>` set for
   created/moved/renamed/undone entries. Click → `opener::reveal` (Explorer with file
   selected); Ctrl+Click → open in default video player. Custom styled hover tooltip
   (same visual language as TitleBar tooltips) explains both actions.
3. **Pause watching.** Runtime flag `watch_paused` + `set_watch_paused` command: stops/starts
   the watcher, gates OBS injection & health check. Watcher "stopped" status is rewritten to
   "paused" while flag is set. BottomBar toggle + tray CheckMenuItem, kept in sync.
4. **Autostart on login.** `tauri-plugin-autostart`; config `autostart_enabled` (default off),
   synced at startup and on config change. Settings toggle under General.
5. **Window layout memory + default position.** Auto-save pos/size (debounced, physical px)
   to a separate `window_layout.toml` (NOT in AppConfig — avoids the Settings draft model
   clobbering it). Config fields: `remember_window_layout` (default on), `default_monitor`
   (1-based, default 2), `default_anchor` (`top-left | top-right | bottom-left | bottom-right | center`,
   default top-left). Startup: saved layout wins if remember is on; otherwise anchor on the
   default monitor (fallback: last monitor if index too high). `reset_window` applies the
   configured default + clears the saved layout. Settings section with monitor/anchor pickers
   + Reset button.
