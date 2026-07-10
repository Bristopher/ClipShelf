# Stability Round 2 + Visible Error Surface + UX Wins

**Date:** 2026-07-05
**Status:** Implemented

Second audit pass after the 2026-07-02 batch. Three groups: real stability
bugs, a user-visible error surface (toasts), and quick UX wins that mostly
expose already-built backend state. Bigger ideas were parked in
`Docs/future/feature-ideas.md`.

## A. Stability bugs

1. **Tray folder items read stale config** — `tray.rs` called
   `AppConfig::load()` (legacy exe-adjacent path) instead of the managed
   `%APPDATA%` config. Fixed: `live_videos_folder()` reads from the managed
   `AppState`. Tray tooltip version also un-hardcoded via
   `env!("CARGO_PKG_VERSION")`.
2. **Non-atomic saves** — `config.rs::save_to` and `window_layout::save`
   used truncate-then-write; a crash mid-write corrupts the TOML and the
   next launch silently falls back to defaults (total settings loss). Fixed:
   write `*.toml.tmp` then `fs::rename` over the target (atomic on NTFS).
3. **`merge_partial` swallowed invalid updates** — a bad-typed field made the
   merge silently no-op while `update_config` still reported success. Now
   returns `Result` and `update_config` errors out.
4. **Dedup race** — `handle_file_created` checked the last-created path under
   one lock and recorded it under a later one; the OBS-WS and watcher tasks
   could both pass the check for the same clip (double log/sound/timer).
   Fixed: check-and-mark in a single critical section.
5. **Silent hotkey registration failures** — `apply_bindings` now returns the
   failed binds; the listener thread ships them over a channel and lib.rs
   logs + emits an `error` event ("Hotkey 'X' for G1 could not be registered:
   already in use…").
6. **Unbounded memory** — `AppLogger` history/display capped at 5000 entries
   (oldest evicted; daily file log keeps the full record); frontend
   `useEventLog` capped at 500.
7. **Hardening** — timer commands no longer `expect()` on a poisoned lock;
   `rename_file` / `undo_last_action` commands run on `spawn_blocking` (they
   sleep-retry up to ~1.7s); `update_config` writes the config file outside
   the state lock; OBS auth errors also emit the `error` event.

## B. Error surface (toasts)

Previously: `unhandledrejection` replaced the whole window with the fatal
error page, most action buttons had no `.catch`, and nothing listened to the
backend `error` event — failures were either catastrophic-looking or
invisible.

- `src/lib/toast.ts` — module-level store (no lib): `toast/toastError/
  toastInfo/toastSuccess`, duplicate-collapsing, max 4 visible,
  `errorMessage()` normalizer. Callable from non-React code.
- `src/components/Toaster.tsx` — bottom-right stack; mounted in App
  (with `listenBackendErrors` → subscribes to the backend `error` event),
  SettingsApp, FirstRunApp.
- `main.tsx`: `unhandledrejection` → toast (fatal page now reserved for real
  JS `error` events).
- `.catch(toastError)` added to: Sidebar G-key presses, RenameDialog submit,
  BottomBar (wipe/restore/undo/pause/count-up), EventLog reveal/play,
  FirstRunApp finish, SettingsApp save (replaces `alert()`).

## C. UX wins

- **Status truth on mount**: new `last_watcher_status` / `last_obs_status`
  in AppState + `get_watcher_status` / `get_obs_status` commands; both hooks
  fetch on mount (status events usually fire before the webview loads, so
  the old hooks guessed wrong until the next event).
- **OBS visibility**: status pill in Settings' OBS section; compact OBS dot
  in the BottomBar (only when the integration is enabled).
- **Watcher visibility**: BottomBar now distinguishes Watching / Paused
  (amber) / Stopped (red, click restarts); tooltip surfaces restart count.
- **Sidebar**: G-key tags come from the configured folder names (were
  hardcoded "!!"/"CHKD"/"!!!"), with destination tooltips and aria-labels.
- **Rename dialog**: live "→ resulting-filename" preview (backend appends
  " - text"), illegal-character validation, disabled submit when invalid.
- **Settings**: Ctrl+S saves; keybind conflict detection across all seven
  bind fields with inline "same key as X" warnings; custom sound file
  pickers for `clip_save_sound_custom` / `error_sound_custom` (fields
  existed, UI didn't).
- **EventLog**: auto-scroll only when already near the bottom; `success`
  level gets a color.
- **Dead code**: unreachable user-timer frontend wiring removed (backend
  actor kept).
