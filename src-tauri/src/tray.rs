use tauri::{
    AppHandle, Manager,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let videos_item = MenuItem::with_id(app, "videos_folder", "Video Folder", true, None::<&str>)?;
    let log_item = MenuItem::with_id(app, "log_folder", "Log Folder", true, None::<&str>)?;
    let help_item = MenuItem::with_id(app, "help", "Help", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Exit", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&videos_item, &log_item, &help_item, &quit_item])?;

    TrayIconBuilder::new()
        .tooltip("Gkey Mover v2.0.0")
        .menu(&menu)
        .on_menu_event(|app, event| {
            match event.id().as_ref() {
                "videos_folder" => {
                    if let Ok(config) = crate::config::AppConfig::load() {
                        let _ = opener::open(&config.videos_folder);
                    }
                }
                "log_folder" => {
                    if let Ok(config) = crate::config::AppConfig::load() {
                        let log_path = std::path::PathBuf::from(&config.videos_folder).join("logs");
                        let _ = opener::open(log_path);
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
