# Feature Batch Verification (2026-07-10)

Manual checks for the diagnostics/shortcuts/drag-drop/MRU/filter/stats/
first-run-OBS batch (`Docs/specs/2026-07-10-feature-batch-design.md`).
Everything passed cargo tests (51), tsc, and pnpm build — none of it has
run live yet. Work through in `pnpm tauri dev`. The older
`2026-07-05-manual-verification-checklist.md` still applies too if you
haven't done it.

## 1. Diagnostics popover
- [ ] Activity icon at the right of the bottom bar opens the popover;
      click-outside and Esc close it
- [ ] Shows version, watcher status (+ restart count), OBS status, clips
      folder, config path
- [ ] Clips folder row opens the folder; Config row reveals
      gkey_config.toml in Explorer
- [ ] "Restart watcher" button → log shows watcher restart, restart count
      bumps on next open
- [ ] Shortcut cheat sheet listed at the bottom

## 2. In-app shortcuts (main window focused)
- [ ] `Del` wipes the log, `Ctrl+Z` undoes the last move/rename,
      `Ctrl+,` opens Settings, `Ctrl+F` toggles the log filter
- [ ] None of them fire while typing in the rename dialog or filter box

## 3. Drag-and-drop
- [ ] Drag a video from Explorer over the window → dashed "Drop to select
      & rename" overlay on the log; hovering G1–G3 highlights that button
      (ring) and hides the overlay
- [ ] Drop on G2 → file moves to the G2 folder (collision-safe, undo works,
      count badge appears on the button, move sound if enabled)
- [ ] Drop SEVERAL videos on G2 → all of them move; "Sorted N clips" toast;
      if one was locked/missing → "N couldn't be moved" toast + log entries
- [ ] Drop on the log (or G4) → rename dialog opens with that file's name;
      renaming works; the file also became the "current clip" (multi-drop
      here uses the first video, info toast says so)
- [ ] Drop a .txt → error toast, nothing happens

## 4. Rename MRU
- [ ] Rename a clip with "clutch" → next dialog open shows a "clutch" chip;
      clicking it fills the input
- [ ] Chips dedupe case-insensitively, newest first, max 8
- [ ] MRU survives app restart (persisted in gkey_config.toml)
- [ ] Rename with "scrim {date}" → file gets the actual date (preview showed
      it too); the MRU chip still says "scrim {date}" (raw template kept)
- [ ] "{time}" expands to HH.MM (dots, not colons)
- [ ] With Settings open (dirty), rename a clip, then Save Settings → the
      new MRU chip is NOT lost

## 5. Log filter + context menu + empty state
- [ ] Ctrl+F opens the filter bar focused; text narrows entries live;
      level chips (info/success/warning/error) filter; "n / m" count right
- [ ] Esc in the filter box (or X) closes it and the full log returns
- [ ] Right-click a clip entry → menu: Reveal, Play, Copy path, Copy
      filename — all four work (paste to confirm copies)
- [ ] Empty log shows watcher status + folder + "press <bind> in-game"
      hint instead of bare "Waiting for events..."

## 6. Per-G-key stats
- [ ] After sorting clips, G-key buttons show a count badge (top-right)
- [ ] Hovering a badged button ~0.5s shows the flyout: folder name, "N
      today", last ≤5 clips; clicking a clip reveals it in Explorer
- [ ] Restart the app same day → badges keep today's counts
      (gkey_stats.toml next to the config); flyout list is empty until the
      first move of the new session (recent list is session-only)
- [ ] `%APPDATA%\com.cbuzi.gkey-mover-v2\gkey_stats.toml` exists with
      date + g1/g2/g3 counts, no leftover .toml.tmp

## 7. First-run OBS section
- [ ] Reset first-run (clear videos_folder in config) → setup window shows
      the optional OBS WebSocket section; toggle reveals the password field
- [ ] Test button with OBS running + right password → green "Connected";
      wrong password → red "Authentication failed"; OBS closed → red
      "Couldn't reach OBS" (within ~5s)
- [ ] Finishing with OBS enabled + correct password → main window OBS dot
      goes green shortly after
