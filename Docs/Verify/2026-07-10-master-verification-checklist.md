# Master Verification Checklist (2026-07-10)

**Updated:** 2026-07-13 (adds §18; §16 history is now a full main-view toggle)

Everything code-verified but not yet exercised live, across three batches:
the 2026-07-02 QoL batch (undo, pause, clickable log, autostart, window
layout), the 2026-07-05 stability/toasts/UX batch, and the two 2026-07-10
feature rounds (drag-drop, diagnostics, MRU/tokens, filter, stats, first-run
OBS, batch undo). Supersedes the two older checklists in this folder.
§15 adds the 2026-07-12 game-detection + history-store feature (Phase 1),
§16 adds the History panel + day rollover (Phase 2), §17 adds the in-game
overlay (Phase 3).

All of it passes 51/51 cargo tests, tsc, and pnpm build (§1–§14). §15's
game-detection round adds its own suite — see that section for numbers.
Work through §1–§12 in `pnpm tauri dev`, then §13–§14 with
`.\build-release.ps1`.

Specs: `Docs/specs/2026-07-05-stability-round2-and-error-surface-design.md`,
`Docs/specs/2026-07-10-feature-batch-design.md`.

## 1. Launch & first-run
- [ ] App starts clean; log shows "GKey Mover started"; window restores its
      remembered position/size (move/resize, Ctrl+click X to quit, relaunch)
- [ ] Second launch of the exe just focuses the existing window
- [ ] Reset first-run (clear videos_folder in config): setup window opens;
      OBS WebSocket section present (optional, toggle reveals password)
- [ ] OBS Test button: OBS running + right password → green "Connected";
      wrong password → red "Authentication failed"; OBS closed → red
      "Couldn't reach OBS" (within ~5s)
- [ ] Finishing setup starts the watcher and (if enabled) connects OBS

## 2. Toasts / error surface
- [ ] Click a G-key with no clip detected → small toast bottom-right (NOT
      the full-screen red error page)
- [ ] Toast auto-dismisses ~6s; X dismisses; duplicates collapse; max 4
- [ ] Delete a clip in Explorer, click its log entry → "File no longer
      exists" toast
- [ ] Settings → Save → green "Settings saved" toast (no alert() popup)

## 3. Hotkeys & binds
- [ ] Bind a combo another app owns → Save → error toast + red log entry
      naming the bind and action
- [ ] Two actions on the same key in Settings → inline "Same key as X —
      only one of them will work" note under both
- [ ] G1–G3 move clips; rename bind opens the dialog; count-up bind works
- [ ] Bind changes apply immediately after Save (no restart)

## 4. Undo
- [ ] Move a clip with G1, press undo hotkey (or bottom-bar Undo) → clip
      returns; log shows "Undo: X → Y"
- [ ] Undo with nothing to undo → "Nothing to undo" info log, no crash
- [ ] After a MULTI-file drop, ONE undo restores ALL of them — per-file
      "Undo:" lines + "Undo batch: restored n/m" summary
- [ ] Ctrl+Z inside the main window works as undo (in-app shortcut)

## 5. Watcher status, pause & diagnostics
- [ ] On launch with a folder set, bottom bar shows "Watching" immediately
      (not "Stopped")
- [ ] Click Watching → amber "Paused"; new files in the folder are fully
      ignored (no log, no sound); tray "Pause Watching" mirrors both ways
- [ ] Click Paused → resumes; clear the folder in Settings → red "Stopped"
- [ ] Activity icon (bottom-right) opens the diagnostics popover: version,
      watcher status + restart count, OBS status, clips folder (click
      opens), config path (click reveals), shortcut cheat sheet
- [ ] "Restart watcher" button → restart logged, count bumps on next open
- [ ] Esc / click-outside closes the popover

## 6. OBS WebSocket
- [ ] OBS running + integration enabled → green pill in Settings + green
      "OBS" dot in the bottom bar
- [ ] Start app BEFORE OBS → amber Connecting/Reconnecting, flips green on
      its own after OBS starts (≤30s)
- [ ] Wrong password → "OBS WebSocket auth failed" toast + red log entry
- [ ] Close OBS → red dot; reopen → reconnects; disable integration → dot
      disappears
- [ ] Save a replay via OBS → clip appears ONCE (no double log/sound)

## 7. Event log
- [ ] Click a clip entry → reveals in Explorer; Ctrl+Click → plays; hover
      tooltip (~0.4s) bolds whichever action Ctrl state matches
- [ ] Right-click a clip entry → context menu: Reveal, Play, Copy path,
      Copy filename (paste somewhere to confirm the copies)
- [ ] Ctrl+F → filter bar opens focused; text narrows live; level chips
      filter; "n / m" count correct; Esc (or X) closes and restores
