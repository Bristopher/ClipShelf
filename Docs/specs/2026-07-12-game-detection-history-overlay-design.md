# Game Detection, History Panel & In-Game Overlay — Design

**Date:** 2026-07-12
**Status:** Approved design, not yet implemented
**Phases:** 1) Game detection + history store → 2) History panel → 3) In-game overlay
Each phase ships independently, in order, with tests and verification-checklist entries.

---

## Problem

1. Clips have no record of what game they came from, so sorting/browsing later is guesswork.
2. There is no way to see "what did I clip today" — the Restore button only resurrects the log display.
3. The user can never remember the G-key binds mid-game, so clips never get labeled, rated,
   or sorted in the moment. An in-game overlay with visible keybinds solves this.

---

## Phase 1 — Game detection + history store

### Detection moment
- Primary: the instant the user presses their **save-clip bind** (they are guaranteed to be
  in-game at that moment), snapshot the foreground window.
- Fallback: clips that arrive with no matching hotkey press (folder watcher / OBS WS only)
  get a snapshot at file-creation time instead.
- The snapshot is paired with the clip via the existing FileCreated flow (same pairing the
  save-clip health check already does).

### Classification (Win32, `windows` crate)
1. `GetForegroundWindow` → PID → exe path (`QueryFullProcessImageName`) + window title.
2. **Fullscreen test:** window rect covers its monitor's full rect → treat as a game.
   This single check catches exclusive fullscreen AND borderless-windowed.
3. Game display name: exe version-info `ProductName` (fallback `FileDescription`,
   fallback window title, fallback exe stem). So "Counter-Strike 2", not "cs2.exe".
4. Not fullscreen → label `Desktop-<AppName>` (e.g. `Desktop-Discord`) from the focused window.
5. **Override map first:** `game_overrides` in config (`exe stem → display name`). Checked
   before steps 2–4. Populated by "Remember" in the overlay/History panel; editable in Settings.
6. Master toggle `game_detection_enabled`, **default true**.

### History store — `history.jsonl`
- Append-only JSONL next to the config file. Kept forever. Source of truth.
- One line per event. Schema (fields omitted when not applicable):

```json
{
  "ts": "2026-07-12T21:34:56-06:00",
  "event": "created | moved | renamed | rated | labeled | described | game_edited | undone",
  "path": "C:/clips/clip.mp4",
  "old_path": "C:/clips/old.mp4",
  "game": "Counter-Strike 2",
  "key": 1,
  "rating": 4,
  "label": "clutch",
  "description": "1v4 on Mirage",
  "source": "hotkey | overlay | drop | app"
}
```

- Writes happen on the blocking pool, outside the state lock (same discipline as config saves).
- Corrupt/unparseable lines are skipped on read, never fatal.

### Windows property writes (best-effort mirror)
- Via `IPropertyStore` (`SHGetPropertyStoreFromParsingName`, read-write). No re-encode;
  the shell property handler updates the MP4's metadata atoms in place.
