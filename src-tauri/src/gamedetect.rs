//! Foreground-app detection for clip labeling. Pure classification is
//! separated from the unsafe Win32 snapshot so the naming rules are unit-
//! testable. Snapshot is taken at save-clip-bind press (user is in-game at
//! that instant) with a fallback at file-creation (see lib.rs wiring).

use crate::config::GameOverride;
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct ForegroundApp {
    pub exe_stem: String,
    pub product_name: Option<String>,
    pub title: String,
    pub fullscreen: bool,
}

#[derive(Debug, Clone)]
pub struct GameSnapshot {
    pub label: String,
    pub exe_stem: String,
    pub taken_at: SystemTime,
}

/// Naming rules (spec order): override map → fullscreen game name →
/// Desktop-<app>. Product name from exe version info beats window title
/// beats exe stem, skipping blank values.
pub fn classify(app: &ForegroundApp, overrides: &[GameOverride]) -> String {
    let lower = app.exe_stem.to_lowercase();
    if let Some(o) = overrides.iter().find(|o| o.exe.to_lowercase() == lower) {
        return o.name.clone();
    }
    let best = [app.product_name.as_deref(), Some(app.title.as_str()), Some(app.exe_stem.as_str())]
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|s| !s.is_empty())
        .unwrap_or("Unknown")
        .to_string();
    if app.fullscreen {
        best
    } else {
        // Desktop apps prefer the app's name over its window title
        // ("Desktop-Discord", not "Desktop-#general").
        let app_name = [app.product_name.as_deref(), Some(app.exe_stem.as_str())]
            .into_iter()
            .flatten()
            .map(str::trim)
            .find(|s| !s.is_empty())
            .unwrap_or("Unknown");
        format!("Desktop-{}", app_name)
    }
}

/// Win32 snapshot of the current foreground window. Any failure → None
/// (caller records the clip without a game label). Never panics.
pub fn snapshot_foreground(overrides: &[GameOverride]) -> Option<GameSnapshot> {
    let raw = read_foreground()?;
    Some(GameSnapshot {
        label: classify(&raw, overrides),
        exe_stem: raw.exe_stem,
        taken_at: SystemTime::now(),
    })
}

/// Win32 snapshot of the current foreground window. Any failure → None.
/// Never panics.
#[cfg(windows)]
fn read_foreground() -> Option<ForegroundApp> {
    use windows_sys::Win32::Foundation::{CloseHandle, RECT};
    use windows_sys::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId,
    };

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return None;
        }

        // Window title
        let mut title_buf = [0u16; 512];
        let title_len = GetWindowTextW(hwnd, title_buf.as_mut_ptr(), title_buf.len() as i32);
        let title = String::from_utf16_lossy(&title_buf[..title_len.max(0) as usize]);

        // Exe path via the owning process
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 {
            return None;
        }
        let hproc = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if hproc.is_null() {
            return None;
        }
        let mut path_buf = [0u16; 1024];
        let mut path_len = path_buf.len() as u32;
        let ok = QueryFullProcessImageNameW(hproc, 0, path_buf.as_mut_ptr(), &mut path_len);
        CloseHandle(hproc);
        if ok == 0 {
            return None;
        }
        let exe_path = String::from_utf16_lossy(&path_buf[..path_len as usize]);
        let exe_stem = std::path::Path::new(&exe_path)
            .file_stem()?
            .to_string_lossy()
            .to_string();

        // Fullscreen test: window rect covers its monitor's full rect —
        // catches exclusive fullscreen AND borderless-windowed in one check.
        let mut wrect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        if GetWindowRect(hwnd, &mut wrect) == 0 {
            return None;
        }
        let hmon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut mi: MONITORINFO = std::mem::zeroed();
        mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(hmon, &mut mi) == 0 {
            return None;
        }
        let m = mi.rcMonitor;
        let fullscreen = wrect.left <= m.left
            && wrect.top <= m.top
            && wrect.right >= m.right
            && wrect.bottom >= m.bottom;

        Some(ForegroundApp {
            exe_stem,
            product_name: product_name_of(&exe_path),
            title,
            fullscreen,
        })
    }
}

#[cfg(not(windows))]
fn read_foreground() -> Option<ForegroundApp> {
    None
}

/// Read ProductName (fallback FileDescription) from the exe's version info.
#[cfg(windows)]
fn product_name_of(exe_path: &str) -> Option<String> {
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
    };
    let wide: Vec<u16> = exe_path.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let mut handle = 0u32;
        let size = GetFileVersionInfoSizeW(wide.as_ptr(), &mut handle);
        if size == 0 {
            return None;
        }
        let mut data = vec![0u8; size as usize];
        if GetFileVersionInfoW(wide.as_ptr(), 0, size, data.as_mut_ptr() as _) == 0 {
            return None;
        }

        // Translation table → first language, then the two string values.
        let mut cp_ptr: *mut core::ffi::c_void = std::ptr::null_mut();
        let mut cp_len = 0u32;
        let trans_key: Vec<u16> = "\\VarFileInfo\\Translation"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        if VerQueryValueW(data.as_ptr() as _, trans_key.as_ptr(), &mut cp_ptr, &mut cp_len) == 0
            || cp_len < 4
        {
            return None;
        }
        let lang = *(cp_ptr as *const u16);
        let code = *(cp_ptr as *const u16).add(1);

        for field in ["ProductName", "FileDescription"] {
            let key: Vec<u16> = format!("\\StringFileInfo\\{:04x}{:04x}\\{}", lang, code, field)
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let mut val_ptr: *mut core::ffi::c_void = std::ptr::null_mut();
            let mut val_len = 0u32;
            if VerQueryValueW(data.as_ptr() as _, key.as_ptr(), &mut val_ptr, &mut val_len) != 0
                && val_len > 1
            {
                let slice = std::slice::from_raw_parts(val_ptr as *const u16, (val_len - 1) as usize);
                let s = String::from_utf16_lossy(slice).trim().to_string();
                if !s.is_empty() {
                    return Some(s);
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GameOverride;

    fn app(exe: &str, product: Option<&str>, title: &str, fullscreen: bool) -> ForegroundApp {
        ForegroundApp {
            exe_stem: exe.to_string(),
            product_name: product.map(|s| s.to_string()),
            title: title.to_string(),
            fullscreen,
        }
    }

    #[test]
    fn test_override_wins_regardless_of_fullscreen() {
        let overrides = vec![GameOverride { exe: "CS2".into(), name: "Counter-Strike 2".into() }];
        assert_eq!(classify(&app("cs2", Some("Valve CS2"), "cs2", true), &overrides), "Counter-Strike 2");
        assert_eq!(classify(&app("cs2", None, "cs2", false), &overrides), "Counter-Strike 2");
    }

    #[test]
    fn test_fullscreen_prefers_product_name_then_title_then_exe() {
        assert_eq!(classify(&app("r5apex", Some("Apex Legends"), "Apex", true), &[]), "Apex Legends");
        assert_eq!(classify(&app("r5apex", None, "Apex Legends Window", true), &[]), "Apex Legends Window");
        assert_eq!(classify(&app("r5apex", None, "", true), &[]), "r5apex");
    }

    #[test]
    fn test_windowed_gets_desktop_prefix() {
        assert_eq!(classify(&app("discord", Some("Discord"), "#general", false), &[]), "Desktop-Discord");
        assert_eq!(classify(&app("weirdapp", None, "", false), &[]), "Desktop-weirdapp");
    }

    #[test]
    fn test_whitespace_product_name_is_ignored() {
        assert_eq!(classify(&app("game", Some("   "), "Real Title", true), &[]), "Real Title");
    }
}
