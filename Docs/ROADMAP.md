# Gkey Mover v2 — Roadmap

**Last Updated:** 2026-04-25

Living list of pending work. Mirrors session task IDs so they can be cross-referenced. Items are ordered by suggested execution order — verify-first (cheap, de-risks the rest), then build-out, then spec gaps.

**Attribution legend:**
- 🤖 *Claude proposal* — surfaced by Claude during sessions, not yet user-confirmed as required
- 👤 *User request* — explicitly asked for by Chris
- 📋 *From design spec* — pulled from `Docs/specs/2026-04-15-gkey-mover-v2-design.md`

---

## Verify-First (small, unblock the rest)

### #25 🤖 — Verify `MissedTickBehavior::Delay` fix resolves timer insta-expire
After restarting `pnpm tauri dev` with a full kill, record a clip and confirm the countdown now runs the full configured duration instead of expiring instantly. Validates the fix in `src-tauri/src/timer.rs` (commit `0dd4533`).

### #29 🤖 — Smoke-test first-run UX end-to-end
Recent refactor moved first-run to a separate window with custom chrome, lock overlay, beep feedback, and a watcher-restart-on-folder-change fix (commit `72e7da2`). Walk a fresh config through the full flow: launch with empty `videos_folder` → first-run window opens and requests attention → main UI locked with overlay → pick folder in first-run → main unlocks, watcher starts detecting clips without restart needed.

### #27 🤖 — Audit rename flow end-to-end (G4 dialog + `rename_bind`)
`RenameDialog` and `rename_bind` exist. Verify: hotkey opens dialog with current filename; `" - "` prefix auto-prepended; Enter invokes `rename_file`; Escape cancels; window focused on open; "No current file" error path works.

---

## Build-Out (bigger features)

### #26 🤖📋 — Wire OBS WebSocket backend to detect replay-buffer saves
Settings UI already exposes `obs_websocket_enabled` and password, but the `obs_ws.rs` module from spec §5.8 may not be wired. Implement `tokio-tungstenite` client, op:0 → op:1 → op:2 SHA256 auth, listen for `ReplayBufferSaved`, feed `savedReplayPath` into the same pipeline as the file watcher. Spec: `Docs/specs/2026-04-15-gkey-mover-v2-design.md` §5.8.

### #28 🤖📋 — Verify Windows toast notifications fire on clip save/move
`windows_notification_enabled` is in `AppConfig` but `SettingsForm` doesn't expose a toggle for it, and it's unclear whether the Rust side actually emits a notification. Either wire it up (Tauri notification plugin) and add the settings switch, or drop the field.

### #30 👤 — Capture-hidden on-screen notification popups (hide from OBS recordings)
User-requested feature: notification popups visible to the user but invisible in OBS recordings, covering both DXGI Desktop Duplication and Windows Graphics Capture (WGC).

**Approach:** Win32 `SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE)`. The DWM compositor skips flagged windows when building capture surfaces, so they're excluded from BitBlt, PrintWindow, DXGI Desktop Duplication, *and* WGC in one call.

**Requirements:**
- Windows 10 version 2004+ (`WDA_EXCLUDEFROMCAPTURE` was added then — older `WDA_MONITOR` only hides from legacy GDI, not DXGI/WGC).
- Caveat: not DRM. Phone camera still works; useless if DWM compositing is off (rare).

**Tauri v2 sketch:**
1. Spawn a small borderless always-on-top transparent Tauri window for notifications (like existing first-run/settings windows).
2. Get the HWND via `window.hwnd()` on Windows, call `SetWindowDisplayAffinity` through the `windows` or `windows-sys` crate.
3. Wire events (clip saved, move complete, error) to push notifications into that window.
4. Add a settings toggle under Notifications.

**Refs:**
- https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowdisplayaffinity
- https://www.meziantou.net/how-to-exclude-your-windows-app-from-screen-capture-and-recall.htm
- https://blogs.windows.com/windowsdeveloper/2019/09/16/new-ways-to-do-screen-capture/

---

## Spec Gaps 📋 (untracketed, from design spec §11 parity table)

Pulled from `Docs/specs/2026-04-15-gkey-mover-v2-design.md`. The spec marks everything "Planned" so it's not a live tracker — these are inferred from code vs spec.

- **Sleep/resume detection in watcher** (§5.2) — 10s wall-clock delta check between health-check ticks; if exceeded, restart watcher. Mirrors v1's `ResumableObserver`.
- **Black-screen warning for files <6.5MB** (§5.4) — warning sound + red log entry.
- **Multi-monitor positioning** (§6 Theme) — open on secondary monitor if available.
- **Legacy `options.txt` migration** (§8) — probably skip; v1→v2 is a clean break.

---

## Done (recent — see git log for full history)

- Timer insta-expire fix via `MissedTickBehavior::Delay` (`0dd4533`)
- ≤5s flash by inverting theme each second (`1fcff40`)
- Watcher restart on `videos_folder` change (`72e7da2`)
- Tray icon fix — added `.icon()` to `TrayIconBuilder` (`d8d8f4f`)
- `useTimer` snap-back to full duration on expire + re-sync on config change (`ffa896c`)
