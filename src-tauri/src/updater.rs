//! In-app update pipeline: MediaStopper-style in-app UX, Velopack engine.
//!
//! Nothing ever updates silently, and nothing uses native popup dialogs.
//! Results surface in-app instead:
//! - Settings → Updates renders an inline status card from
//!   `check_update_status` and installs via `install_update` (one click,
//!   "Install vX & relaunch" — same pattern as MediaStopper's settings).
//! - The startup/tray check logs to the event log and emits
//!   `update-available` so the main window can show a consent banner.
//!
//! Two delivery paths:
//! - **Installed (Velopack setup):** `UpdateManager` reads
//!   `releases.win.json` from the GitHub release's `latest/download/` feed,
//!   downloads the nupkg (delta when possible), applies, and restarts.
//! - **Portable exe / dev build:** Velopack can't self-update (no
//!   `Update.exe` alongside), so the check falls back to the GitHub API and
//!   installing means opening the releases page for a manual download.

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use velopack::{sources::HttpSource, UpdateCheck, UpdateManager};

use crate::events::{LogCategory, LogLevel};
use crate::state::AppState;

pub const GITHUB_REPO: &str = "Bristopher/ClipShelf";

fn releases_page() -> String {
    format!("https://github.com/{GITHUB_REPO}/releases/latest")
}

/// Velopack update feed: GitHub rewrites `releases/latest/download/<asset>`
/// to the newest release's asset, so this one URL always serves the current
/// `releases.win.json` + nupkg that `build-release.ps1` uploads.
fn feed_url() -> String {
    format!("https://github.com/{GITHUB_REPO}/releases/latest/download/")
}

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Structured check result for the frontend (Settings card + banner).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    /// "update" | "current" | "error"
    pub status: String,
    /// Version we're running, no "v" prefix.
    pub current: String,
    /// Newer version tag ("v2.0.17") when status == "update".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest: Option<String>,
    /// True when this build can install in place (Velopack install); false
    /// for portable/dev, where installing opens the releases page instead.
    pub can_install: bool,
    /// Human-readable detail for status == "error".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl UpdateStatus {
    fn current_version() -> Self {
        Self {
            status: "current".into(),
            current: CURRENT_VERSION.into(),
            latest: None,
            can_install: false,
            message: None,
        }
    }

    fn update(latest: String, can_install: bool) -> Self {
        Self {
            status: "update".into(),
            latest: Some(latest),
            can_install,
            ..Self::current_version()
        }
    }

    fn error(message: String) -> Self {
        Self {
            status: "error".into(),
            message: Some(message),
            ..Self::current_version()
        }
    }
}

