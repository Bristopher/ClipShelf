//! In-app update pipeline: MicGuard-style consent flow, Velopack engine.
//!
//! Nothing ever updates silently. The startup check (config `check_updates`,
//! on by default) and the manual check (Settings / tray menu) both end in a
//! dialog that ASKS before anything is downloaded or applied.
//!
//! Two delivery paths:
//! - **Installed (Velopack setup):** `UpdateManager` reads
//!   `releases.win.json` from the GitHub release's `latest/download/` feed,
//!   downloads the nupkg (delta when possible), applies, and restarts.
//! - **Portable exe / dev build:** Velopack can't self-update (no
//!   `Update.exe` alongside), so the check falls back to the GitHub API and
//!   a found update opens the releases page for a manual download — same
//!   fallback MicGuard uses when its in-place swap fails.

use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use velopack::{sources::HttpSource, UpdateCheck, UpdateManager};

pub const GITHUB_REPO: &str = "Bristopher/GKeyMover";

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
        .set("User-Agent", "GKeyMover-updater")
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

fn info_dialog(app: &AppHandle, title: &str, message: &str) {
    app.dialog()
        .message(message)
        .title(title)
        .kind(MessageDialogKind::Info)
        .blocking_show();
}

/// "Update now / Later" consent dialog. Blocking — call from a worker thread.
fn ask_update(app: &AppHandle, remote: &str, action: &str) -> bool {
    app.dialog()
        .message(format!(
            "GKey Mover {remote} is available (you have v{CURRENT_VERSION}).\n\n{action}"
        ))
        .title("GKey Mover — Update available")
        .kind(MessageDialogKind::Info)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Update now".to_string(),
            "Later".to_string(),
        ))
        .blocking_show()
}

/// Run one update check. `quiet` (startup): stay silent unless an update is
/// actually available. Manual (tray/Settings): always ends in a dialog —
/// up-to-date, update offer, or the error.
///
/// Blocking (network + dialogs) — always spawned on a worker thread.
pub fn check(app: &AppHandle, quiet: bool) {
    match UpdateManager::new(HttpSource::new(feed_url()), None, None) {
        Ok(um) => check_velopack(app, &um, quiet),
        Err(e) => {
            // Portable exe or dev build — no Velopack install to update.
            log::info!("updater: velopack unavailable ({e}); using GitHub API fallback");
            check_fallback(app, quiet);
        }
    }
}

fn check_velopack(app: &AppHandle, um: &UpdateManager, quiet: bool) {
    match um.check_for_updates() {
        Ok(UpdateCheck::UpdateAvailable(info)) => {
            let remote = format!("v{}", info.TargetFullRelease.Version);
            if !ask_update(app, &remote, "Update and restart now?") {
                return;
            }
            if let Err(e) = um
                .download_updates(&info, None)
                .and_then(|_| um.apply_updates_and_restart(&info.TargetFullRelease))
            {
                log::warn!("updater: apply failed: {e}");
                info_dialog(
                    app,
                    "GKey Mover — Update failed",
                    &format!(
                        "The update could not be installed automatically.\n\n\
                         Opening the releases page so you can download {remote} manually."
                    ),
                );
                let _ = opener::open_browser(releases_page());
            }
        }
        Ok(_) => {
            if !quiet {
                info_dialog(
                    app,
                    "GKey Mover — Up to date",
                    &format!("You're on the latest version (v{CURRENT_VERSION})."),
                );
            }
        }
        Err(e) => {
            log::info!("updater: check failed: {e}");
            if !quiet {
                info_dialog(
                    app,
                    "GKey Mover — Update check failed",
                    &format!("Could not reach the update feed.\n\n{e}"),
                );
            }
        }
    }
}

fn check_fallback(app: &AppHandle, quiet: bool) {
    match github_latest_tag() {
        Ok(tag) if is_newer(&tag, CURRENT_VERSION) => {
            if ask_update(
                app,
                &tag,
                "This build can't update itself (portable/dev) — open the releases page to download it?",
            ) {
                let _ = opener::open_browser(releases_page());
            }
        }
        Ok(_) => {
            if !quiet {
                info_dialog(
                    app,
                    "GKey Mover — Up to date",
                    &format!("You're on the latest version (v{CURRENT_VERSION})."),
                );
            }
        }
        Err(e) => {
            log::info!("updater: GitHub check failed: {e}");
            if !quiet {
                info_dialog(
                    app,
                    "GKey Mover — Update check failed",
                    &format!("Could not reach GitHub to check for updates.\n\n{e}"),
                );
            }
        }
    }
}

/// Manual "Check for updates" — Settings button and tray menu item.
#[tauri::command]
pub fn manual_update_check(app: AppHandle) {
    std::thread::spawn(move || check(&app, false));
}

/// Startup check, gated on `check_updates` in config. Delayed so the app is
/// fully up (watcher, windows) before any dialog can appear.
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
}
