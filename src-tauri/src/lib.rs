mod config;
mod events;
mod logger;
mod mover;
mod sound;
mod state;
mod timer;
mod watcher;

use state::create_app_state;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_state = create_app_state();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(app_state)
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
