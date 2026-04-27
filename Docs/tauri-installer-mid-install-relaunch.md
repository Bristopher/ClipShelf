# Tauri NSIS Updater: "App opens mid-install and breaks the update"

## Symptom

When updating a Tauri v2 Windows app via `tauri-plugin-updater`:

1. Updater downloads the new `*-setup.exe`.
2. Old app gets killed by the installer's `PREINSTALL` hook.
3. Files copy.
4. **Right before the installer's progress bar finishes, the app pops open.**
5. The user sees a window appear and either closes it or watches the
   install fail with a file-lock error.

It does not happen on manual `*-setup.exe` runs (double-click), only
when launched by the updater. Killing the running instance with extra
`taskkill` passes in the `POSTINSTALL` hook does **not** help — the
relaunch happens *after* that hook fires.

## Root cause

The Tauri NSIS template (look at `src-tauri/target/release/nsis/x64/installer.nsi`
after a build) contains this at the end of the install section:

```nsis
Function .onInstSuccess
  ${If} $PassiveMode = 1
  ${OrIf} ${Silent}
    ${GetOptions} $CMDLINE "/R" $R0
    ${IfNot} ${Errors}
      ${GetOptions} $CMDLINE "/ARGS" $R0
      nsis_tauri_utils::RunAsUser "$INSTDIR\${MAINBINARYNAME}.exe" "$R0"
    ${EndIf}
  ${EndIf}
FunctionEnd
```

`tauri-plugin-updater` invokes the installer with `/P` (passive) and
`/R` (re-run after install). When both flags are present,
`.onInstSuccess` calls `nsis_tauri_utils::RunAsUser` to launch the new
binary using the explorer-shell drop-privilege trick. **That call fires
while the installer process is still alive** — between "files copied"
and "installer window closes."

The order is:

```
PREINSTALL hook (your taskkill)
  ↓
Files section (copy binaries)
  ↓
POSTINSTALL hook (your taskkill, registry, shortcuts)
  ↓
.onInstSuccess  ← relaunch happens HERE, you can't override this
  ↓
Installer process exits
```

The `POSTINSTALL` hook's `taskkill` runs before `.onInstSuccess`, so
the process it kills is the *old* one (already gone). The new instance
spawned by `RunAsUser` starts fresh after `POSTINSTALL` already ran,
which is why it survives.

Manual installer runs don't pass `/R`, so `.onInstSuccess` skips the
relaunch — which is why the bug only reproduces via the updater.

## Why naive fixes don't work

| Attempt | Why it fails |
|---|---|
| `MUI_FINISHPAGE_RUN_NOTCHECKED` in hooks | Only affects the GUI Finish-page checkbox. Passive mode skips the Finish page entirely; `.onInstSuccess` is a separate code path. |
| Extra `taskkill` in `POSTINSTALL` | Runs *before* `.onInstSuccess`, so it kills nothing useful. |
| Stripping `/R` from `$CMDLINE` in `POSTINSTALL` | Works to suppress the launch — but then the user never gets the post-update relaunch, which is the whole point of `/R`. You'd have to re-implement the relaunch yourself in NSIS, and any cmd-shell-based delayed-launch trick from inside NSIS is fragile (escaping nested quotes through `Exec` + `cmd /c start ""`). |
| `app.exit(0)` from a Tauri command | Too late — the backend has already booted and grabbed file handles. |

## The fix

Detect at the very top of `fn main()` whether our parent process is the
NSIS installer, and if so:

1. Spawn a fully-detached helper that sleeps a few seconds (long enough
   for the installer to exit) and then launches a fresh instance of the
   app.
2. `process::exit(0)` immediately. **Nothing else runs** — no Tauri
   builder, no backend threads, no file watcher, no window. The
   installer's `RunAsUser` call effectively becomes a no-op.

After the installer exits, the helper's `cmd.exe /c timeout && start`
launches the app cleanly. The new instance has no installer in its
ancestry, so the parent-process check returns false and it boots
normally.

### Why this beats fighting NSIS

- Pure Rust, in your own codebase. No template overrides, no escaping
  hell with `Exec`/`cmd`/`start` inside an NSIS string.
- Works for any future updater quirk that ends up launching the app
  while the installer is still alive — the check is "is my parent the
  installer?", not "is some specific flag set?"
- Manual installer runs naturally don't trigger the path, because they
  don't relaunch the app (parent-check still works, but
  `.onInstSuccess` doesn't fire on manual installs anyway).