/// Parse "2.0.13" / "v2.0.13" into a comparable triple. Returns None for
/// anything that isn't plain x.y.z (pre-release tags won't be offered).
fn parse_version(v: &str) -> Option<(u64, u64, u64)> {
    let v = v.trim().trim_start_matches('v');
    let mut parts = v.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

/// True when `remote` is strictly newer than `current`.
fn is_newer(remote: &str, current: &str) -> bool {
    match (parse_version(remote), parse_version(current)) {
        (Some(r), Some(c)) => r > c,
        _ => false,
    }
}

/// Latest release tag from the GitHub API (fallback path for portable/dev
/// runs where Velopack isn't available).
fn github_latest_tag() -> Result<String, String> {
    let url = format!("https://api.github.com/repos/{GITHUB_REPO}/releases/latest");
    let resp = ureq::get(&url)
        .set("User-Agent", "ClipShelf-updater")
        .set("Accept", "application/vnd.github+json")
        .timeout(std::time::Duration::from_secs(10))
        .call()
        .map_err(|e| e.to_string())?;
    let json: serde_json::Value = resp.into_json().map_err(|e| e.to_string())?;
    json.get("tag_name")
        .and_then(|t| t.as_str())
        .map(|t| t.to_string())
        .ok_or_else(|| "no tag_name in latest release".to_string())
}

/// One blocking update check → structured status. Never touches UI.
fn compute_status() -> UpdateStatus {
    match UpdateManager::new(HttpSource::new(feed_url()), None, None) {
        Ok(um) => match um.check_for_updates() {
            Ok(UpdateCheck::UpdateAvailable(info)) => {
                UpdateStatus::update(format!("v{}", info.TargetFullRelease.Version), true)
            }
            Ok(_) => UpdateStatus::current_version(),
            Err(e) => UpdateStatus::error(format!("Could not reach the update feed: {e}")),
        },
        Err(e) => {
            // Portable exe or dev build — no Velopack install to update.
            log::info!("updater: velopack unavailable ({e}); using GitHub API fallback");
            match github_latest_tag() {
                Ok(tag) if is_newer(&tag, CURRENT_VERSION) => UpdateStatus::update(tag, false),
                Ok(_) => UpdateStatus::current_version(),
                Err(e) => UpdateStatus::error(format!("Could not reach GitHub: {e}")),
            }
        }
    }
}

/// Blocking download + apply + restart. Only valid on a Velopack install.
fn do_install() -> Result<(), String> {
    let um = UpdateManager::new(HttpSource::new(feed_url()), None, None)
        .map_err(|e| format!("This build can't update itself: {e}"))?;
    match um.check_for_updates().map_err(|e| e.to_string())? {
        UpdateCheck::UpdateAvailable(info) => {
            um.download_updates(&info, None).map_err(|e| e.to_string())?;
            um.apply_updates_and_restart(&info.TargetFullRelease)
                .map_err(|e| e.to_string())
        }
        _ => Err("No update available — you're already up to date.".into()),
    }
}

/// Append to the in-app event log and push it to the main window.
fn log_line(app: &AppHandle, level: LogLevel, message: String) {
    let state = app.state::<AppState>();
    let entry = {
        let mut s = state.lock().unwrap();
        s.logger.log(level, message, LogCategory::System)
    };
    let _ = app.emit("log-entry", &entry);
}

/// Run one check and surface the result in-app. `quiet` (startup): stay
/// silent unless an update is actually available. Manual (tray): always
/// leaves an event-log line. An available update additionally emits
/// `update-available` so the main window shows its consent banner.
///
/// Blocking (network) — always spawned on a worker thread.
pub fn check(app: &AppHandle, quiet: bool) {
    let status = compute_status();
    match status.status.as_str() {
        "update" => {
            let latest = status.latest.clone().unwrap_or_default();
            log_line(
                app,
                LogLevel::Info,
                format!(
                    "Update {latest} available (you have v{CURRENT_VERSION}) — install from the banner or Settings → Updates"
                ),
            );
            let _ = app.emit("update-available", &status);
        }
        "current" if !quiet => {
            log_line(
                app,
                LogLevel::Info,
                format!("You're on the latest version (v{CURRENT_VERSION})"),
            );
        }
        "error" if !quiet => {
            log_line(
                app,
                LogLevel::Warning,
                format!(
                    "Update check failed — {}",
                    status.message.as_deref().unwrap_or("unknown error")
                ),
            );
        }
        _ => {}
    }
}

/// Settings → Updates "Check for updates" button: returns the structured
/// result for the inline card.
#[tauri::command]
pub async fn check_update_status() -> UpdateStatus {
    tauri::async_runtime::spawn_blocking(compute_status)
        .await
        .unwrap_or_else(|e| UpdateStatus::error(format!("update check panicked: {e}")))
}

/// One-click "Install vX & relaunch". On failure the releases page opens as
/// a manual fallback and the error string goes back to the caller's card.
#[tauri::command]
pub async fn install_update() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(|| {
        do_install().inspect_err(|e| {
            log::warn!("updater: install failed: {e}");
            let _ = opener::open_browser(releases_page());
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Open the GitHub releases page (portable/dev "install" path).
#[tauri::command]
pub fn open_releases_page() -> Result<(), String> {
    opener::open_browser(releases_page()).map_err(|e| e.to_string())
}

/// Manual "Check for updates" — tray menu item. Result lands in the event
/// log / banner, so bring the main window up so it's actually visible.
#[tauri::command]
pub fn manual_update_check(app: AppHandle) {
    std::thread::spawn(move || {
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.show();
            let _ = w.unminimize();
            let _ = w.set_focus();
        }
        check(&app, false);
    });
}

/// Startup check, gated on `check_updates` in config. Delayed so the app is
/// fully up (watcher, windows) before the banner/log line can appear.
pub fn spawn_startup_check(app: AppHandle, enabled: bool) {
    if !enabled {
        return;
    }
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(5));
        check(&app, true);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_version_plain_and_v_prefixed() {
        assert_eq!(parse_version("2.0.13"), Some((2, 0, 13)));
        assert_eq!(parse_version("v2.1.0"), Some((2, 1, 0)));
        assert_eq!(parse_version(" v10.20.30 "), Some((10, 20, 30)));
    }

    #[test]
    fn test_parse_version_rejects_garbage() {
        assert_eq!(parse_version("2.0"), None);
        assert_eq!(parse_version("2.0.13.1"), None);
        assert_eq!(parse_version("2.0.13-beta"), None);
        assert_eq!(parse_version("latest"), None);
    }

    #[test]
    fn test_is_newer_orders_semver_not_lexically() {
        assert!(is_newer("v2.0.14", "2.0.13"));
        assert!(is_newer("v2.1.0", "2.0.99"));
        assert!(is_newer("v10.0.0", "9.9.9")); // lexical compare would fail this
        assert!(!is_newer("v2.0.13", "2.0.13"));
        assert!(!is_newer("v2.0.12", "2.0.13"));
        assert!(!is_newer("not-a-version", "2.0.13"));
    }

    #[test]
    fn test_update_status_shapes() {
        let s = UpdateStatus::update("v9.9.9".into(), true);
        assert_eq!(s.status, "update");
        assert_eq!(s.latest.as_deref(), Some("v9.9.9"));
        assert!(s.can_install);
        let c = UpdateStatus::current_version();
        assert_eq!(c.status, "current");
        assert_eq!(c.current, CURRENT_VERSION);
        let e = UpdateStatus::error("boom".into());
        assert_eq!(e.status, "error");
        assert_eq!(e.message.as_deref(), Some("boom"));
    }
}
