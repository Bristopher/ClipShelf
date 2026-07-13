# Phase 3: In-Game Overlay Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Shift+F1 (rebindable) in-game overlay — non-activating always-on-top window, CS:GO-buy-menu style, driven by temporary number-key hotkeys — that sorts, rates, labels, describes, fixes the game, and toggles the stopwatch for the most recent clip, with a WH_KEYBOARD_LL type mode that never steals focus.

**Architecture:** New `overlay.rs` (window mgmt + action commands) and `keyhook.rs` (LL keyboard hook thread). Overlay window pre-created hidden in setup like settings/first-run, routed by label in `main.tsx` → new `OverlayApp.tsx`. Hotkey plumbing extends the existing RegisterHotKey listener: a persistent `OverlayToggle` binding plus temporary digit/Esc bindings swapped in via the controller's existing `reload()` mechanism while the overlay is visible.

**Tech Stack:** Rust windows-sys (WS_EX_NOACTIVATE, WH_KEYBOARD_LL), existing hotkeys/props/history/mover modules, React overlay UI.

**Spec:** `Docs/specs/2026-07-12-game-detection-history-overlay-design.md` (Phase 3). Contract: `Docs/Features/Clip-Metadata-Interop.md`.

## Global Constraints

- No disk IO / blocking calls under the state lock; blocking work on the blocking pool.
- All new config via serde-default fns + `impl Default` (config.rs house style); TOML-safe types only.
- Rust warning-free at every task end; `#[allow(dead_code)]` only with `// consumed in <task> (...)` comment and only when a LATER task consumes it.
- Frontend: pnpm only; `pnpm exec tsc --noEmit` + `pnpm build` clean per task.
- History event vocabulary additions used here: `rated`, `labeled`, `described` (already in the interop contract as design-only — Task 7 flips them to implemented).
- Property writes for rating/description go through `props::write_with_retry_resolving` with the same identity closure pattern lib.rs uses for Game (never a raw path captured at event time).
- The overlay must NEVER take focus: window created `.focused(false)` + WS_EX_NOACTIVATE|WS_EX_TOOLWINDOW applied to the raw HWND; all keyboard input arrives via global hotkeys or the LL hook, never webview key events.
- Commit per task with the stated message + `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`; never commit tauri.conf.json/Cargo.lock-only noise/.superpowers.

---

### Task 1: Overlay config fields

**Files:** `src-tauri/src/config.rs`, `src/types/index.ts`

**Interfaces produced:** `overlay_enabled: bool` (default true), `overlay_bind: String` (default `"shift+F1"`), `overlay_typing_enabled: bool` (default true), `label_presets: Vec<String>` (default `["clutch","ace","funny","fail"]`), `description_presets: Vec<String>` (default `[]`).

- [ ] TDD: test `test_overlay_config_defaults` asserting all five defaults + a TOML roundtrip with edited presets. Implement per house style (default fns, serde attrs, impl Default entries, doc comments in neighbor voice). TS interface mirrors: `overlay_enabled: boolean; overlay_bind: string; overlay_typing_enabled: boolean; label_presets: string[]; description_presets: string[]`.
- [ ] Verify: cargo test green, zero warnings, tsc clean.
- [ ] Commit: "Add overlay config: bind, typing toggle, label and description presets"

---

### Task 2: Overlay window — creation, non-activation, show/hide

**Files:** Create `src-tauri/src/overlay.rs`; modify `src-tauri/src/lib.rs` (mod + setup pre-creation + command registration), `src/main.tsx` (route label `overlay` → placeholder `OverlayApp`), create `src/OverlayApp.tsx` (minimal shell this task: fixed-size dark rounded panel saying "Overlay" — Task 6 replaces it).