- No file system sentinels, no race conditions, no flaky sleeps in the
  main process.

### Implementation

`src-tauri/Cargo.toml` — add the two `windows-sys` features:

```toml
windows-sys = { version = "0.59", features = [
    # ...existing features...
    "Win32_System_Diagnostics_ToolHelp",
    "Win32_System_Threading",
] }
```

`src-tauri/src/main.rs`:

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(windows)]
    if launched_by_installer() {
        spawn_delayed_relaunch();
        std::process::exit(0);
    }
    your_lib::run()
}

#[cfg(windows)]
fn launched_by_installer() -> bool {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW,
        PROCESSENTRY32W, TH32CS_SNAPPROCESS,
    };
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE { return false; }

        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let our_pid = std::process::id();
        let mut parent_pid = None;

        if Process32FirstW(snapshot, &mut entry) != 0 {
            loop {
                if entry.th32ProcessID == our_pid {
                    parent_pid = Some(entry.th32ParentProcessID);
                    break;
                }
                if Process32NextW(snapshot, &mut entry) == 0 { break; }
            }
        }

        let mut is_installer = false;
        if let Some(ppid) = parent_pid {
            let mut e2: PROCESSENTRY32W = std::mem::zeroed();
            e2.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            if Process32FirstW(snapshot, &mut e2) != 0 {
                loop {
                    if e2.th32ProcessID == ppid {
                        let len = e2.szExeFile.iter().position(|&c| c == 0)
                            .unwrap_or(e2.szExeFile.len());
                        let name = std::ffi::OsString::from_wide(&e2.szExeFile[..len]);
                        // Tauri NSIS pattern: <product>_<version>_<arch>-setup.exe
                        if name.to_string_lossy().to_lowercase().ends_with("-setup.exe") {
                            is_installer = true;
                        }
                        break;
                    }
                    if Process32NextW(snapshot, &mut e2) == 0 { break; }
                }
            }
        }
        CloseHandle(snapshot);
        is_installer
    }
}

#[cfg(windows)]
fn spawn_delayed_relaunch() {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let Ok(exe) = std::env::current_exe() else { return };
    let cmd = format!(
        r#"/c timeout /t 4 /nobreak >nul && start "" "{}""#,
        exe.display()
    );
    let _ = std::process::Command::new("cmd.exe")
        .raw_arg(cmd)
        .creation_flags(DETACHED_PROCESS | CREATE_NO_WINDOW)
        .spawn();
}
```

### What about non-Tauri Windows apps?

The same pattern works for **any** Windows app whose installer relaunches
it before exiting (custom InnoSetup, WiX, plain NSIS, etc.). The only
thing to tweak is the installer-name detection — adjust the
`ends_with("-setup.exe")` check to match your installer's output name.

For installers that pass an explicit "I just installed you" CLI flag
(e.g. Velopack's `--veloapp-install`/`--veloapp-updated`/`--veloapp-obsolete`),
prefer matching on the flag instead of the parent-process name — it's
more deterministic. The flow is identical: detect → `process::exit(0)`
before any backend boots → optionally spawn a detached helper to
relaunch later.

## Verifying the fix

1. Build a release installer: `pnpm tauri build`.
2. Run the previous version, leave it open in the tray.
3. Trigger an update through the in-app updater UI.
4. Expected: progress bar fills, installer window closes, ~4 seconds
   later the new app launches. **No mid-install window flash, no
   "install failed" dialog.**
5. Manual sanity check: double-click the new `*-setup.exe`. Expected:
   normal install, no app launch from the installer (the user clicks
   "Run app" on Finish page if they want it).

## Diagnostic tips

- If you suspect the installer-relaunch path is firing, check
  `target/release/nsis/x64/installer.nsi` for the `.onInstSuccess`
  block. The `nsis_tauri_utils::RunAsUser` call is the smoking gun.
- To capture the parent process name during install for confirmation,
  temporarily log it to a file in the early `main()` check before
  exiting. The Tauri NSIS installer is named e.g. `MyApp_2.0.6_x64-setup.exe`.
- If the helper relaunch isn't firing, confirm `cmd.exe`'s `timeout`
  command exists in the user's PATH (it ships with Windows since
  Vista, so this is virtually guaranteed). The 4-second delay can be
  tuned — anything ≥ 2 seconds is generally safe.
