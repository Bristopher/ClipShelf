use tauri::{
    AppHandle, Manager, Wry,
    menu::{CheckMenuItem, Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

/// Tray menu items that other code needs to poke (managed in app state).
pub struct TrayItems {
    pub pause_item: CheckMenuItem<Wry>,
}

/// The videos folder from the managed AppState (the live, %APPDATA%-backed
/// config). Empty string if unset or the lock is unavailable.
fn live_videos_folder(app: &AppHandle) -> String {
    app.try_state::<crate::state::AppState>()
        .and_then(|s| s.lock().ok().map(|s| s.config.videos_folder.clone()))
        .unwrap_or_default()
}

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let pause_item = CheckMenuItem::with_id(
        app,
        "pause_watching",
        "Pause Watching",
        true,
        false,
        None::<&str>,
    )?;
    let videos_item = MenuItem::with_id(app, "videos_folder", "Video Folder", true, None::<&str>)?;
    let log_item = MenuItem::with_id(app, "log_folder", "Log Folder", true, None::<&str>)?;
    let help_item = MenuItem::with_id(app, "help", "Help", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Exit", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[&pause_item, &videos_item, &log_item, &help_item, &quit_item],
    )?;
    app.manage(TrayItems {
        pause_item: pause_item.clone(),
    });

    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or("default window icon not available")?;

    TrayIconBuilder::new()
        .icon(icon)
        .tooltip(format!("GKey Mover v{}", env!("CARGO_PKG_VERSION")))
        .menu(&menu)
        .on_menu_event(|app, event| {
            match event.id().as_ref() {
                "pause_watching" => {
                    // CheckMenuItem toggles itself; read the new state and
                    // route through the same logic as the UI toggle.
                    let paused = app
                        .try_state::<TrayItems>()
                        .map(|t| t.pause_item.is_checked().unwrap_or(false))
                        .unwrap_or(false);
                    let state = app.state::<crate::state::AppState>();
                    let channels = app.state::<crate::state::ChannelState>();
                    let _ = crate::commands::set_watch_paused(
                        paused,
                        state,
                        channels,
                        app.clone(),
                    );
                }
                // Read the folder from live state — AppConfig::load() reads
                // the legacy exe-adjacent path, not the managed %APPDATA%
                // config, so it returns stale or default (empty) settings.
                "videos_folder" => {
                    let folder = live_videos_folder(app);
                    if !folder.is_empty() {
                        let _ = opener::open(&folder);
                    }
                }
                "log_folder" => {
                    let folder = live_videos_folder(app);
                    if !folder.is_empty() {
                        let _ = opener::open(std::path::PathBuf::from(&folder).join("logs"));
                    }
                }
                "help" => {
                    let _ = opener::open_browser("https://github.com");
                }
                "quit" => {
                    app.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)?;

    Ok(())
}
