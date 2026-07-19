# Overlay Command Center — Design

**Date:** 2026-07-19 · **Status:** Approved
**Builds on:** `Docs/specs/2026-07-12-game-detection-history-overlay-design.md`

Expand the existing Shift+F1 in-game overlay from "tag the latest clip" into
a full command center: act on any of today's clips (MicGuard-mixer-style
history rolodex), undo from in-game, and control the app (watch pause, main
window, timers) — all without leaving a fullscreen game. Inspiration:
MicGuard's Shift+F3 mixer popup (rolodex viewport, digit+arrow navigation,
ephemeral hotkeys, no-activate show).

## Root menu (bind unchanged)

Grows 6 → 9 rows. Existing conventions keep working: digits select, Esc
closes, `0` = back inside submenus, mouse works, keystrokes never reach the
game.

| # | Row | Behavior |
|---|-----|----------|
| 1 | Sort | unchanged (G1/G2/G3 folders) |
| 2 | Rate | unchanged |
| 3 | Label | unchanged |
| 4 | Description | unchanged |
| 5 | Game | unchanged |
| 6 | Timer | NOW a submenu: `1 Start/Stop · 2 Reset · 0 Back` |
| 7 | History | NEW — rolodex of today's clips |
| 8 | Undo | NEW — instant, flashes result, stays open |
| 9 | App | NEW — app-control submenu |

## Target-clip model

- `AppStateInner.overlay_target: Option<PathBuf>` (not persisted).
- `acting_clip()` prefers the target; if the target file no longer exists it
  clears the field and falls back to `current_file` (pure decision helper +
  unit test).
- New commands `overlay_set_target(path)` / `overlay_clear_target`. The
  target clears on every overlay hide (`hide()` in overlay.rs).
- `overlay_get_context` gains `from_history: bool` + `target_time:
  Option<String>` so the header can render
  `▸ name (from history · game · 9:12 PM)` and a "latest" reset affordance.
- Rate / Label / Describe / Game already act through `acting_clip` → they
  work on targeted clips with no further changes.
- **Sort on a targeted clip** must NOT use `do_press_gkey` (that moves
  `current_file`). It routes through the same move core as
  `drop_files_to_gkey` (collision-safe move, undo push, history event,
  log, sound), event source `"overlay"`.

## History submenu (7)

- New command `overlay_history()` → today's clips (respecting
  `day_rollover_hour`, same bucketing as the History view), newest first,
  capped at 30: `{ filename, path, game, time, exists }`. Derived from
  history.jsonl events + on-disk existence check.
- Rolodex viewport: max 7 visible rows; digits 1–7 pick a VISIBLE row;
  Up/Down arrows move the selection and scroll; dot indicators above/below
  when clipped (port of MicGuard's `mixer_viewport`, pure + unit-tested).
- Up/Down are new ephemeral overlay keys — registered only while the
  overlay is visible, exactly like the existing digit keys. WASD is never
  registered.
- Rows with `exists: false` render dimmed and are unselectable.
- Picking a row: set target → jump to root with the target header shown.

## Undo row (8)

Calls the existing `undo_last_action` path. Flash shows the outcome (e.g.
"Undid move: Ace clutch.mp4" / error text). Overlay stays open.

## App submenu (9)

| # | Row | Behavior |
|---|-----|----------|
| 1 | Pause/Resume watching | live label from watcher state; toggles `set_watch_paused` |
| 2 | Open ClipShelf window | shows the main window WITHOUT activation (SWP_NOACTIVATE-style show — game keeps focus) |
| 3 | Hide to tray | hides the main window |
| 4 | Wipe current clip | existing `wipe_log` semantics |
| 5 | Count-up timer | start/stop `toggle_count_up`, live state label |

Every action flashes confirmation and keeps the overlay open. `0` back.

## Stopwatch upgrade (6)

- Timer row becomes a submenu: `1 Start/Stop`, `2 Reset` (new
  `overlay_timer_reset` command), `0 Back`.
- While the stopwatch runs, the overlay header shows the live elapsed time
  (1 s interval in the overlay window only while visible; no new polling
  anywhere else).

## Fullscreen-game safety (inherited, unchanged)

`focusable(false)` + WS_EX_NOACTIVATE + foreground watchdog + ephemeral
visible-only hotkeys — already shipped and live-verified. All new keys
(arrows) follow the ephemeral pattern. "Open ClipShelf window" must use a
no-activate show so it can never minimize an exclusive-fullscreen game.

## Error handling

- Every command returns `Result<_, String>`; the overlay flashes errors in
  the existing Flash strip and never closes on error.
- Vanished target → auto-fallback to latest (header updates), flash
  "Clip no longer exists — back to latest".
- Empty history → "No clips yet today" placeholder row.

## Testing

- Pure + unit-tested: rolodex viewport windowing, target/fallback decision,
  history-day bucketing reuse, app-row label selection.
- Frontend `pnpm build` + full `cargo test` green (111 existing tests must
  not regress).

## Out of scope

Multi-day history browsing in the overlay, per-row inline action strips,
reveal/play actions in-game, changing the overlay bind.
