mod archive;
mod backups;
mod commands;
mod deps;
mod error;
mod game;
mod launcher;
mod library;
mod manifest;
mod mods_commands;
mod profiles;
mod rosetta;
mod settings;
mod smapi;
mod updates;
mod version;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let data_dir = app.path().app_data_dir()?;
            let cache_dir = app.path().app_cache_dir()?;
            app.manage(commands::AppState::new(config_dir, data_dir, cache_dir));
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
            mods_commands::library_get,
            mods_commands::library_install,
            mods_commands::library_import,
            mods_commands::library_remove,
            mods_commands::library_toggle,
            mods_commands::library_open,
            mods_commands::profile_create,
            mods_commands::profile_rename,
            mods_commands::profile_delete,
            mods_commands::profile_activate,
            mods_commands::updates_check,
            mods_commands::mod_update,
            mods_commands::mod_rollback,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
