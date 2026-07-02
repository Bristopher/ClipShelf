//! Window position/size memory + configurable default open position.
//!
//! The saved layout lives in its own `window_layout.toml` next to the config
//! file — deliberately NOT inside `AppConfig`. The Settings window uses a
//! draft/save model that writes the whole config back on Save; if the layout
//! lived there, a stale draft would clobber whatever the auto-save wrote
//! while Settings was open.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::{Manager, PhysicalPosition, PhysicalSize};

use crate::config::AppConfig;
use crate::state::AppState;

/// Physical (device-pixel) outer position + size of the main window.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct WindowLayout {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Default window size from tauri.conf.json, in logical pixels.
pub const DEFAULT_W: f64 = 900.0;
pub const DEFAULT_H: f64 = 260.0;

pub fn layout_path(config_path: &Path) -> PathBuf {
    config_path
        .parent()
        .map(|p| p.join("window_layout.toml"))
        .unwrap_or_else(|| PathBuf::from("window_layout.toml"))
}

pub fn load(path: &Path) -> Option<WindowLayout> {
    let contents = std::fs::read_to_string(path).ok()?;
    toml::from_str(&contents).ok()
}

pub fn save(path: &Path, layout: &WindowLayout) {
    if let Ok(contents) = toml::to_string(layout) {
        if let Err(e) = std::fs::write(path, contents) {
            log::warn!("Failed to save window layout: {}", e);
        }
    }
}

pub fn clear(path: &Path) {
    let _ = std::fs::remove_file(path);
}

/// True if the layout's top-left corner is on (or near) any connected
/// monitor — guards against restoring onto an unplugged display.
fn layout_visible(layout: &WindowLayout, monitors: &[tauri::Monitor]) -> bool {
    const SLACK: i32 = 64;
    monitors.iter().any(|m| {
        let pos = m.position();
        let size = m.size();
        layout.x >= pos.x - SLACK
            && layout.x < pos.x + size.width as i32
            && layout.y >= pos.y - SLACK
            && layout.y < pos.y + size.height as i32
    })
}

/// Apply the remembered layout if enabled and still on-screen; otherwise the
/// configured default position. Called at startup before the window shows.
pub fn apply_startup_layout(
    window: &tauri::WebviewWindow,
    config: &AppConfig,
    config_path: &Path,
) {
    if config.remember_window_layout {
        if let Some(layout) = load(&layout_path(config_path)) {
            let monitors = window.available_monitors().unwrap_or_default();
            if layout_visible(&layout, &monitors) {
                let _ = window.set_size(tauri::Size::Physical(PhysicalSize {
                    width: layout.width,
                    height: layout.height,
                }));
                let _ = window.set_position(tauri::Position::Physical(PhysicalPosition {
                    x: layout.x,
                    y: layout.y,
                }));
                return;
            }
        }
    }
    apply_default_position(window, config, true);
}

/// Position the window at the configured anchor corner of the configured
/// monitor. `reset_size` also restores the default window size first (so the
/// anchor math uses the size the window will actually have).
pub fn apply_default_position(window: &tauri::WebviewWindow, config: &AppConfig, reset_size: bool) {
    let monitors = window.available_monitors().unwrap_or_default();
    if monitors.is_empty() {
        return;
    }
    // 1-based config index, clamped to what's actually connected.
    let idx = (config.default_monitor.max(1) as usize - 1).min(monitors.len() - 1);
    let monitor = &monitors[idx];

    if reset_size {
        let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize {
            width: DEFAULT_W,
            height: DEFAULT_H,
        }));
    }

    let mpos = monitor.position();
    let msize = monitor.size();
    let wsize = window
        .outer_size()
        .unwrap_or(PhysicalSize {
            width: (DEFAULT_W * monitor.scale_factor()) as u32,
            height: (DEFAULT_H * monitor.scale_factor()) as u32,
        });

    let mw = msize.width as i32;
    let mh = msize.height as i32;
    let ww = wsize.width as i32;
    let wh = wsize.height as i32;

    let (x, y) = match config.default_anchor.as_str() {
        "top-right" => (mpos.x + mw - ww, mpos.y),
        "bottom-left" => (mpos.x, mpos.y + mh - wh),
        "bottom-right" => (mpos.x + mw - ww, mpos.y + mh - wh),
        "center" => (mpos.x + (mw - ww) / 2, mpos.y + (mh - wh) / 2),
        // "top-left" and anything unrecognized.
        _ => (mpos.x, mpos.y),
    };

    let _ = window.set_position(tauri::Position::Physical(PhysicalPosition { x, y }));
}

// Debounced auto-save: every Moved/Resized bumps the generation; only the
// task holding the latest generation actually writes after the quiet period.
static SAVE_GEN: AtomicU64 = AtomicU64::new(0);

pub fn schedule_layout_save(window: &tauri::Window) {
    let gen = SAVE_GEN.fetch_add(1, Ordering::SeqCst) + 1;
    let window = window.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(800)).await;
        if SAVE_GEN.load(Ordering::SeqCst) != gen {
            return; // Superseded by a later move/resize.
        }
        let app = window.app_handle();
        let state = app.state::<AppState>();
        let (remember, path) = {
            let Ok(s) = state.lock() else { return };
            (
                s.config.remember_window_layout,
                layout_path(&s.config_path),
            )
        };
        if !remember {
            return;
        }
        // A minimized window reports a bogus -32000,-32000 position; a
        // hidden window's geometry isn't meaningful either.
        if window.is_minimized().unwrap_or(false) || !window.is_visible().unwrap_or(false) {
            return;
        }
        let (Ok(pos), Ok(size)) = (window.outer_position(), window.outer_size()) else {
            return;
        };
        save(
            &path,
            &WindowLayout {
                x: pos.x,
                y: pos.y,
                width: size.width,
                height: size.height,
            },
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("window_layout.toml");
        let layout = WindowLayout { x: -1920, y: 42, width: 900, height: 260 };
        save(&path, &layout);
        let loaded = load(&path).expect("load failed");
        assert_eq!(loaded.x, -1920);
        assert_eq!(loaded.y, 42);
        assert_eq!(loaded.width, 900);
        assert_eq!(loaded.height, 260);

        clear(&path);
        assert!(load(&path).is_none());
    }

    #[test]
    fn test_layout_path_is_sibling_of_config() {
        let p = layout_path(Path::new("C:/cfg/gkey_config.toml"));
        assert_eq!(p, PathBuf::from("C:/cfg/window_layout.toml"));
    }
}
