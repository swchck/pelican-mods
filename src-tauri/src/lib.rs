mod archive;
mod backups;
mod bisect;
mod checkup;
mod chests;
mod commands;
mod companion;
mod compat;
mod conflicts;
mod deps;
mod downloads;
mod error;
mod farm_history;
mod game;
mod history;
mod jsonc;
mod launcher;
mod library;
mod manifest;
mod mod_config;
mod moddrop;
mod modlist;
mod nexus;
mod perfection;
mod profile_configs;
mod profiles;
mod rosetta;
mod save_backups;
mod save_diff;
mod save_usage;
mod saves;
mod settings;
mod smapi;
mod smapi_log;
mod storage;
mod translations;
mod updates;
mod version;
mod walnuts;

use tauri::Manager;
use tauri_plugin_deep_link::DeepLinkExt;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let data_dir = app.path().app_data_dir()?;
            let cache_dir = app.path().app_cache_dir()?;
            app.manage(commands::AppState::new(config_dir, data_dir, cache_dir));
            commands::companion::start(app.handle());

            // Nexus' "Mod Manager Download" button opens nxm:// links; macOS hands them
            // to the running app, or launches it with the link
            let handle = app.handle().clone();
            let open = move |urls: Vec<url::Url>| {
                for u in urls {
                    if let Err(e) = commands::nexus::handle_nxm(&handle, u.as_str()) {
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
            commands::app::settings_get,
            commands::app::settings_set_locale,
            commands::app::game_language,
            commands::app::settings_set_wiki,
            commands::app::settings_set_spoilers,
            commands::app::wiki_titles,
            commands::game::smapi_log_report,
            commands::game::game_catalog,
            commands::app::storage_usage,
            commands::app::storage_clean,
            commands::updates::compat_for_nexus,
            commands::companion::settings_set_companion,
            commands::companion::settings_set_briefing,
            commands::companion::companion_status,
            commands::companion::companion_available,
            commands::companion::companion_apply,
            commands::companion::companion_forecast,
            commands::companion::companion_items,
            commands::companion::companion_profile_start,
            commands::companion::companion_profile_stop,
            commands::companion::companion_patches,
            commands::game::game_detect,
            commands::game::game_current,
            commands::game::game_set_path,
            commands::game::game_forget,
            commands::game::smapi_latest,
            commands::game::smapi_run,
            commands::game::game_launch,
            commands::game::game_stop,
            commands::game::game_state,
            commands::game::game_log,
            commands::game::game_console,
            commands::app::open_place,
            commands::app::rosetta_install,
            commands::app::app_changelog,
            commands::library::library_get,
            commands::library::library_install,
            commands::library::library_import,
            commands::library::library_remove,
            commands::library::library_toggle,
            commands::library::library_open,
            commands::library::library_set_tags,
            commands::library::bisect_start,
            commands::library::bisect_answer,
            commands::library::bisect_partner,
            commands::library::bisect_cancel,
            commands::library::profile_create,
            commands::library::profile_rename,
            commands::library::profile_delete,
            commands::library::profile_activate,
            commands::library::profile_set_own_configs,
            commands::updates::updates_check,
            commands::updates::mod_update,
            commands::updates::mod_rollback,
            commands::updates::mod_set_github,
            commands::updates::mod_install_dependency,
            commands::library::mod_save_usage,
            commands::game::game_preflight,
            commands::game::game_crash,
            commands::library::saves_remember_profile,
            commands::library::history_list,
            commands::library::history_undo,
            commands::library::library_conflicts,
            commands::updates::mod_changelog,
            commands::modlist::modlist_export,
            commands::modlist::modlist_read,
            commands::modlist::modlist_profile,
            commands::nexus::nexus_search,
            commands::nexus::nexus_mod,
            commands::nexus::nexus_account,
            commands::nexus::nexus_set_key,
            commands::nexus::nexus_clear_key,
            commands::nexus::nexus_download,
            commands::nexus::nexus_file_page,
            commands::nexus::nexus_collection,
            commands::nexus::nexus_thumbnails,
            commands::nexus::nxm_open,
            commands::nexus::nxm_take_pending,
            commands::nexus::downloads_list,
            commands::nexus::downloads_clear,
            commands::saves::saves_list,
            commands::saves::saves_progress,
            commands::saves::saves_perfection,
            commands::saves::saves_checkup,
            commands::saves::saves_items,
            commands::saves::saves_walnuts,
            commands::saves::saves_history,
            commands::saves::saves_compare,
            commands::saves::saves_snapshots,
            commands::saves::saves_backup,
            commands::saves::saves_restore,
            commands::saves::saves_export,
            commands::saves::saves_import,
            commands::saves::saves_delete,
            commands::saves::saves_mirror,
            commands::saves::settings_set_backup_folder,
            commands::config::mod_config_get,
            commands::config::mod_translation_get,
            commands::config::mod_translation_save,
            commands::config::mod_config_save,
            commands::config::mod_config_defaults,
            commands::config::mods_generate_configs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
