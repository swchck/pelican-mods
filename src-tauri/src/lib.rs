mod archive;
mod backups;
mod checkup;
mod commands;
mod companion;
mod companion_commands;
mod config_commands;
mod deps;
mod downloads;
mod error;
mod game;
mod launcher;
mod library;
mod manifest;
mod mod_config;
mod moddrop;
mod mods_commands;
mod nexus;
mod nexus_commands;
mod perfection;
mod profiles;
mod rosetta;
mod save_backups;
mod saves;
mod saves_commands;
mod settings;
mod smapi;
mod smapi_log;
mod updates;
mod version;

use tauri::Manager;
use tauri_plugin_deep_link::DeepLinkExt;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let data_dir = app.path().app_data_dir()?;
            let cache_dir = app.path().app_cache_dir()?;
            app.manage(commands::AppState::new(config_dir, data_dir, cache_dir));
            companion_commands::start(app.handle());

            // Nexus' "Mod Manager Download" button opens nxm:// links; macOS hands them
            // to the running app, or launches it with the link
            let handle = app.handle().clone();
            let open = move |urls: Vec<url::Url>| {
                for u in urls {
                    if let Err(e) = nexus_commands::handle_nxm(&handle, u.as_str()) {
                        let _ = tauri::Emitter::emit(&handle, "nxm:error", &e);
                    }
                }
            };
            if let Ok(Some(urls)) = app.deep_link().get_current() {
                open(urls);
            }
            app.deep_link().on_open_url(move |event| open(event.urls()));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::settings_get,
            commands::settings_set_locale,
            commands::game_language,
            commands::settings_set_wiki,
            commands::wiki_titles,
            commands::smapi_log_report,
            companion_commands::settings_set_companion,
            companion_commands::companion_status,
            companion_commands::companion_available,
            companion_commands::companion_apply,
            companion_commands::companion_forecast,
            companion_commands::companion_items,
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
            commands::app_changelog,
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
            mods_commands::mod_set_github,
            mods_commands::mod_install_dependency,
            nexus_commands::nexus_search,
            nexus_commands::nexus_mod,
            nexus_commands::nexus_account,
            nexus_commands::nexus_set_key,
            nexus_commands::nexus_clear_key,
            nexus_commands::nexus_download,
            nexus_commands::nexus_file_page,
            nexus_commands::nxm_open,
            nexus_commands::nxm_take_pending,
            nexus_commands::downloads_list,
            nexus_commands::downloads_clear,
            saves_commands::saves_list,
            saves_commands::saves_progress,
            saves_commands::saves_perfection,
            saves_commands::saves_checkup,
            saves_commands::saves_snapshots,
            saves_commands::saves_backup,
            saves_commands::saves_restore,
            saves_commands::saves_export,
            saves_commands::saves_import,
            saves_commands::saves_delete,
            config_commands::mod_config_get,
            config_commands::mod_config_save,
            config_commands::mod_config_defaults,
            config_commands::mods_generate_configs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