- [ ] Scroll UP in a busy log → new entries do NOT yank you down; scroll
      back to bottom → auto-follow resumes
- [ ] Del wipes; Restore brings entries back, still clickable
- [ ] Empty log shows watcher status + watched folder + "press <bind>
      in-game" hint (not bare "Waiting for events...")

## 8. Sidebar
- [ ] G1–G3 tags show your configured folder names; renaming a folder in
      Settings updates them after Save
- [ ] Count badges appear after moves and show TODAY's count (persist
      across same-day restarts — gkey_stats.toml)
- [ ] Hover a badged button ~0.5s → flyout: folder name, "N today", last
      ≤5 clips this session; clicking one reveals it in Explorer
- [ ] Fresh launch: badges keep counts, flyout stays closed until the
      first move of the session (recent list is session-only)

## 9. Drag-and-drop
- [ ] Drag a video over the window → dashed "Drop to select & rename"
      overlay; hovering G1–G3 highlights that button and hides the overlay
- [ ] Drop on G2 → moves to the G2 folder (collision-safe " (2)" on name
      clash; badge bumps; move sound if enabled)
- [ ] Drop SEVERAL videos on a G-key → all move; "Sorted N clips" toast;
      failures → "N couldn't be moved" toast + log entries
- [ ] Drop on the log (or G4) → rename dialog opens with that file's name;
      the file became the current clip (G-keys now act on it)
- [ ] Drop a .txt → error toast, nothing happens
- [ ] Multi-drop on the rename area → uses first video, info toast says so

## 10. Rename dialog
- [ ] Live "→ OriginalName - yourtext.mp4" preview while typing
- [ ] Illegal char (? / etc.) → red warning, Rename disabled; empty input
      disabled; Enter submits when valid
- [ ] MRU chips appear after renames (dedupe case-insensitively, newest
      first, max 8, survive restart); clicking a chip fills the input
- [ ] "scrim {date}" → file gets the real date (preview showed it); the
      MRU chip keeps the raw "{date}" template; {time} → HH.MM with dots
- [ ] With Settings open AND dirty, rename a clip, then Save Settings →
      the new MRU chip is NOT lost

## 11. Settings window
- [ ] Ctrl+S saves when dirty; close with unsaved changes → button bar
      scroll + shake, "Exit without saving" works
- [ ] Custom .mp3/.wav pickers for clip-save + error sounds play the file;
      X reverts to default
- [ ] Autostart toggle adds/removes the entry in Task Manager → Startup
- [ ] Window Reset button snaps to the configured monitor/anchor default
- [ ] Tray: "Video Folder"/"Log Folder" open the CORRECT folders; tooltip
      shows the real current version; "Pause Watching" checkbox works

## 12. In-app shortcuts (main window)
- [ ] Del = wipe, Ctrl+Z = undo, Ctrl+F = filter, Ctrl+, = Settings
- [ ] None fire while typing in the rename dialog or filter box
- [ ] All four listed in the diagnostics popover

## 13. Release build (`.\build-release.ps1`)
- [ ] Script suggests the right next version (max of Releases folders and
      tauri.conf.json, +1 patch)
- [ ] After install: Settings footer + title-bar hover + tray tooltip +
      diagnostics popover all show the new version
- [ ] Upgrade over the previous version keeps: config (folders, binds,
      themes, MRU), window layout, autostart state, today's stats
- [ ] Update flow: after install completes, launching from the Start menu
      works (no zombie/locked instance)

## 14. Config & data durability
- [ ] Save settings, kill the app from Task Manager immediately, relaunch
      → settings intact (no silent reset to defaults)
