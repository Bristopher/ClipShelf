# Phase 2: History Panel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The Restore button becomes History — a panel showing today's clips grouped by detected game (day starts at the configurable rollover hour), with a full-history view, row actions, and Edit game…/Remember; daily G-key stats switch to the same day boundary.

**Architecture:** A pure `logical_date` day-bucketing core in `stats.rs` shared by stats and history filtering. Two new Tauri commands (`get_history`, `edit_history_game`) that read/append `history.jsonl` on the blocking pool. Frontend: a History panel component replacing the Restore button in `BottomBar.tsx`, grouping payloads client-side for display only (all boundary logic stays in Rust).

**Tech Stack:** Rust (Tauri v2), chrono, existing history/config modules from Phase 1; React + TS frontend following the DiagnosticsButton popover idiom.

**Spec:** `Docs/specs/2026-07-12-game-detection-history-overlay-design.md` (Phase 2 section). Contract: `Docs/Features/Clip-Metadata-Interop.md`.

## Global Constraints

- Never do disk IO under the state lock; file reads/writes on the blocking pool.
- History is APPEND-ONLY — `edit_history_game` appends a `game_edited` event; it never rewrites the file.
- Day boundary: "today" starts at `config.day_rollover_hour` (default 4). One implementation, used by BOTH stats and history filtering.
- New JSONL field `exe` is additive-optional; the interop doc must be updated in the same task that adds it.
- Rust: `cargo test` green, `cargo build` ZERO warnings at the end of every task (Phase 1 ended warning-clean; `#[allow(dead_code)]` only with a `// consumed in Phase 2/3 (...)` comment for genuinely later-phase items).
- Frontend: `pnpm exec tsc --noEmit` + `pnpm build` clean; always pnpm.
- Event payloads to the frontend use camelCase serde attrs (see events.rs siblings).
- Test command: `Set-Location '...\src-tauri'; cargo test`.

---

### Task 1: Rollover-aware day bucketing in `stats.rs` + callers

**Files:**
- Modify: `src-tauri/src/stats.rs`
- Modify: `src-tauri/src/state.rs` (AppStateInner::new passes rollover hour)
- Modify: `src-tauri/src/commands.rs` (increment call site passes rollover hour)

**Interfaces:**
- Produces: `pub fn logical_date_of(dt: chrono::DateTime<chrono::Local>, rollover_hour: u8) -> String` (PURE); `pub fn logical_today(rollover_hour: u8) -> String`; `load(path, rollover_hour)`, `DailyStats::increment(key, rollover_hour)` (signature changes — update ALL callers; `today()` is deleted).

- [ ] **Step 1: Failing tests** (replace/extend `stats.rs` tests; keep existing ones updated to the new signatures)

```rust
#[test]
fn test_logical_date_respects_rollover_hour() {
    use chrono::TimeZone;
    let d = |y, mo, d, h, mi| chrono::Local.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap();
    // 23:59 belongs to its own calendar day
    assert_eq!(logical_date_of(d(2026, 7, 12, 23, 59), 4), "2026-07-12");
    // 03:59 still belongs to the PREVIOUS day (late-night session)
    assert_eq!(logical_date_of(d(2026, 7, 13, 3, 59), 4), "2026-07-12");
    // 04:00 exactly starts the new day
    assert_eq!(logical_date_of(d(2026, 7, 13, 4, 0), 4), "2026-07-13");
    // rollover 0 = plain calendar midnight
    assert_eq!(logical_date_of(d(2026, 7, 13, 0, 0), 0), "2026-07-13");
    // hour clamped: 25 treated as 23
    assert_eq!(logical_date_of(d(2026, 7, 13, 22, 59), 25), "2026-07-12");
}
```

- [ ] **Step 2: Run** `cargo test stats` → compile FAIL.

- [ ] **Step 3: Implement**

```rust
/// The "logical" date a timestamp belongs to when the day starts at
/// `rollover_hour` instead of midnight — 3 AM clips count as yesterday for
/// a 4 AM rollover. Pure so it's testable; clamps the hour to 0-23.
pub fn logical_date_of(dt: chrono::DateTime<chrono::Local>, rollover_hour: u8) -> String {
    let shifted = dt - chrono::Duration::hours(rollover_hour.min(23) as i64);
    shifted.format("%Y-%m-%d").to_string()
}

pub fn logical_today(rollover_hour: u8) -> String {
    logical_date_of(chrono::Local::now(), rollover_hour)
}
```

