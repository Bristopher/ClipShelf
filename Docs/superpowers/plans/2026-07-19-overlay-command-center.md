# Overlay Command Center Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expand the Shift+F1 overlay into a command center: act on any of today's clips via a history rolodex, undo in-game, and control the app (watch pause, main window, timers).

**Architecture:** Backend adds a `overlay_target` override consumed by the existing `acting_clip()` funnel, an `overlay_history` query over history.jsonl, arrow keys to the ephemeral overlay hotkey set, and a no-activate main-window show. Frontend grows the root menu to 9 rows with three new submenus (History rolodex, App controls, Timer) reusing the existing `Menu`/`selectDigit`/Flash machinery.

**Tech Stack:** Rust (Tauri v2, windows-sys), React + TS, vitest, cargo test.

**Spec:** `Docs/specs/2026-07-19-overlay-command-center-design.md`

## Global Constraints

- Overlay bind unchanged; digits select, Esc closes, `0` = back (existing conventions).
- All new overlay keys are EPHEMERAL (registered only while the overlay is visible); WASD is never registered.
- "Open ClipShelf window" must NOT activate/steal focus (no `set_focus`, SWP_NOACTIVATE-style show).
- History rolodex: max 7 visible rows, digits pick visible rows, Up/Down scroll, dots when clipped, `exists:false` rows dimmed/unselectable, cap 30 clips, today = `day_rollover_hour` bucketing.
- Sorting a targeted (non-latest) clip must NOT go through `do_press_gkey`; it reuses the drop-files move core (undo push + history event + log + sound), source `"overlay"`.
- Errors flash in the overlay (existing Flash strip); the overlay never closes on error.
- Existing 111 cargo tests must stay green; `pnpm build` clean; commit per task.

---

### Task 1: Backend target-clip model

**Files:**
- Modify: `src-tauri/src/state.rs` (AppStateInner: add field + Default/new init)
- Modify: `src-tauri/src/overlay.rs` (`acting_clip`, `hide`, `overlay_get_context`, new commands)
- Modify: `src-tauri/src/lib.rs` (register commands)

**Interfaces:**
- Produces: `overlay_set_target(path: String)`, `overlay_clear_target()` Tauri commands; `OverlayContext` gains `from_history: bool` and `target_time: Option<String>`; `AppStateInner.overlay_target: Option<PathBuf>`; pure `resolve_acting(target: Option<(PathBuf, bool)>, current: Option<PathBuf>) -> (Option<PathBuf>, bool)` where bool-in = target exists on disk, bool-out = target was dropped (caller clears the field).

- [ ] **Step 1: failing tests** in `overlay.rs` `#[cfg(test)]`:

```rust
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
```

- [ ] **Step 2:** `cargo test resolve_acting` → FAIL (fn missing)
- [ ] **Step 3:** implement:
  - `state.rs`: `pub overlay_target: Option<PathBuf>,` init `None` in `AppStateInner::new`.
  - `overlay.rs`:

```rust
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
```

  - Rework `acting_clip(s)`: build `target = s.overlay_target.clone().map(|t| { let e = t.exists(); (t, e) })` (NOTE: `acting_clip` takes `&AppStateInner`; the exists() probe is a cheap metadata hit, acceptable under the lock as all overlay commands already lock briefly), `current = s.current_file.as_ref().map(|cf| cf.moved_path.as_ref().unwrap_or(&cf.path).clone())`, call `resolve_acting`; a `true` drop flag can't mutate through `&` — so make `acting_clip` take `&mut AppStateInner`, clear `s.overlay_target` on drop, and update the two `&s` call sites (`overlay_get_context`, `overlay_set_game`, `overlay_needs_label`, rate/describe blocks) to lock mutably.
  - Commands:

```rust
#[tauri::command]
pub fn overlay_set_target(state: State<'_, AppState>, path: String) -> Result<(), String> {
    let p = PathBuf::from(&path);
    if !p.exists() { return Err("Clip no longer exists".into()); }
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.overlay_target = Some(p);
    Ok(())
}
#[tauri::command]
pub fn overlay_clear_target(state: State<'_, AppState>) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.overlay_target = None;
    Ok(())
}
```

  - `hide(app)`: after hiding the window, clear the target: lock state via `app.state::<AppState>()`, `s.overlay_target = None`.
  - `overlay_get_context`: add `pub from_history: bool` + `pub target_time: Option<String>` to `OverlayContext`; `from_history = s.overlay_target.is_some()` (after acting_clip ran); `target_time`: find the newest history event for the acting path via `crate::history::read_all(&history_path(&s.config_path))` filtered by `path == acting` → format its `ts` as `%I:%M %p` (chrono parse; None on parse failure). Keep the read OUTSIDE the lock (clone config_path first).
  - Register both commands in `lib.rs` next to `overlay::hide_overlay`.
- [ ] **Step 4:** `cargo test` → all green (111 + 3).
- [ ] **Step 5:** commit `feat(overlay): target-clip model — act on any clip, not just the latest`

### Task 2: Backend history query + targeted sort

**Files:**
- Modify: `src-tauri/src/overlay.rs` (new `overlay_history`, rework `overlay_sort`)
- Modify: `src-tauri/src/commands.rs` (expose the drop move core if private)
- Modify: `src-tauri/src/lib.rs` (register `overlay_history`)

**Interfaces:**
- Consumes: `crate::commands::get_history`-style payload derivation (`HistoryEntryPayload` has `day`, `filename`, clip identity, `path`).
- Produces: `overlay_history() -> Vec<OverlayHistoryRow>` where `OverlayHistoryRow { filename: String, path: String, game: Option<String>, time: String, exists: bool }` (camelCase serialize); `overlay_sort` that routes targeted clips through the drop move core.

- [ ] **Step 1:** failing test for the pure reducer:

```rust
#[test]
fn test_overlay_history_rows_dedupe_to_latest_event_per_clip() {
    // Build three HistoryEntryPayload-shaped events: clip A created then
    // labeled (new path), clip B created. Expect 2 rows, newest first,
    // clip A under its LABELED (latest) path/filename.
}
```

Implement as `fn history_rows(events: &[HistoryEntryPayload], today: &str, cap: usize) -> Vec<OverlayHistoryRow>` — filter `day == today`, group by the payload's clip-identity field, take each clip's latest event (path/filename/game), newest-first by ts, truncate to `cap`; `exists` filled by the caller (`Path::new(&row.path).exists()`), so the reducer stays pure/testable.
- [ ] **Step 2:** run → FAIL; **Step 3:** implement reducer + command:

```rust
#[tauri::command]
pub async fn overlay_history(state: State<'_, AppState>) -> Result<Vec<OverlayHistoryRow>, String> {
    // Reuse the exact payload pipeline get_history uses (same rollover
    // bucketing), then reduce to distinct clips of the CURRENT logical day.
}
```