**Interfaces produced:**
- `overlay::init(app: &AppHandle)` — called in setup AFTER the settings/first-run builders, mirrors their pattern: label `"overlay"`, `WebviewUrl::App(PathBuf::new())`, `.inner_size(420.0, 480.0)`, `.resizable(false)`, `.decorations(false)`, `.transparent(true)`, `.always_on_top(true)`, `.skip_taskbar(true)`, `.focused(false)`, `.visible(false)`, then `apply_noactivate(&window)`.
- `overlay::apply_noactivate(window)` — raw HWND via `window.hwnd()`; `SetWindowLongPtrW(hwnd, GWL_EXSTYLE, prev | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW)` (windows-sys `Win32_UI_WindowsAndMessaging`, already a feature).
- `overlay::show(app) `/`overlay::hide(app)` — show positions the window at the bottom-center of the monitor containing the cursor (`window.current_monitor()` fallback primary; compute x = mon.x + (mon.w - win.w)/2, y = mon.y + mon.h - win.h - 80, physical px) then `show()` WITHOUT focusing; hide just hides. Both emit `overlay-visible` `{visible: bool}` app-wide.
- Tauri commands `show_overlay`/`hide_overlay` (thin wrappers, registered) for dev/testing.
- `transparent` needs the `macos-private-api` flag ONLY on macOS — we are Windows-only, plain `.transparent(true)` is fine.

- [ ] Manual-gate task (window behavior isn't unit-testable): `cargo test` stays green/warning-free; `pnpm build` clean; report must state `pnpm tauri dev` was launched and `show_overlay` invoked (via the existing devtools or a temporary keybind is NOT needed — invoke from main window console `__TAURI__.core.invoke('show_overlay')`) with the game-focus check: clicking the overlay must NOT steal focus from another focused window (verify by watching the other window's title bar stay active).
- [ ] Commit: "Add non-activating always-on-top overlay window with show/hide plumbing"

---

### Task 3: Hotkeys — persistent toggle + temporary digit keys

**Files:** `src-tauri/src/hotkeys.rs`, `src-tauri/src/lib.rs`

**Interfaces produced:**
- `HotkeyAction::OverlayToggle` and `HotkeyAction::OverlayKey(u8)` (0–9 digits; 10 = Esc sentinel — use `OverlayKey(10)`) with `label()` arms ("Overlay toggle", "Overlay key").
- `bindings_from_config` adds `(OverlayToggle, config.overlay_bind)` when `overlay_enabled` && bind non-empty.
- `HotkeyController::set_overlay_keys(&self, active: bool)` — when active, the listener ALSO registers plain `1`–`9`, `0`, and `Esc` (VK_ESCAPE) mapped to `OverlayKey(n)`; when inactive they are unregistered. Implement via the existing pending/reload mechanism: keep the controller's last full binding list, append the temp set when active (digits parse as key names via existing `key_name_to_vk` — verify "1".."9","0","escape" are handled; extend the map if not). Registration failures for digits go to the existing failure event path but with a distinct context string "overlay keys" (a game or app rarely holds global digit registrations; failures must not kill the toggle).
- lib.rs hotkey handler arms: `OverlayToggle` → if overlay window visible → `overlay::hide` + `set_overlay_keys(false)`; else `overlay::show` + `set_overlay_keys(true)` + emit `overlay-open` carrying a fresh payload: current clip filename + game label (short state lock). `OverlayKey(n)` → `app.emit("overlay-key", n)` (frontend interprets; 10 = Esc arrives as n=10 → frontend requests hide via `hide_overlay`... NO — handle Esc in Rust directly: hide + release keys, symmetric with toggle).
- Hide must ALWAYS release the temp keys (single helper `close_overlay(app, channels)` used by Esc arm, toggle arm, and a `hide_overlay` command call).

- [ ] TDD where pure: extend the `key_name_to_vk` test coverage for "0"-"9"/"escape" if missing. Listener changes are thread-plumbing — cover by compile + existing tests + Task 2's manual gate extended: report must state toggle open/close with Shift+F1 works in dev and digits reach the frontend (console.log in the placeholder OverlayApp).
- [ ] Verify: cargo test green, zero warnings; tsc/build clean if OverlayApp touched.
- [ ] Commit: "Register overlay toggle hotkey and temporary digit keys while the overlay is open"