Delete `today()`; thread `rollover_hour: u8` through `load` and `increment` (replace every `today()` call with `logical_today(rollover_hour)`). Update the module doc comment ("Counts roll over at local midnight" → the configurable-hour truth). Callers: `state.rs` `AppStateInner::new` calls `stats::load(&stats_path, config.day_rollover_hour)` (read the hour from the `config` parameter BEFORE moving it into the struct); `commands.rs` `record_gkey_move` path — `increment` is called inside `state.record_gkey_move`; change `record_gkey_move(&mut self, key, dest)` to read `self.config.day_rollover_hour` internally and pass it down. Update existing stats tests to the new signatures (pass `0` where they relied on calendar-midnight behavior... NO — pass a literal `4` and keep using `logical_today(4)` in assertions so tests stay meaningful).

- [ ] **Step 4: Verify** — full `cargo test` green, zero warnings.
- [ ] **Step 5: Commit** — "Make daily stats roll over at the configurable hour instead of midnight"

---

### Task 2: Record exe stem in created events (enables Remember from history)

**Files:**
- Modify: `src-tauri/src/history.rs` (optional `exe` field + `with_exe`)
- Modify: `src-tauri/src/lib.rs` (created events carry snapshot exe; fallback snapshot returns the full GameSnapshot, not just the label)
- Modify: `src-tauri/src/gamedetect.rs` (remove the `#[allow(dead_code)]` from `GameSnapshot.exe_stem` — now consumed)
- Modify: `Docs/Features/Clip-Metadata-Interop.md` (§3: add `exe` to the schema block + a consumer note: optional, present on `created` events when detection ran; additive since 2026-07-12)

**Interfaces:**
- Produces: `HistoryEvent.exe: Option<String>` + `.with_exe(&str)`; `created` events now carry `exe` whenever a snapshot existed.

- [ ] **Step 1: Failing test** (history.rs)

```rust
#[test]
fn test_exe_field_roundtrip_and_omitted_when_none() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("history.jsonl");
    append(&path, &HistoryEvent::new("created", Path::new("C:/a.mp4"), "app")
        .with_game("Counter-Strike 2").with_exe("cs2"));
    append(&path, &HistoryEvent::new("moved", Path::new("C:/b.mp4"), "app"));
    let all = read_all(&path);
    assert_eq!(all[0].exe.as_deref(), Some("cs2"));
    assert!(serde_json::to_string(&all[1]).unwrap().contains("\"exe\"") == false);
}
```

- [ ] **Step 2–3: Implement** — field with `#[serde(skip_serializing_if = "Option::is_none")]` placed after `game`; builder `with_exe`. In `lib.rs` `handle_file_created`: keep the whole `GameSnapshot` (label + exe_stem) instead of mapping to label — the pending-snapshot arm already has it; change the fallback arm to return the snapshot (`gamedetect::snapshot_foreground(&overrides)` directly, no `.map(|s| s.label)`), then use `.label` for the game and `.exe_stem` for `with_exe`. `clip_games` still stores the label only.

