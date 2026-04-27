// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Tauri's NSIS template, when invoked by the updater (passive + /R),
    // calls nsis_tauri_utils::RunAsUser from .onInstSuccess to relaunch
    // the new binary BEFORE the installer process has actually exited.
    // The newly-launched app then races the installer, holding handles
    // on files the installer is still finalizing — install fails or the
    // user sees the app pop up mid-install. We can't override the
    // template, but we can detect the case at the very top of main()
    // and exit before anything initializes. To preserve the updater's
    // "open the app after update" UX, we spawn a fully-detached helper
    // that waits a few seconds for the installer to exit, then launches
    // a fresh instance.
    #[cfg(windows)]
    if launched_by_installer() {
        spawn_delayed_relaunch();
        std::process::exit(0);
    }

    gkey_mover_v2_lib::run()
}

/// Returns true if our parent process looks like an in-progress NSIS
/// installer for this app (e.g. "GKey Mover_2.0.6_x64-setup.exe" running
/// from %TEMP% during an update). Once the installer exits its handle is
/// gone, so a manual launch after install completes returns false and
/// boots normally.
#[cfg(windows)]
fn launched_by_installer() -> bool {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return false;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

        let our_pid = std::process::id();
        let mut parent_pid: Option<u32> = None;

        if Process32FirstW(snapshot, &mut entry) != 0 {
            loop {
                if entry.th32ProcessID == our_pid {
                    parent_pid = Some(entry.th32ParentProcessID);
                    break;
                }
                if Process32NextW(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }

        let mut is_installer = false;
        if let Some(ppid) = parent_pid {
            // Re-snapshot to find the parent entry. (Snapshots are static.)
            let mut entry2: PROCESSENTRY32W = std::mem::zeroed();
            entry2.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            if Process32FirstW(snapshot, &mut entry2) != 0 {
                loop {
                    if entry2.th32ProcessID == ppid {
                        let len = entry2
                            .szExeFile
                            .iter()
                            .position(|&c| c == 0)
                            .unwrap_or(entry2.szExeFile.len());
                        let name = std::ffi::OsString::from_wide(&entry2.szExeFile[..len]);
                        let lower = name.to_string_lossy().to_lowercase();
                        // Tauri NSIS installer binaries follow the pattern
                        // "<product>_<version>_<arch>-setup.exe". Match on
                        // "-setup.exe" suffix to be specific (not just
                        // "setup" which would catch unrelated tools).
                        if lower.ends_with("-setup.exe") {
                            is_installer = true;
                        }
                        break;
                    }
                    if Process32NextW(snapshot, &mut entry2) == 0 {
                        break;
                    }
                }
            }
        }

        CloseHandle(snapshot);
        is_installer
    }
}

/// Spawn a detached cmd.exe that sleeps for a few seconds (long enough
/// for the NSIS installer to fully exit and release any handles), then
/// launches a new instance of this app via `start`. The helper outlives
/// our own exit because DETACHED_PROCESS gives it no parent dependency.
#[cfg(windows)]
fn spawn_delayed_relaunch() {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return,
    };
    // Single /c command: wait, then `start "" "<exe>"` to launch fully
    // detached from the cmd shell. raw_arg avoids Rust's auto-escaping
    // mangling the embedded quotes.
    let cmd = format!(
        r#"/c timeout /t 4 /nobreak >nul && start "" "{}""#,
        exe.display()
    );
    let _ = std::process::Command::new("cmd.exe")
        .raw_arg(cmd)
        .creation_flags(DETACHED_PROCESS | CREATE_NO_WINDOW)
        .spawn();
}
