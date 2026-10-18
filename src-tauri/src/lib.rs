mod commands;
mod error;
mod game;
mod launcher;
mod rosetta;
mod settings;
mod smapi;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let cache_dir = app.path().app_cache_dir()?;
            app.manage(commands::AppState::new(config_dir, cache_dir));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::settings_get,
            commands::settings_set_locale,
            commands::game_detect,
            commands::game_current,
            commands::game_set_path,
            commands::smapi_latest,
            commands::smapi_run,
            commands::game_launch,
            commands::game_stop,
            commands::game_state,
            commands::game_log,
            commands::game_console,
            commands::open_place,
            commands::rosetta_install,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
