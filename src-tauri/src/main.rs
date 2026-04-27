// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Tauri's NSIS template, when invoked by the updater (passive + /R),
    // calls nsis_tauri_utils::RunAsUser from .onInstSuccess to relaunch
    // the new binary BEFORE the installer process has actually exited.
    // The newly-launched app then races the installer, holding handles
    // on files the installer is still finalizing — install fails or the
    // user sees the app pop up mid-install.
    //
    // Fix: at the very top of main(), if our parent is the NSIS installer
    // we block on WaitForSingleObject until it exits. We've loaded nothing
    // yet (no Tauri builder, no backend, no watcher), so we hold no
    // locks. Once the installer exits, fall through to normal startup —
    // same process, no helper, no detached cmd, no second launch.
    #[cfg(windows)]
    wait_for_installer_parent();

    gkey_mover_v2_lib::run()
}

/// If our parent process is an NSIS installer for this app, block until
/// it exits before returning. Returns immediately (no wait) for any
/// other parent — normal launches, manual installer runs, etc.
#[cfg(windows)]
fn wait_for_installer_parent() {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, WaitForSingleObject, INFINITE, PROCESS_SYNCHRONIZE,
    };

    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return;
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

        let mut parent_is_installer = false;
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
                        // Tauri NSIS pattern: <product>_<version>_<arch>-setup.exe
                        if name.to_string_lossy().to_lowercase().ends_with("-setup.exe") {
                            parent_is_installer = true;
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

        if !parent_is_installer {
            return;
        }
        let ppid = match parent_pid {
            Some(p) => p,
            None => return,
        };

        // PROCESS_SYNCHRONIZE is the minimum access right needed for
        // WaitForSingleObject on a process handle.
        let parent_handle = OpenProcess(PROCESS_SYNCHRONIZE, 0, ppid);
        if parent_handle.is_null() {
            return;
        }
        WaitForSingleObject(parent_handle, INFINITE);
        CloseHandle(parent_handle);
    }
}