---

### Task 4: Overlay action commands

**Files:** `src-tauri/src/overlay.rs` (commands live here), `src-tauri/src/lib.rs` (registration), `src-tauri/src/state.rs` if a small accessor helps.

**Interfaces produced (all `#[tauri::command]`, all act on `current_file` — the most recent clip — and error `"No recent clip"` when none):**
- `overlay_get_context(state) -> OverlayContext { filename, path, game, exe, labelPresets, descriptionPresets, typingEnabled, binds: {g1,g2,g3,g1Name,g2Name,g3Name,overlay} }` — one snapshot the UI renders from (camelCase serde struct in events.rs or overlay.rs).
- `overlay_sort(app, state, key: u8)` — validates 1–3, runs the EXISTING `move_file_with_key` path on the blocking pool with source `"overlay"` and pushes the undo entry exactly like `do_press_gkey` (read do_press_gkey; reuse, don't fork logic — if a thin shared helper is needed, extract it).
- `overlay_rate(app, state, stars: u8)` — clamp 1–5; history `rated` event (with_rating + game from clip_games, source "overlay"); when `write_file_properties`, spawn_blocking `props::write_with_retry_resolving(resolve_clip_current_path closure, &[PropValue::Stars(stars)])` — copy lib.rs's closure pattern; Success log "Rated ★{n}: {filename}" + log-entry emit.
- `overlay_label(app, state, label: String)` — trim, reject empty; collision-safe rename appending ` - {label}` before the extension (use mover's existing rename/collision helpers — read mover.rs first; the new name = `{stem} - {label}.{ext}`); updates current_file/moved_path, re-keys clip_games, pushes undo entry, history `labeled` event (with_label, old_path, source "overlay"), log Success.
- `overlay_describe(app, state, text: String)` — trim, reject empty; history `described` event (with_description, source "overlay"); props Description via the same resolving pattern when enabled; log Success.
- `overlay_set_game(app, state, game: String, remember: bool)` — delegates to the exact same logic as `edit_history_game` (call it or extract the shared core) with the current clip's path + exe (exe: the clip's created event exe is not in state — pass exe from `overlay_get_context`'s value, which comes from the pending snapshot stored... simplest correct source: extend `clip_games` value to a small struct? NO schema churn: keep a parallel session map `clip_exes: HashMap<PathBuf,String>` maintained exactly like clip_games' re-keying (add in this task, state.rs + the three re-key sites; the created-event site populates it).
- `overlay_timer_toggle(channels)` — sends `CountUpCommand::Toggle` (the stopwatch — same action as the count-up hotkey).
- `overlay_needs_label(app, state)` — when typing is disabled and the user picks custom: Warning log entry + emit "log-entry": "Clip needs a label: {filename}" with path (visible reminder in the event log; NO new history vocabulary — note this as accepted drift from the spec's needs-label flag in the report).

- [ ] TDD: pure/lockable parts get tests — the ` - label` filename construction (extract `pub(crate) fn labeled_name(path, label) -> PathBuf` with tests: extension preserved, no extension, label trimmed) and `clip_exes` re-keying (extend the state test). Command bodies follow existing tested paths.
- [ ] Verify: cargo test green, zero warnings.
- [ ] Commit: "Add overlay action commands: sort, rate, label, describe, set game, timer toggle"

---

### Task 5: `keyhook.rs` — WH_KEYBOARD_LL type mode

**Files:** Create `src-tauri/src/keyhook.rs`; modify `src-tauri/src/lib.rs` (mod, two commands registered, spawn the hook thread lazily).

**Interfaces produced:**
- `keyhook::start(app: AppHandle) -> Result<(), String>` / `keyhook::stop()` — commands `start_type_mode` (gated: error if `!overlay_typing_enabled`)/`stop_type_mode` wrap them.
- Implementation contract: a dedicated OS thread (spawned on first start, reused after) runs `SetWindowsHookExW(WH_KEYBOARD_LL, hook_proc, hinstance, 0)` + `GetMessageW` pump; `stop()` posts a custom thread message that unhooks (`UnhookWindowsHookEx`) but keeps the thread parked for reuse (or exits the thread cleanly — implementer's choice, document it). An `AtomicBool ACTIVE` gates the proc; a `OnceLock<AppHandle>` (or channel) lets the proc emit.
- `hook_proc`: when ACTIVE and `wParam` is WM_KEYDOWN/WM_SYSKEYDOWN: read the `KBDLLHOOKSTRUCT` vkCode; track Shift state (VK_SHIFT/LSHIFT/RSHIFT down/up); translate A–Z (case by shift), 0–9 and their shifted symbols NOT needed (plain digits fine), space, minus/underscore, period — anything else unmapped is swallowed silently except: VK_RETURN → emit `{kind:"enter"}`, VK_ESCAPE → `{kind:"esc"}`, VK_BACK → `{kind:"backspace"}`; mapped chars emit `{kind:"char", ch}`. Event name `overlay-type`. Return `1` (swallow) for EVERY key while ACTIVE (including modifiers' keydowns? NO — let pure modifier keys pass through (return CallNextHookEx) so the game doesn't see stuck/blocked Shift weirdness; swallow everything else). When not ACTIVE: `CallNextHookEx` immediately, first line.
- SAFETY rule stated in code: the hook proc must do NO locking and NO blocking — emit via a pre-cloned handle only; if emit fails, drop silently.
- Frontend contract (Task 6 consumes): while type mode is on, OverlayApp builds the string from `overlay-type` events; `enter` commits (calls the pending action command), `esc` cancels; both then `stop_type_mode`.

- [ ] Tests: vk→char translation extracted as a pure fn `translate_vk(vk: u32, shift: bool) -> Option<char>` with a table test (letters both cases, digits, space, minus, period, unmapped → None). Hook lifecycle is manual (checklist).
- [ ] Verify: cargo test green, zero warnings.
- [ ] Commit: "Add low-level keyboard hook type mode: capture text without stealing game focus"

---

### Task 6: OverlayApp UI + Settings section

**Files:** Rewrite `src/OverlayApp.tsx`; modify `src/components/SettingsForm.tsx`, `src/lib/commands.ts`, `src/types/index.ts`; possibly `src/main.tsx` (already routed in Task 2).

**Requirements:**
- commands.ts: `overlayGetContext`, `overlaySort(key)`, `overlayRate(stars)`, `overlayLabel(label)`, `overlayDescribe(text)`, `overlaySetGame(game, remember)`, `overlayTimerToggle`, `overlayNeedsLabel`, `startTypeMode`, `stopTypeMode`, `hideOverlay` — names matching the registered Rust commands.
- OverlayApp: dark translucent panel (bg like `bg-black/85`, rounded, border) — it renders over a game, so: large readable text, no theme dependence on the main window's CSS variables if they don't load in this window (check index.css applies; if theme tokens work, use them). Layout top→bottom: header (filename truncated middle + game chip), then the numbered action list, footer hint "Esc closes · press the number".
- State machine: `menu = "root" | "sort" | "rate" | "label" | "describe" | "game" | {type: "typing", target: "label"|"describe"|"game"}`. Root rows: `1 Sort` `2 Rate` `3 Label` `4 Description` `5 Game` `6 Timer` — each row ALSO shows its main-app bind where one exists (from overlayGetContext.binds: sort rows show g1/g2/g3 binds + folder names on the sort submenu; header shows the overlay bind). Digit events (`overlay-key` payload 1–9/0) drive selection per menu; `0` = back to root on submenus; Esc handled in Rust (window hides) — on `overlay-visible {visible:false}` reset to root. Submenus: sort → 1–3 destinations (folder names + binds); rate → 1–5 stars rendered ★; label/describe → numbered preset chips (from context) + `0 custom…` (typing mode if typingEnabled else overlayNeedsLabel + toast-style inline notice); game → `1 confirm` (no-op close), `2..n recent games?` NO — keep spec-minimal: `1 Keep "{game}"`, `2 Edit (type)` (typing mode targeting game; on commit `overlaySetGame(text, false)`), `3 Edit & Remember` (same but remember=true, disabled without exe).
- Every action: await the command, show a 1s inline success flash ("Sorted → {folder}" etc.), then `hideOverlay()` (except Timer which flashes but stays open? — close too, simplest and predictable).
- Typing mode: input-LOOKING div (not a real focused input — the window must not need focus): shows buffer + blinking caret, fed by `overlay-type` events; enter commits per target, esc cancels back to the submenu; call startTypeMode/stopTypeMode around it; if startTypeMode errors (typing disabled), fall back to needs-label notice.
- Mouse: all rows clickable (mousedown works without focus thanks to NOACTIVATE) — same handlers as digits.
- SettingsForm: "Overlay" section — enable Switch, bind capture (REUSE the existing keybind-capture component/idiom the other binds use — read how g1_bind is edited), typing Switch, label presets editor + description presets editor (chip list: text input + Add, ✕ per chip — mirror the overrides-table idiom), all draft-only.
- Listen for `overlay-open` (payload refreshes context) and refetch `overlayGetContext` on each open.

- [ ] Verify: tsc + pnpm build clean.
- [ ] Commit: "Build CS:GO-style overlay menu UI and overlay settings section"

---

### Task 7: Docs, contract flip, verify backlog §17

**Files:** `Docs/Verify/2026-07-10-master-verification-checklist.md` (§17 + Updated line), `Docs/Features/Clip-Metadata-Interop.md` (Status: rated/labeled/described events + System.Rating/System.Comment writes + filename ` - label` suffix now IMPLEMENTED), `Docs/future/feature-ideas.md` (add: needs-label queue/prompt-after-session if the Task 4 drift stands; overlay recent-games quick-pick).

- [ ] §17 human items: Shift+F1 opens over a borderless game without stealing focus (game keeps rendering + input until a digit is pressed); digits drive menus while game is focused; Esc closes and releases digit keys (verify digits reach the game again after close); sort/rate/label/describe/game/timer each work end-to-end (Explorer shows Rating stars + Comments; filename gains " - label"; history gains rated/labeled/described rows; History panel groups reflect edits); type mode: game keeps focus while typing, keys don't reach the game, Enter commits/Esc cancels, hook released after (typing in the game works again); typing-disabled path shows the needs-label notice; exclusive-fullscreen game: overlay may not draw (documented limitation) but hotkeys/detection unaffected; overlay bind rebindable in Settings incl. plain G-key; digit registration failure (another app holding a digit hotkey) degrades gracefully.
- [ ] Verify docs internally consistent; commit: "Add overlay verification section; mark rating/label/description contract parts implemented"

---

## Self-Review Notes

- Spec deltas carried consciously: needs-label = visible log reminder instead of a persisted flag (Docs/future captures the queue idea); game submenu offers keep/edit/remember rather than a recent-games list; Esc handled Rust-side for guaranteed key release. All flagged for §17/feature-ideas.
- Cross-task interfaces: OverlayKey(10)=Esc never reaches the frontend (Rust hides directly); `clip_exes` (T4) feeds `overlay_get_context.exe` and `overlay_set_game` remember; `translate_vk` (T5) is the only key→char source; binds displayed come from `overlay_get_context.binds` (T4) not a second config fetch.
- Riskiest bits for the final review: LL hook thread lifecycle, temp-digit registration/release symmetry, rename-while-OBS-holds interplay (labeled rename uses the mover's existing retry path).