`today` = `crate::stats::logical_today(rollover_hour)`. `time` = event `ts` → `%I:%M %p`.
- [ ] **Step 4:** rework `overlay_sort`: read `s.overlay_target` first — `None` → existing `do_press_gkey` path unchanged; `Some(t)` → call the drop core with `vec![t]` + key (make `commands::do_drop_files_to_gkey` a `pub(crate) fn` if it's currently locked inside the command). Success flash text comes from the existing command result. History event source stays `"overlay"` (pass through if the core takes a source; if it hardcodes `"drop"`, add a `source: &str` param and update the two call sites).
- [ ] **Step 5:** `cargo test` green; commit `feat(overlay): today's-clips history query + targeted sort via drop core`

### Task 3: Arrow keys + no-activate main-window show + timer reset

**Files:**
- Modify: `src-tauri/src/hotkeys.rs` (`overlay_temp_bindings`, dispatch)
- Modify: `src-tauri/src/timer.rs` (`CountUpCommand::Reset`)
- Modify: `src-tauri/src/overlay.rs` (`overlay_timer_reset`), `src-tauri/src/commands.rs` (`show_main_window_noactivate`), `src-tauri/src/lib.rs` (register)

**Interfaces:**
- Produces: `overlay-key` event now also fires `11` (Up) / `12` (Down); commands `overlay_timer_reset()`, `show_main_window_noactivate()`.

- [ ] **Step 1:** update `test_overlay_temp_bindings_shape` expectation (13 entries: digits 1-9, 0, escape, up, down) → FAIL.
- [ ] **Step 2:** `overlay_temp_bindings` appends `(OverlayKey(11), "up")` and `(OverlayKey(12), "down")`; confirm `parse_hotkey` knows "up"/"down" (it parses the overlay/G-key bind vocabulary — if missing, add VK_UP 0x26 / VK_DOWN 0x28 to its key map with tests). The existing OverlayKey dispatch already forwards the numeric payload to the webview as `overlay-key` — verify range guards don't drop 11/12.
- [ ] **Step 3:** `CountUpCommand::Reset` variant: stops the ticker if running and emits one tick payload with `seconds: 0, running: false` (mirror the Toggle-stop arm). `overlay_timer_reset` command sends it (clone of `overlay_timer_toggle` with `Reset`).
- [ ] **Step 4:** `show_main_window_noactivate`: get main window; `ShowWindow(hwnd, SW_SHOWNOACTIVATE)` via windows-sys (`Win32::UI::WindowsAndMessaging::{ShowWindow, SW_SHOWNOACTIVATE}`) + `window.unminimize()` guarded the same way; never `set_focus`.
- [ ] **Step 5:** `cargo test` green; commit `feat(overlay): arrow overlay keys, count-up reset, no-activate main show`

### Task 4: Frontend rolodex viewport helper

**Files:**
- Create: `src/lib/overlayViewport.ts`
- Test: `src/lib/overlayViewport.test.ts`

**Interfaces:**
- Produces: `overlayViewport(nRows: number, selected: number, offset: number, visible?: number): { offset: number; dotsAbove: boolean; dotsBelow: boolean }` (default `visible = 7`) — MicGuard `mixer_viewport` port.

- [ ] **Step 1: failing tests:**

```ts
import { overlayViewport } from "./overlayViewport";
test("no scroll when rows fit", () => {
  expect(overlayViewport(5, 2, 0)).toEqual({ offset: 0, dotsAbove: false, dotsBelow: false });
});
test("clamps offset and follows selection down", () => {
  expect(overlayViewport(10, 9, 0)).toEqual({ offset: 3, dotsAbove: true, dotsBelow: false });
});
test("follows selection up", () => {
  expect(overlayViewport(10, 1, 3)).toEqual({ offset: 1, dotsAbove: true, dotsBelow: true });
});
```

- [ ] **Step 2:** `pnpm test overlayViewport` → FAIL
- [ ] **Step 3:**

```ts
export function overlayViewport(nRows: number, selected: number, offset: number, visible = 7) {
  if (nRows <= visible) return { offset: 0, dotsAbove: false, dotsBelow: false };
  let off = Math.max(0, Math.min(offset, nRows - visible));
  if (selected < off) off = selected;
  else if (selected >= off + visible) off = selected - visible + 1;
  return { offset: off, dotsAbove: off > 0, dotsBelow: off + visible < nRows };
}
```

- [ ] **Step 4:** tests PASS; **Step 5:** commit `feat(overlay): rolodex viewport helper`

### Task 5: Frontend — root menu, Undo, App submenu, Timer submenu

**Files:**
- Modify: `src/OverlayApp.tsx`, `src/lib/commands.ts`, `src/types/index.ts`

**Interfaces:**
- Consumes: Task 1-3 commands. New wrappers in `commands.ts`: `overlaySetTarget(path)`, `overlayClearTarget()`, `overlayHistory()`, `overlayTimerReset()`, `showMainWindowNoactivate()`; `OverlayContext` type gains `fromHistory: boolean; targetTime?: string`.
- Produces: `Menu` union gains `"history" | "app" | "timer"`.

- [ ] **Step 1:** root rows become: existing 1-5, `6 Timer` → `setMenu("timer")`, `7 History` (Task 6 wires the list; this task renders the submenu shell + fetch), `8 Undo`, `9 App`. `selectDigit` root arm maps 7/8/9 accordingly.
- [ ] **Step 2:** Undo (8): `undoLastAction().then(() => flash success "Undid last action").catch(e => flash error)` — overlay stays open, context refetched (a move may have changed the acting clip's name).
- [ ] **Step 3:** App submenu rows (digits act in `menu === "app"` arm):
  `1` Pause/Resume — `getDiagnostics()` on submenu entry for live watcher status; toggle via `setWatchPaused(!paused)`; flash + relabel.
  `2` Open ClipShelf window — `showMainWindowNoactivate()`, flash "ClipShelf window shown".
  `3` Hide to tray — reuse existing window-hide command used by the tray/titlebar (`getCurrentWindow` is the OVERLAY window here — must call a backend command that hides MAIN, add `hide_main_window` alongside `show_main_window_noactivate` if none exists).
  `4` Wipe current clip — `wipeLog()` + flash.
  `5` Count-up start/stop — `overlayTimerToggle()`; row label reflects running state from the count-up tick listener (Step 5).
  `0` Back.
- [ ] **Step 4:** Timer submenu (`menu === "timer"`): `1 Start/Stop` → `overlayTimerToggle()`, `2 Reset` → `overlayTimerReset()`, `0 Back`.
- [ ] **Step 5:** live stopwatch header: listen to the count-up tick event (find exact name in `timer.rs` `tick_event`; it's the same event the main window's TimerDisplay consumes) while the overlay is visible; when `running`, header shows `⏱ mm:ss`.
- [ ] **Step 6:** `pnpm build` clean; manual dev check (`show_overlay` dev command); commit `feat(overlay): 9-row root, undo row, app + timer submenus, live stopwatch header`

### Task 6: Frontend — history rolodex + target header

**Files:**
- Modify: `src/OverlayApp.tsx`

**Interfaces:**
- Consumes: `overlayHistory()` rows, `overlaySetTarget`, `overlayClearTarget`, `overlayViewport`, `overlay-key` values 11/12.

- [ ] **Step 1:** `menu === "history"` state: `rows`, `sel`, `off` (reset + `overlayHistory()` fetch on entry). Render: header "Today's clips"; `overlayViewport(rows.length, sel, off)` window; dots rows (`▲ …` / `▼ …`) when clipped; each visible row `MenuRow` numbered 1-7 with `label={filename}` + `hint={game ?? time}`; `exists:false` rows `disabled`.
- [ ] **Step 2:** key handling in the history arm: digits 1-7 → pick visible row `off + (d-1)` if in range and exists; `overlay-key` 11/12 → move `sel` up/down (clamp), recompute `off` via `overlayViewport`; `0`/Esc → root. Empty list → single disabled row "No clips yet today".
- [ ] **Step 3:** picking a row: `overlaySetTarget(row.path)` → refetch context → `setMenu("root")`. Header (root, while `ctx.fromHistory`): `▸ {filename}` + line `from history · {game} · {targetTime}` + extra `MenuRow n="L" label="Back to latest clip"` → `overlayClearTarget()` + refetch. Sort/Rate/Label/Describe/Game rows now operate on the target via backend.
- [ ] **Step 4:** vanished-target flash: if a context refetch flips `fromHistory` false unexpectedly (backend dropped it), flash warn "Clip no longer exists — back to latest".
- [ ] **Step 5:** `pnpm build` + `pnpm test` green; commit `feat(overlay): history rolodex with target-clip selection`

### Task 7: Docs + verification + release hygiene

**Files:**
- Modify: `Docs/Verify/2026-07-10-master-verification-checklist.md` (new §23)
- Modify: `Docs/Features/` overlay feature doc if present (else note in spec), `README.md` overlay key table (7/8/9 rows)

- [ ] **Step 1:** §23 "Overlay command center": ship date, commit range, automated coverage (test counts), human items — history pick → re-rate an older clip; targeted sort moves the right file with undo; arrows scroll past 7 rows; dimmed missing clip unselectable; Undo row works; App submenu pause/show-no-activate (fullscreen game keeps focus)/hide/wipe/count-up; timer reset; vanished-target fallback flash; Esc/0 conventions still hold.
- [ ] **Step 2:** README overlay table gains rows 7 History / 8 Undo / 9 App.
- [ ] **Step 3:** full `cargo test` + `pnpm build` + `pnpm test`; commit `docs: overlay command center verification items + README key table`
