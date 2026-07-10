# Feature Batch: Diagnostics, Shortcuts, Drag-Drop, Rename MRU, Log Filter, G-Key Stats, First-Run OBS

**Date:** 2026-07-10
**Status:** Implemented

Implements the parking lot from `Docs/future/feature-ideas.md` (2026-07-05
audit). Ordered by bang-for-buck; each section notes the design decisions.

## 1. Watcher diagnostics panel (S)

Popover in the BottomBar (Activity icon, right side). Fetches a new
`get_diagnostics` command on open (no polling): app version, config path,
videos folder, watcher status + restart count, pause state, OBS
enabled/status. Folder rows are click-to-open. Includes a "Restart watcher"
button (`restart_watcher` command finally gets UI) and the in-app shortcut
list (see §2) as a mini cheat-sheet. Click-outside / Esc closes.

## 2. In-app keyboard shortcuts (S)

Main window only, local (not global) keydown handler in `App.tsx`:

- `Delete` → Wipe log
- `Ctrl+Z` → Undo last move/rename (in-window complement to the global
  `undo_bind`, which users are told to keep rare)
- `Ctrl+,` → open Settings
- `Ctrl+F` → toggle the event-log filter bar (§5)

Suppressed while focus is in an input/textarea/contenteditable (covers the
rename dialog). Listed in the diagnostics popover.

## 3. Drag-and-drop a clip onto the bar (L)

Tauri webview `onDragDropEvent` (HTML5 drag events don't fire while
`dragDropEnabled` is on, so hit-testing uses the event's physical position ÷
`devicePixelRatio` → `document.elementFromPoint` → closest `[data-drop]`).

- Drop on **G1–G3** → new `drop_file_to_gkey(path, key)` command: validates
  `watcher::is_video_file` + existence, then routes through the exact same
  collision-safe move path as a key press (shared `move_file_with_key` free
  fn extracted from `do_press_gkey`), including undo entry, stats, log
  entry, sound, file-moved event. Runs on `spawn_blocking` (retry sleeps).
- Drop on **G4 or the log area** → new `select_dropped_file(path)` command:
  validates, sets it as `current_file`, logs "Selected clip", returns the
  filename; the frontend then opens the rename dialog via the existing
  `hotkey-triggered` event, extended with an optional `filename` payload so
  the dialog doesn't wait for a `file-created` event that never came.
- During drag-over, the hovered G-key button gets a highlight ring and the
  log area an overlay hint. Non-video drops → error toast, no command call.
- Manual fallback for clips the watcher missed, or bulk-sorting old clips
  (one file per drop; first video path in a multi-drop wins, rest ignored
  with a toast note).

## 4. Rename MRU (M, scoped down)

`rename_mru: Vec<String>` in `AppConfig` (serde default empty, cap 8,
case-insensitive dedupe, most recent first — helper + test in config.rs).
Backend appends on successful rename in `do_rename_file`, saves config
outside the lock, emits `config-changed`. The rename dialog shows the MRU as
clickable chips that fill the input (one keypress to re-apply "clutch").

Deliberately NOT doing token templates (`{game} {n}`) this round — parsing
semantics need their own pass; MRU covers the main ask.

Settings-draft interaction: SettingsApp saves its whole draft, which would
clobber an MRU updated while the window sat open. Fix: SettingsApp strips
`rename_mru` from the save payload (update_config is a partial merge, so the
backend value survives).

## 5. Log filter + context menu + richer empty state (M)

- **Filter bar** (Ctrl+F or search icon in the bar): text match
  (case-insensitive) + level chips (all/info/success/warning/error), shows
  "n / m" count, Esc closes. Pure frontend filter over the rendered entries.
- **Context menu** (right-click on entries with a path): Copy path, Copy
  filename, Reveal in Explorer, Play. Clipboard via `navigator.clipboard`
  (WebView2 secure context) with toast on failure. Click-outside/Esc closes.
- **Empty state**: instead of "Waiting for events...", show the watched
  folder, watcher/OBS status lines, and "press <save_clip_bind> in-game"
  hint (falls back to generic wording when no bind is set).

## 6. Per-G-key stats + recent-clips flyout (L, scoped down)

Session-only (resets on launch — daily persistence deferred):
`gkey_stats: HashMap<u8, GKeyStat { count, recent: Vec<RecentClip> }>` in
AppState, recorded in the shared move path (hotkey, button, drop). Recent
capped at 5, newest first. New `get_gkey_stats` command returns all three
keys. Sidebar shows a small count badge on each G-key button and, on hover
(500 ms delay), a flyout listing the recent clips — click reveals in
Explorer. Frontend refetches on `file-moved` events + on mount. No
thumbnails this round (ffmpeg/shell-thumbnail dependency not worth it yet).

## 7. First-run OBS step (M, scoped down)

New optional section in the setup window: enable toggle + password field,
saved with Finish. No live test-connection button — the OBS actor is
config-driven and connects right after save; the main window's OBS dot and
the Settings status pill are the feedback loop. Copy in the section says
exactly that. (A one-shot test would duplicate `connect_and_run`; revisit if
users get confused.)

## Cross-cutting

- New commands registered in lib.rs: `drop_file_to_gkey`,
  `select_dropped_file`, `get_gkey_stats`, `get_diagnostics`.
- `hotkey-triggered` payload extended: `{ key, filename? }` — backward
  compatible (Rust hotkey path still sends just `key`).
- All new frontend actions `.catch(toastError)`.
