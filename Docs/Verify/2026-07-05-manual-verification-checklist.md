# Manual Verification Checklist

Everything code-verified (tests/tsc/build) but not yet exercised in the
running app. Work through this in `pnpm tauri dev` first; re-check the
release-build items (§9) after `.\build-release.ps1`.

Covers the 2026-07-02 batch (undo, pause, clickable log, autostart, window
layout) and the 2026-07-05 batch (stability round 2, toasts, status pills,
settings UX). Check items off as you go.

## 1. Toasts / error surface
- [ ] Click a G-key button with no clip detected → small toast appears
      bottom-right (NOT the full-screen red error page)
- [ ] Toast auto-dismisses after ~6s; X button dismisses immediately
- [ ] Mash the same failing action → one toast (duplicates collapse), max 4 stacked
- [ ] Undo with nothing to undo → "Nothing to undo" appears in the log (info, no crash)
- [ ] Delete/move a clip in Explorer, then click its log entry → toast
      "File no longer exists at this location"
- [ ] Settings → Save → green "Settings saved" toast (no alert() popup)

## 2. Hotkeys
- [ ] Set a bind that's taken (e.g. bind the same combo in Discord first, or
      use one Windows owns like Ctrl+Alt+Del-adjacent combos) → Save → error
      toast + red log entry naming the bind and action
- [ ] In Settings, give two actions the same key → red "Same key as X — only
      one of them will work" note under both inputs
- [ ] Set undo hotkey (e.g. Ctrl+Alt+Z), move a clip with G1, press it → clip
      returns to original location, log shows "Undo: X → Y"
- [ ] Undo button in bottom bar does the same
- [ ] G1–G3 hotkeys still move clips; rename hotkey opens the dialog

## 3. Watcher status + pause
- [ ] On launch with a folder configured, bottom bar shows "Watching"
      immediately (not "Stopped" — this was the stale-on-mount bug)
- [ ] Click Watching → turns amber "Paused"; drop a test video in the folder
      → app ignores it completely (no log entry, no sound)
- [ ] Tray "Pause Watching" checkbox mirrors the UI toggle both directions
- [ ] Click Paused → resumes, new files detected again
- [ ] Clear the videos folder in Settings → button shows red "Stopped";
      clicking it with a folder set restarts watching

## 4. OBS WebSocket status
- [ ] With OBS integration enabled and OBS running: green pill "Connected" in
      Settings OBS section + green dot "OBS" in bottom bar
- [ ] Start the app BEFORE OBS → pill shows amber Connecting/Reconnecting,
      flips green on its own after OBS starts (within ~30s)
- [ ] Wrong password → error toast "OBS WebSocket auth failed" + red log entry
- [ ] Close OBS → dot goes red "Disconnected"; reopen → reconnects
- [ ] Disable the integration → OBS dot disappears from the bottom bar
- [ ] Save a replay via OBS → clip appears ONCE in the log (no double entry /
      double sound — dedup race fix)

## 5. Event log
- [ ] Hover a clip entry → tooltip appears after ~0.4s; Click reveals in
      Explorer, Ctrl+Click plays in the default player (tooltip bolds the
      action matching whether Ctrl is held)
- [ ] Scroll UP in a busy log, let a new entry arrive → view does NOT jump to
      the bottom; scroll back down → auto-follow resumes
- [ ] Wipe / Restore still work; restored entries stay clickable

## 6. Sidebar
- [ ] G1–G3 buttons show your configured folder names (not "!!"/"CHKD"/"!!!");
      rename a folder in Settings → tags update after Save
- [ ] Hovering a G-key button shows "Move current clip to …" tooltip

## 7. Rename dialog
- [ ] Open via hotkey/G4 with a clip present → typing shows a live
      "→ OriginalName - yourtext.mp4" preview
- [ ] Type an illegal char (e.g. `?` or `/`) → red warning, Rename disabled
- [ ] Empty input → Rename disabled; Enter submits when valid

## 8. Settings window
- [ ] Ctrl+S saves when dirty (button flashes to disabled, toast confirms)
- [ ] Sounds section: pick a custom .mp3/.wav for clip-save and error sounds
      → plays your file; X button reverts to the default sound
- [ ] Autostart toggle: enable → entry appears in Task Manager > Startup apps;
      disable → it's gone
- [ ] Window section: move/resize main window, quit (Ctrl+click X), relaunch →
      restores position/size. Reset button → snaps to configured
      monitor/anchor default
- [ ] Tray → "Video Folder" and "Log Folder" open the CORRECT folders (this
      read a stale config path before)
- [ ] Tray icon tooltip shows the real current version

## 9. Release build (`.\build-release.ps1`)
- [ ] Script suggests the right next version (max of Releases folders and
      tauri.conf.json, +1 patch)
- [ ] After build: Settings footer + title-bar hover + tray tooltip all show
      the new version
- [ ] Installer upgrade over the previous version keeps: config (folders,
      binds, themes), window layout, autostart state
- [ ] Second launch of the exe just focuses the existing window (single-instance)

## 10. Config durability (atomic-save fix — optional but quick)
- [ ] Save settings, kill the app from Task Manager immediately after,
      relaunch → settings intact (no silent reset to defaults)
- [ ] `%APPDATA%\com.cbuzi.gkey-mover-v2\` contains `gkey_config.toml` and
      `window_layout.toml`, and no leftover `*.toml.tmp` files after saves