- [ ] `%APPDATA%\com.cbuzi.gkey-mover-v2\` has `gkey_config.toml`,
      `window_layout.toml`, `gkey_stats.toml` — and no leftover
      `*.toml.tmp` files after saves
- [ ] gkey_stats.toml holds date + g1/g2/g3; counts reset the next
      calendar day

## 15. Game detection + history store

Commits `7eba313..7e06621` (six commits on `main`), shipped 2026-07-12.
Spec: `Docs/specs/2026-07-12-game-detection-history-overlay-design.md`.
Phase 1 only — the History panel UI and stats-rollover switch land in
Phase 2; rating/label/description writers and the overlay land in Phase 3.

**Automated coverage** — full suite 67 passed, `cargo build` zero warnings,
`tsc` + `pnpm build` clean:
- `history.rs`: `test_history_path_is_sibling_of_config`,
  `test_append_and_read_roundtrip`, `test_read_skips_corrupt_lines`,
  `test_read_missing_file_is_empty`, `test_optional_fields_omitted_from_json`
- `config.rs`: `test_game_detection_defaults`,
  `test_game_overrides_toml_roundtrip`,
  `test_remember_game_override_upserts_case_insensitive`
- `gamedetect.rs`: `test_override_wins_regardless_of_fullscreen`,
  `test_fullscreen_prefers_product_name_then_title_then_exe`,
  `test_windowed_gets_desktop_prefix`, `test_whitespace_product_name_is_ignored`
- `props.rs`: `test_stars_to_system_rating_explorer_scale`,
  `test_probe_exclusive_free_vs_held_file`, `test_probe_missing_file_is_false`
- `state.rs`: `test_take_pending_game_respects_age`

**NOT covered by automation** — live Win32 foreground detection, the actual
COM property write as it appears in Explorer, lock-retry behavior against a
genuinely held file, and the Settings section's look/feel. Human items:

- [ ] Clip saved while a fullscreen game is focused → log shows
      `— <game>`; `history.jsonl` gains a `created` line with the right
      game; Explorer Details shows the game in Tags after OBS releases the
      file
- [ ] Borderless-windowed game → same result
- [ ] Clip saved with only Discord focused (windowed) → `Desktop-Discord`
- [ ] Wrong detection → add override in Settings → next clip uses the
      corrected name
- [ ] Hold the file open in another program → property write retries then
      skips with a warning log line; history still has the game
- [ ] G1 move / rename / undo each append their history line (open the
      JSONL and eyeball)
- [ ] Detection toggle off → no game anywhere, everything else unaffected

## 16. History panel + day rollover

Commits `36766f6..98ee95e` (five commits on `main`), shipped 2026-07-12.
Phase 2 — replaces the old Restore button with a History panel and switches
daily stats to rollover-aware bucketing (`day_rollover_hour` in config,
default 4 AM). Rating/label/description writers and the overlay remain
Phase 3.

**Automated coverage** — full suite 74 passed, `cargo build` zero warnings,
`tsc` + `pnpm build` clean:
- `stats.rs`: `test_logical_date_respects_rollover_hour`,
  `test_increment_rolls_over_stale_date`
- `history.rs`: `test_exe_field_roundtrip_and_omitted_when_none`
- `commands.rs`: `test_history_payloads_buckets_orders_and_filters`,
  `test_history_payloads_unparseable_ts_kept_with_prefix_fallback`,
  `test_history_payloads_short_ts_fallback_does_not_panic`

**NOT covered by automation** — the actual History panel UI, cross-timezone
day-bucketing behavior as seen live, and Settings integration for Remember.
Note: each history entry's logical day is computed from the clip's OWN
recorded UTC offset at write time, not the viewer's current timezone — a
clip saved while traveling will bucket by the offset that was active when it
was created, which can look surprising if you change timezones and then
browse old history. Human items:

- [ ] History button opens; Today groups by game with correct counts after a
      few clips
      (2026-07-13: History is now a full main-view toggle, not a popover —
      the button swaps the log for the history view and back; see §18)
- [ ] Clip at 3:50 AM (or temporarily set `day_rollover_hour` to a
      near-future hour) lands in the PREVIOUS day's bucket; G-key badge
      counts agree with the panel
- [ ] All view groups by day; entries match `history.jsonl`
- [ ] Edit game from the CREATED row of an already-sorted clip → the whole
      chain (created + moved rows) relabels into ONE group under the new
      game, never split across two groups; Save & Remember adds the
      override in Settings and the next clip from that exe uses it
- [ ] With `gkey_stats.toml` carrying a stale date (or the app left running
      across the rollover hour), G-key badges read 0 and agree with an
      empty Today panel — before any new move
- [ ] Remember disabled (tooltip) on entries without exe
- [ ] Restore log display still works from the panel footer
- [ ] Right-click menu actions work on rows (Reveal/Play/Copy)

## 17. In-game overlay

Commits `5ca3459..30fe343`, shipped 2026-07-12. Phase 3 — the final phase of
the game-detection/history/overlay design. Spec:
`Docs/specs/2026-07-12-game-detection-history-overlay-design.md`.
Delivered: overlay config (bind shift+F1, typing toggle, label/description
presets), a non-activating always-on-top overlay window
(`WS_EX_NOACTIVATE|WS_EX_TOOLWINDOW`), the `OverlayToggle` global hotkey plus
temporary digit/Esc hotkeys registered while the overlay is open (published
atomically), eight overlay action commands (sort/rate/label/describe/
set-game/timer/needs-label/context) with an acting-path property-write
fallback, a `WH_KEYBOARD_LL` type mode (ACTIVE-gated, Shift passes through,
translation is pure `translate_vk`), a CS:GO-style menu UI with a flash
guard, and a Settings overlay section.

**Automated coverage** — full suite 102 passed, `cargo build` zero warnings,
`tsc` + `pnpm build` clean:
- `config.rs`: `test_overlay_config_defaults`,
  `test_overlay_presets_toml_roundtrip`
- `mover.rs`: `test_labeled_name_preserves_extension`,
  `test_labeled_name_no_extension`, `test_labeled_name_trims_label`,
  `test_rename_file_at_collision_safe`
- `state.rs`: `test_rekey_clip_moves_game_and_exe_in_lockstep`
- `overlay.rs`: `test_with_acting_fallback_identity_wins`,
  `test_with_acting_fallback_identityless_existing_clip`,
  `test_with_acting_fallback_missing_file_stays_skip`
- `keyhook.rs`: `letters_lowercase_without_shift`,
  `letters_uppercase_with_shift`, `digits_ignore_shift`, `numpad_digits`,
  `space`, `minus_and_underscore`, `period`, `unmapped_keys_return_none`

**NOT covered by automation** — live Win32 window layering/focus behavior,
the LL keyboard hook against a real foreground game, and the overlay UI's
look/feel. Human items:

- [ ] Shift+F1 opens the overlay over a borderless game without stealing
      focus — the game keeps rendering and receiving input until a digit is
      pressed
- [ ] Digits drive the overlay menus while the game stays focused
- [ ] Esc closes the overlay and releases the digit hotkeys — verify digits
      reach the game again immediately after close
- [ ] Sort/rate/label/describe/game/timer each work end-to-end: Explorer
      shows Rating stars + Comments on the clip, the filename gains
      ` - label`, `history.jsonl` gains `rated`/`labeled`/`described` rows,
      and the History panel groups reflect the edits
- [ ] Type mode: the game keeps focus while typing, keystrokes don't reach
      the game, Enter commits, Esc cancels, the hook is released afterward
      (typing in the game works again)
- [ ] Type mode entered with Shift already held: the first letter types
      lowercase (known cosmetic quirk) — verify it self-corrects once Shift
      is released and re-pressed
- [ ] After closing the overlay (including after a type-mode session),
      digits and Esc reach the game again — no stuck hook
- [ ] While type mode is active, Alt+Tab and Alt+F4 are intentionally
      captured by the hook — verify the mouse still works as an escape
      hatch (click out of the game/overlay)
- [ ] Typing-disabled path (typing toggle off in Settings) shows the
      needs-label notice instead of opening type mode
- [ ] Exclusive-fullscreen game: the overlay may not draw (documented
      limitation) but hotkey registration and game detection are unaffected
- [ ] Overlay bind is rebindable in Settings, including binding it to a
      plain G-key
- [ ] Digit registration failure (another app already holds a digit
      hotkey) degrades gracefully — no crash, remaining digits still work
      or a clear log entry explains the gap

## 18. History full-view toggle + theme-flash repaint fix

Shipped 2026-07-13. The History button in the bottom bar now toggles the
MAIN view: pressed once it swaps the live event log for a full-pane history
view (Today/All, same grouping, context menu, game editing, live-refreshes
on every new log entry); pressed again — or Esc, or the "Back to log"
button — it returns to the live log. Restore-log-display moved into the
history footer and flips back to the log after restoring.

Same change fixes the ≤5s timer flash only recoloring "the edges": the boot
`<style>` injected by index.html (first-paint anti-flash) is unlayered CSS
and permanently outranked Tailwind's layered `body { bg-background }`, so
the window background never followed runtime theme changes. `applyTheme`
now rewrites that boot style (`#theme-boot-paint`) on every application.
Also fixed latently: the overlay window now forces `#root` transparent too
(the boot style painted it opaque; previously only html/body were
overridden).

**Automated coverage** — `tsc` + `pnpm build` clean (no frontend test
runner in this repo). Human items:

- [ ] History button swaps the main area to the history view; button shows
      pressed/active styling; clicking again returns to the live log with
      the current log entries intact (nothing lost while history was open)
- [ ] Esc and "Back to log" also return to the live log; Esc still closes
      an open context menu / cancels an in-row game edit FIRST (one Esc per
      layer)
- [ ] Save a clip while the history view is open → the new clip appears in
      Today without reopening the view
- [ ] Timer flash (set timer ≤ 10s, let it run to 5): the WHOLE window
      alternates to the contrasting theme each second — background, log
      area, panels, text — and returns exactly to the active theme after
      the flash ends and on expiry
- [ ] Theme switching in Settings still repaints instantly, and app relaunch
      first-paints in the last active theme (no white/dark flash at boot)
- [ ] Overlay window is still fully transparent around the menu card (no
      opaque rectangle behind it)
