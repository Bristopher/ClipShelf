# Future Feature Ideas (parking lot)

Bigger features surfaced by the 2026-07-05 audit that we deliberately deferred.
Each is worth its own small design pass before building.

## Drag-and-drop a clip onto the bar (L)
Tauri supports file-drop events. Dropping a video file onto a G-key button
sorts it to that folder; dropping onto the log area opens the rename dialog
for it. Natural manual fallback when the watcher missed a file or for sorting
old clips. Needs: file-drop handler, hover highlight on the G-key buttons,
routing through the same collision-safe `mover` path (set it as
`current_file` or add a `move_specific_file` command).

## Per-G-key stats + recent-clips flyout (L)
Show a running count per key ("G1 · 12 today") and a small flyout listing the
last N clips sorted to each folder (name + click-to-reveal, maybe thumbnails
via ffmpeg or the Windows Shell thumbnail cache). Answers "did that sort land
where I think it did?" without reading the log. Needs: per-key counters in
AppState (persisted daily or session-only), a flyout component.

## Rename templates / tokens (M)
The rename dialog appends free text. Power version: templates with tokens
like `{game} {date} {n}`, plus a most-recently-used names list (one keypress
to re-apply "clutch"). Needs: template parsing in `mover::rename_file_with_text`
or a frontend-side expansion, MRU list persisted in config.

## Log search / filter / copy-path (M)
A filter box over the event log (text + level/category chips) and a
right-click context menu on entries: Copy path, Copy filename, Reveal, Play.
Becomes valuable once sessions get long; pairs with the 500-entry cap.

## Richer empty state + OBS step in first-run (M)
Empty log currently says "Waiting for events...". Could show: watched folder
path, watcher/OBS status lines, "press <save_clip_bind> in your game to test".
First-run wizard has no OBS WebSocket step even though it's a core
integration — add an optional page with password + live test-connection.

## In-app keyboard shortcuts on the main window (S-M)
The main bar has no local (non-global) shortcuts: e.g. Del = Wipe,
Ctrl+Z-in-window = Undo, Ctrl+, = Settings. Cheap muscle-memory wins, just
needs a keydown handler + a small help tooltip listing them.

## Watcher restart button / diagnostics panel (S)
`restart_watcher` command exists with no UI. A small diagnostics popover
(watcher status, restart count, last error, OBS status, config path) with a
Restart button would make support/debugging self-serve.
