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
binary. **That call fires while the installer process is still alive** —
between "files copied" and "installer window closes."

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
spawned by `RunAsUser` survives because nothing kills it after.

Manual installer runs don't pass `/R`, so `.onInstSuccess` skips the
relaunch — which is why the bug only reproduces via the updater.

## Why naive fixes don't work

| Attempt | Why it fails |
|---|---|
| `MUI_FINISHPAGE_RUN_NOTCHECKED` | Only affects the GUI Finish-page checkbox. Passive mode skips the Finish page; `.onInstSuccess` is a separate code path. |
| Extra `taskkill` in `POSTINSTALL` | Runs *before* `.onInstSuccess`, so it kills the wrong instance. |
| Stripping `/R` from `$CMDLINE` in `POSTINSTALL` | Suppresses the launch but you lose the post-update relaunch UX, and re-implementing it in NSIS means escaping nested quotes through `Exec`+`cmd /c start "" "..."` which is ugly and fragile. |
| `app.exit(0)` from a Tauri command | Too late — backend has already booted and grabbed file handles. |
| Spawning a detached `cmd.exe /c timeout && start ""` helper before exiting | Works, but it's a separate process that the OS now has to manage, and it's a 4-second sleep heuristic instead of a deterministic signal. Don't do this. |

## The fix

At the very top of `fn main()`, **before any backend or window initializes**,
check the parent process. If it's the NSIS installer for this app, block
on `WaitForSingleObject(parent_handle, INFINITE)` until the installer
exits, then fall through to normal startup.

Why this works:

- We've loaded **nothing** yet — no Tauri builder, no tokio runtime, no
  watcher, no window, no file handles. The OS holds an EXECUTE mapping
  on our own EXE image, but that doesn't conflict with anything the
  installer is finalizing (registry writes, shortcut creation, its own
  cleanup).
- `WaitForSingleObject` is a deterministic signal, not a sleep heuristic.
  The instant the installer exits, we proceed.
- It's the **same process** that boots the app — no helper, no detached
  cmd, no second launch. The OS process tree stays clean.
- Manual installer runs don't pass `/R` so `.onInstSuccess` doesn't
  relaunch anyway; the wait code path simply never executes.
- Normal launches (Start menu, pinned shortcut, taskbar) have a parent
  like `explorer.exe` — the parent-name check fails fast and the
  function returns in microseconds.

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
    wait_for_installer_parent();

    your_lib::run()
}

#[cfg(windows)]
fn wait_for_installer_parent() {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW,
        PROCESSENTRY32W, TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, WaitForSingleObject, INFINITE, PROCESS_SYNCHRONIZE,
    };

    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE { return; }

        // Pass 1: find our PID's parent.
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

        // Pass 2: look up the parent's exe name.
        let mut parent_is_installer = false;
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
                            parent_is_installer = true;
                        }
                        break;
                    }
                    if Process32NextW(snapshot, &mut e2) == 0 { break; }
                }
            }
        }
        CloseHandle(snapshot);

        if !parent_is_installer { return; }
        let Some(ppid) = parent_pid else { return };

        // PROCESS_SYNCHRONIZE is the minimum right needed for
        // WaitForSingleObject on a process handle.
        let h = OpenProcess(PROCESS_SYNCHRONIZE, 0, ppid);
        if h.is_null() { return; }
        WaitForSingleObject(h, INFINITE);
        CloseHandle(h);
    }
}
```

That's it. No helper process, no sleep, no second launch.

## Sequence with the fix

```
Updater runs setup.exe with /P /R
  ↓
PREINSTALL hook kills old app
  ↓
Files copy
  ↓
POSTINSTALL hook
  ↓
.onInstSuccess → RunAsUser launches new app
  ↓
new app: main() entered
  ↓
new app: parent is "MyApp_2.0.6_x64-setup.exe" → WaitForSingleObject
  ↓
installer window closes; installer process exits
  ↓
new app: WaitForSingleObject returns → falls through to run() → boots normally
```

## What about non-Tauri Windows apps?

The same pattern applies anywhere an installer launches the new binary
before the installer process exits.

- Plain NSIS / WiX / Inno: tweak the `ends_with("-setup.exe")` to match
  your installer's output name (`*-Setup.exe`, `*Installer.exe`, etc.).
- Velopack: prefer matching on the lifecycle CLI flags
  (`--veloapp-install`, `--veloapp-updated`, `--veloapp-obsolete`) and
  exiting with `process::exit(0)` immediately. Velopack passes a
  *different* flag (`--veloapp-firstrun`) for the actual post-install
  launch — so you don't need the wait dance, you just don't catch
  firstrun and it falls through to normal startup. This is the cleanest
  variant of the pattern when the installer cooperates with explicit
  flags.
- Other auto-updaters with no signal flags: the parent-process-wait
  approach in this doc is the universal fallback.

The general rule: **detect the install-time launch as early as possible,
then either exit (if the installer will launch you again later) or wait
(if it won't).** Never let the backend boot during this window.

## Verifying the fix

1. Build a release installer: `pnpm tauri build`.
2. Run the previous version, leave it open in the tray.
3. Trigger an update through the in-app updater UI.
4. Expected: progress bar fills, installer window closes, the new app
   launches in its place. No mid-install window flash. No "install
   failed" dialog.
5. Manual sanity check: double-click the new `*-setup.exe`. Expected:
   normal install, no app launch from the installer (the user clicks
   "Run app" on the Finish page if they want it).

## Diagnostic tips

- If you suspect the installer-relaunch path is firing, check
  `target/release/nsis/x64/installer.nsi` for the `.onInstSuccess`
  block. The `nsis_tauri_utils::RunAsUser` call is the smoking gun.
- To capture the parent process name during install, temporarily log it
  to a file in `wait_for_installer_parent` before the early return. The
  Tauri NSIS installer is named like `MyApp_2.0.6_x64-setup.exe`.
- If `WaitForSingleObject` never returns, the parent isn't actually the
  installer — your detection match is too loose. Tighten the suffix
  check.