- Mapping:
  - Game → `System.Keywords` (Tags) — the game name as a tag.
  - Stars → `System.Rating` (1★=1, 2★=25, 3★=50, 4★=75, 5★=99 — Explorer's own scale).
  - Description → `System.Comment`.
- **Lock safety (hard rule):** before any property write, probe the file by opening it with
  exclusive share access. If the open fails (OBS or anything else still holds a handle),
  back off and retry on the blocking pool (~1.7 s cadence, same as the mover), bounded
  attempts. If it never frees, SKIP the write and log a warning — `history.jsonl` already
  has the data, and we never touch a file something is still writing.
- Property writes are always best-effort: failure downgrades to a warning, never an error toast.

### Config additions (all via the existing draft/save Settings model)
- `game_detection_enabled: bool` (default true)
- `game_overrides: Vec<GameOverride { exe: String, name: String }>` (TOML string keys — see
  stats.rs lesson: no integer-keyed maps)
- `day_rollover_hour: u8` (default 4) — see Phase 2
- `write_file_properties: bool` (default true)

---

## Phase 2 — History panel

- The **Restore button becomes History**; "Restore log display" survives as an entry inside
  the panel.
- **Today view (default):** entries grouped by game with counts, newest first. "Today"
  begins at `day_rollover_hour` (default **4:00 AM**, configurable) so late-night sessions
  don't split at midnight.
- The existing daily G-key stats (`gkey_stats.toml`) switch to the **same** rollover hour so
  the badge counts and the History panel never disagree.
- **Full history toggle:** all of `history.jsonl`, grouped by day → game, virtualized list.
- Rows reuse the log's right-click menu (Reveal / Play / Copy path / Copy filename) plus
  **Edit game… / Remember** (writes to `game_overrides` and re-labels that entry).
- Backend commands: `get_history(range)` reads/parses JSONL on the blocking pool;
  `edit_history_game(path, name, remember)`.

---

## Phase 3 — In-game overlay

### Trigger
- Global hotkey, **default `Shift+F1`**, fully rebindable in Settings via the existing
  capture UI (a future G-key or macro-pad key just works).
- Modifier-leak note (accepted): the game sees the Shift keydown (momentary walk/sprint);
  F1 itself is swallowed by the hotkey registration, so no in-game help/console triggers.

### Window
- Second Tauri WebviewWindow: transparent, always-on-top, no decorations, skip-taskbar,
  **non-activating** (`WS_EX_NOACTIVATE` set on the raw HWND). The game keeps focus and
  keeps rendering; mouse clicks on the overlay work without stealing focus.
- Honest limit: like every non-injecting overlay (Discord's included), it cannot draw over
  a TRUE exclusive-fullscreen game. On Windows 10/11 most titles run "fullscreen
  optimizations" (borderless under the hood) where the overlay displays fine. Detection and
  keyboard capture work regardless — only visibility is affected.

### Menu (CS:GO buy-menu style)
- Compact numbered panel acting on the **most recent clip** (filename + detected game shown
  at the top). While open, `1`–`9`, `0`, and `Esc` are registered as temporary global
  hotkeys — grabbed on open, released the instant it closes — and every action row shows
  its configured main-app keybind so the binds get learned passively.
  - `1` Sort → G1/G2/G3 destinations, each labeled with its tag AND configured bind
  - `2` Rate → `1`–`5` = stars (property write + history event)
  - `3` Label → preset chips (configurable list, e.g. clutch/ace/funny/fail), `0` = custom
  - `4` Description → preset chips, `0` = custom
  - `5` Game → confirm / pick from recent games / **Remember** (writes override)
  - `6` Timer toggle
  - `Esc` or `Shift+F1` again closes

### Label semantics
- A label is appended to the filename before the extension: `Name - label.mp4`
  (collision-safe via the existing rename path), plus a `labeled` history event.

### Type mode (custom text) — keyboard hook, no focus steal
- Choosing custom (`0`) activates a text field WITHOUT giving the overlay focus:
  a temporary **`WH_KEYBOARD_LL` low-level keyboard hook** captures keystrokes into the
  overlay's buffer and swallows them so the game never reacts. Same mechanism Discord uses
  for push-to-talk; runs entirely in our process, injects nothing → anti-cheat-safe.
- The game window keeps focus the whole time → **nothing can minimize**, including true
  exclusive fullscreen.
- Shift tracked for capitals; `Enter` commits, `Esc` cancels; the hook is removed the
  instant the field closes. Plain-text only (no IME/emoji) — fine for suffixes and short
  descriptions.
- Settings toggle `overlay_typing_enabled` ("allow typing in overlay — captures your
  keyboard while the text box is open"), **default true**. When off, custom marks the clip
  `needs-label` and the main app prompts after the session.

### Config additions
- `overlay_enabled: bool` (default true), `overlay_bind: String` (default "shift+F1")
- `label_presets: Vec<String>`, `description_presets: Vec<String>`
- `overlay_typing_enabled: bool` (default true)

---

## Rejected alternatives
- **Native Win32 overlay (no webview):** lighter, but duplicates the whole UI stack for
  marginal gain.
- **Main-window "mini mode":** cannot appear over a game at all.
- **Focus-stealing type mode:** superseded by the LL-hook design — focus steal risked
  minimizing exclusive-fullscreen games; the hook removes the tradeoff entirely.
- **Embedding metadata in the MP4 as the only store:** rewriting a file OBS may still hold
  is risky, and scanning tags across thousands of clips is slow. JSONL is the source of
  truth; properties are a mirror.

## Testing
- Pure-Rust units: classification from (rect, monitor, exe info) fixtures; override-map
  precedence; JSONL round-trip incl. corrupt-line skip; rollover-hour day bucketing
  (23:59 vs 03:59 vs 04:00); rating→System.Rating scale mapping; label filename append.
- Manual (verification checklist): property writes visible in Explorer, lock-retry against
  a held file, overlay over borderless vs exclusive fullscreen, hook typing while game
  stays focused, temporary hotkey grab/release.

## External consumers
The metadata contract (filename labels, Windows properties, `history.jsonl`) is documented
for other apps in `Docs/Features/Clip-Metadata-Interop.md`.