- [ ] **Step 4: Verify** — `cargo test` green; **zero warnings** (the exe_stem allow must come OFF and compile clean because it's now read).
- [ ] **Step 5: Commit** — "Record detecting exe stem in created history events for override Remember"

---

### Task 3: Backend commands — `get_history` + `edit_history_game`

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/events.rs` (payload struct)
- Modify: `src-tauri/src/lib.rs` (register both commands)

**Interfaces:**
- Produces:

```rust
// events.rs — camelCase like siblings
#[derive(Debug, Clone, serde::Serialize)]
pub struct HistoryEntryPayload {
    pub ts: String,
    pub event: String,
    pub path: String,
    #[serde(rename = "oldPath")]
    pub old_path: Option<String>,
    pub game: Option<String>,
    pub exe: Option<String>,
    pub key: Option<u8>,
    pub rating: Option<u8>,
    pub label: Option<String>,
    pub description: Option<String>,
    pub source: String,
    /// Logical day bucket ("YYYY-MM-DD") computed with the configured
    /// rollover hour — the frontend groups by this, never re-derives it.
    pub day: String,
    pub filename: String,
}
```

```rust
#[tauri::command]
pub async fn get_history(state: tauri::State<'_, AppState>, full: bool) -> Result<Vec<HistoryEntryPayload>, String>
// newest first; full=false → only entries whose day == logical_today(rollover)
#[tauri::command]
pub async fn edit_history_game(app: tauri::AppHandle, state: tauri::State<'_, AppState>, path: String, game: String, exe: Option<String>, remember: bool) -> Result<(), String>
```

- [ ] **Step 1: Failing test** — the filtering/mapping core must be pure. Extract:

```rust
/// Map + filter history events for the UI: newest first, optionally only
/// the current logical day.
pub(crate) fn history_payloads(
    events: Vec<crate::history::HistoryEvent>,
    rollover_hour: u8,
    full: bool,
    today: &str,
) -> Vec<HistoryEntryPayload>
```

Test (commands.rs tests module): build 3 events with hand-written `ts` strings (`2026-07-12T23:59:00-06:00`, `2026-07-13T03:59:00-06:00`, `2026-07-13T04:00:00-06:00`), rollover 4: all three get `day` of `2026-07-12`, `2026-07-12`, `2026-07-13` respectively (parse with `chrono::DateTime::parse_from_rfc3339` then `.with_timezone(&chrono::Local)`); with `full=false, today="2026-07-12"` only the first two survive; order is newest-first (input order reversed); unparseable `ts` → event kept with `day = ts[..10]` fallback (never dropped silently); `filename` is the path's file_name.

- [ ] **Step 2–3: Implement.** `get_history`: clone `config_path` + `day_rollover_hour` under a short lock; `tauri::async_runtime::spawn_blocking` → `history::read_all(&history_path)` → `history_payloads(...)` with `today = stats::logical_today(hour)`; `.await.map_err(|e| e.to_string())?`. `edit_history_game`: validate `game` non-empty (trim); under one lock: update `clip_games` entry if the path is keyed; if `remember && exe.is_some()` → `config.remember_game_override(&exe, &game)` (this consumes the Phase-2 allow — REMOVE that `#[allow(dead_code)]` + comment), clone config + config_path for saving; log entry "Game set to {game} for {filename}" (Success, LogCategory::System, with path) and emit `log-entry`; drop lock; on the SAME (already async command) thread call `spawn_blocking` for: `history::append(game_edited event with_game + with_exe if present)` and, when remember, `config.save_to` + emit `config-changed` (follow the exact pattern `do_rename_file` uses for config save + emit — read it first). Register both commands in lib.rs `generate_handler`.

- [ ] **Step 4: Verify** — `cargo test` green, zero warnings.
- [ ] **Step 5: Commit** — "Add get_history and edit_history_game commands with rollover-aware day bucketing"

---

### Task 4: Frontend — History panel replaces Restore

**Files:**
- Create: `src/components/HistoryPanel.tsx`
- Modify: `src/components/BottomBar.tsx` (Restore button → History button + panel mount)
- Modify: `src/lib/commands.ts`, `src/types/index.ts`

**Interfaces:**
- Consumes: `getHistory(full: boolean): Promise<HistoryEntry[]>`, `editHistoryGame(path, game, exe | null, remember)`; `HistoryEntry` mirrors HistoryEntryPayload (camelCase: `oldPath`, rest as-is).

- [ ] **Step 1: commands + types**

```ts
export const getHistory = (full: boolean) => invoke<HistoryEntry[]>("get_history", { full });
export const editHistoryGame = (path: string, game: string, exe: string | null, remember: boolean) =>
  invoke<void>("edit_history_game", { path, game, exe, remember });
```

`HistoryEntry`: `{ ts, event, path, oldPath?, game?, exe?, key?, rating?, label?, description?, source, day, filename }` (string/number optionals per payload).

- [ ] **Step 2: HistoryPanel component.** READ `BottomBar.tsx`'s DiagnosticsButton popover first and follow its structure (button + absolutely-positioned panel, click-outside + Esc close, fetch on open) but wider (`w-96`) with `max-h-[70vh] overflow-y-auto`. Content:
  - Header: "History" + segmented Today/All toggle (refetch on switch) + entry count.
  - **Today view:** entries grouped by `game ?? "No game detected"`, groups sorted by entry count desc, each group header `game — N clips`, rows: event icon/badge (created/moved/renamed/undone/game_edited), `filename`, time (`ts` → locale time), destination folder name for moved rows (last path segment of the parent dir).
  - **All view:** grouped by `day` (desc), sub-grouped by game, day header shows the date + total.
  - Row click → reveal in Explorer (`revealInExplorer(path)`); right-click → small inline menu (Reveal / Play via existing play command in commands.ts — check its exact name — / Copy path / Copy filename via `navigator.clipboard`) — follow EventLog's `EntryContextMenu` idiom; if that component is cleanly reusable, extract/reuse it instead of duplicating (prefer extraction to `src/components/EntryContextMenu.tsx` if it currently lives inside EventLog.tsx).
  - Per-row "Edit game…" in the context menu → inline edit state on the row: text input prefilled with `game ?? ""`, [Save] and [Save & Remember] buttons (Remember disabled with tooltip when `exe` is missing: "No exe recorded for this clip"), calls `editHistoryGame`, refetches on success, toast on error.
  - Footer: "Restore log display" ghost button — moves the OLD Restore behavior here (calls `restoreLog()`, passes result up via the existing `onRestore` prop chain), plus a hint line "Today starts at {day_rollover_hour}:00 (Settings)" — BottomBar already has no config prop; pass `dayRolloverHour` down from App.tsx (App has config).
- [ ] **Step 3: BottomBar swap.** Replace the Restore `<Button>` with `<HistoryPanelButton onRestore={onRestore} dayRolloverHour={...} />` (History icon from lucide — `History`). Keep the `onRestore` prop contract unchanged. App.tsx passes `config.day_rollover_hour`.
- [ ] **Step 4: Verify** — `pnpm exec tsc --noEmit`, `pnpm build` clean.
- [ ] **Step 5: Commit** — "Replace Restore with History panel: today-by-game view, full history, edit game with Remember"

---

### Task 5: Docs + verification backlog

**Files:**
- Modify: `Docs/Verify/2026-07-10-master-verification-checklist.md` (append §16 "History panel + day rollover"; keep **Updated:** current)
- Modify: `Docs/Features/Clip-Metadata-Interop.md` (status: `game_edited` events now implemented; `exe` field documented if Task 2 didn't already; day-bucketing note points at rollover hour)

- [ ] **Step 1: §16 items** — commit range + ship date + automated coverage (name the new tests from Tasks 1–3), human items:
  - History button opens; Today groups by game with correct counts after a few clips.
  - Clip at 3:50 AM (or temporarily set rollover to a near-future hour) lands in the PREVIOUS day's bucket; G-key badge counts agree with the panel.
  - All view groups by day; entries match history.jsonl.
  - Edit game → Save relabels; Save & Remember adds the override in Settings and the next clip from that exe uses it.
  - Remember disabled (tooltip) on entries without exe.
  - Restore log display still works from the panel footer.
  - Right-click menu actions work on rows (Reveal/Play/Copy).
- [ ] **Step 2: Commit** — "Add history-panel verification section; mark game_edited implemented in interop contract"

---

## Self-Review Notes

- Spec coverage: Restore→History (T4), today-by-game + rollover (T1/T3/T4), stats share the boundary (T1), full-history view (T3/T4), row context menu + Edit game/Remember (T4 + T3), backend commands (T3). Spec's "virtualized list" is satisfied by max-height scroll + grouped rendering; true windowing deferred until the file is big enough to matter (note in §16 if sluggish).
- Type consistency: `logical_today` (T1) used in T3; `HistoryEntryPayload.day` (T3) consumed in T4 grouping; `exe` flows T2 → T3 payload → T4 Remember gate.
- Phase-1 leftovers consumed here: `history::read_all`, `remember_game_override`, `GameSnapshot.exe_stem` (their allows come off in T2/T3). `with_rating/with_label/with_description`, `PropValue::Stars/Description` remain Phase 3 — allows stay.
