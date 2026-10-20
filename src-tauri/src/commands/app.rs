//! Commands for the app itself: settings, disk space, folders, its changelog.

use serde::Deserialize;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use super::{AppState, game_data_dir, lock_library};
use crate::error::{AppError, AppResult};
use crate::settings::Settings;

#[tauri::command]
pub fn settings_get(state: State<AppState>) -> Settings {
    state.settings.get()
}

/// Picks where item names link: "official", "fandom" or "off".
#[tauri::command]
pub fn settings_set_wiki(state: State<AppState>, wiki: String) -> AppResult<Settings> {
    let wiki = match wiki.as_str() {
        "official" | "fandom" | "off" => wiki,
        _ => "official".to_owned(),
    };
    state.settings.update(|s| s.wiki = Some(wiki))
}

/// English wiki page titles for base game object ids ("(O)24"); unknown ids are left out.
#[tauri::command]
pub fn wiki_titles(ids: Vec<String>) -> std::collections::HashMap<String, String> {
    ids.into_iter()
        .filter_map(|id| Some((id.clone(), crate::perfection::wiki_title(&id)?)))
        .collect()
}

/// Mod version backups kept per mod by the cleanup; the newest is what rollback uses.
const KEEP_MOD_BACKUPS: usize = 1;

/// Save snapshots kept per save by the cleanup.
const KEEP_SAVE_BACKUPS: usize = 5;

/// How much the launcher's own data takes on disk, by kind.
#[tauri::command]
pub async fn storage_usage(app: AppHandle) -> AppResult<Vec<crate::storage::Usage>> {
    crate::commands::blocking(app, |s| {
        Ok(vec![
            crate::storage::measure("modBackups", &s.backup_root),
            crate::storage::measure("removed", &s.removed_root),
            crate::storage::measure("saveBackups", &s.save_backup_root),
            crate::storage::measure("downloads", &s.work_dir.with_file_name("downloads")),
        ])
    })
    .await
}

/// Trims one kind of kept data: old mod and save backups, set-aside removals, downloads.
#[tauri::command]
pub async fn storage_clean(app: AppHandle, kind: String) -> AppResult<u32> {
    crate::commands::blocking(app, move |s| {
        // an update, rollback or undo may be reading these folders
        let _guard = lock_library(s);
        match kind.as_str() {
            "modBackups" => crate::storage::prune(&s.backup_root, KEEP_MOD_BACKUPS, |_| true),
            // the automatic ones only: a manual copy or the state a restore replaced may
            // be the one the player is counting on
            "saveBackups" => crate::storage::prune(&s.save_backup_root, KEEP_SAVE_BACKUPS, |n| {
                n.ends_with("@launch.zip")
            }),
            "removed" => {
                let cleared = crate::storage::clear(&s.removed_root);
                // even a partial clear leaves stashes history can't put back any more
                s.history.forget_stashes()?;
                cleared
            }
            "downloads" => crate::storage::clear(&s.work_dir.with_file_name("downloads")),
            other => Err(AppError::NotFound(other.to_owned())),
        }
    })
    .await
}

/// The language the game is set to, such as "ru", if it has ever started.
#[tauri::command]
pub fn game_language(app: AppHandle) -> Option<String> {
    crate::perfection::game_language(&game_data_dir(&app).ok()?)
}

#[tauri::command]
pub fn settings_set_locale(state: State<AppState>, locale: Option<String>) -> AppResult<Settings> {
    state.settings.update(|s| s.locale = locale)
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Place {
    Game,
    Mods,
    SmapiLog,
    Saves,
}

#[tauri::command]
pub fn open_place(app: AppHandle, state: State<AppState>, place: Place) -> AppResult<()> {
    let data = game_data_dir(&app)?;
    let path = match place {
        Place::Game => state.game_dir()?,
        Place::Mods => state.game_dir()?.join("Mods"),
        Place::SmapiLog => data.join("ErrorLogs/SMAPI-latest.txt"),
        Place::Saves => data.join("Saves"),
    };
    if !path.exists() {
        return Err(AppError::GameNotFound(path.display().to_string()));
    }
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(|e| AppError::Io(std::io::Error::other(e)))
}

#[tauri::command]
pub async fn rosetta_install() -> AppResult<()> {
    crate::rosetta::install().await
}

/// Fetches the changelog as published with release `version`, so an older app can show
/// what a newer one brings. `locale` "ru" picks the Russian file.
#[tauri::command]
pub async fn app_changelog(
    state: State<'_, AppState>,
    version: String,
    locale: String,
) -> AppResult<String> {
    let valid = !version.is_empty()
        && version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+'));
    if !valid {
        return Err(AppError::NotFound(version));
    }
    let file = if locale == "ru" {
        "CHANGELOG.ru.md"
    } else {
        "CHANGELOG.md"
    };
    let url = format!("https://raw.githubusercontent.com/{RELEASE_REPO}/v{version}/{file}");
    let resp = state.http.get(url).send().await?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(AppError::NotFound(format!("changelog for {version}")));
    }
    Ok(resp.error_for_status()?.text().await?)
}

/// Repository whose releases feed the updater; must match `plugins.updater.endpoints`.
const RELEASE_REPO: &str = "swchck/pelican-mods";
