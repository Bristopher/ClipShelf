// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Velopack hooks must run before ANYTHING else: during install/update/
    // uninstall Velopack relaunches the exe with special arguments
    // (--veloapp-*) and this call handles them and exits. On a normal launch
    // it's a no-op.
    velopack::VelopackApp::build().run();

    // Tauri's NSIS template, when invoked by the updater (passive + /R),
    // calls nsis_tauri_utils::RunAsUser from .onInstSuccess to relaunch
    // the new binary BEFORE the installer process has fully exited. If
    // the new app boots its backend during that window it races the
    // installer's final cleanup and the install fails.
    //
    // Fix: at the very top of main(), detect "I was launched by our own
    // installer" and exit immediately. Nothing initializes — no Tauri
    // builder, no watcher, no window — so the installer can finish
    // cleanly. Tradeoff: the user has to launch the app themselves
    // after the update completes (Start menu, pinned shortcut, tray
    // icon). We tried WaitForSingleObject on the parent installer to
    // get a free auto-relaunch — it deadlocked because nsis_tauri_utils
    // ::RunAsUser blocks the installer thread on the spawned child.
    #[cfg(windows)]
    if launched_by_installer() {
        std::process::exit(0);
    }

    gkey_mover_v2_lib::run()
}

/// True if our parent process looks like the Tauri NSIS installer for
/// this app — files matching `*-setup.exe`, the pattern Tauri uses for
/// `<product>_<version>_<arch>-setup.exe`. After the installer exits
/// the parent handle is gone, so a manual launch (Start menu, pinned
/// shortcut, etc.) returns false and boots normally.
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
            let mut e2: PROCESSENTRY32W = std::mem::zeroed();
            e2.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            if Process32FirstW(snapshot, &mut e2) != 0 {
                loop {
                    if e2.th32ProcessID == ppid {
                        let len = e2
                            .szExeFile
                            .iter()
                            .position(|&c| c == 0)
                            .unwrap_or(e2.szExeFile.len());
                        let name = std::ffi::OsString::from_wide(&e2.szExeFile[..len]);
                        if name.to_string_lossy().to_lowercase().ends_with("-setup.exe") {
                            is_installer = true;
                        }
                        break;
                    }
                    if Process32NextW(snapshot, &mut e2) == 0 {
                        break;
                    }
                }
            }
        }
        CloseHandle(snapshot);
        is_installer
    }
}
