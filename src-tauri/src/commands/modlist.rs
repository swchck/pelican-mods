//! Commands for mod list files: export a profile, read one, make a profile from it.

use std::path::PathBuf;

use tauri::AppHandle;

use super::blocking;
use super::library::{LibraryView, build_view};
use crate::error::AppResult;
use crate::game;
use crate::library;

/// Writes the active profile's mods to `dest` as a mod list.
#[tauri::command]
pub async fn modlist_export(app: AppHandle, dest: PathBuf) -> AppResult<()> {
    blocking(app, move |s| {
        let profiles = s.profiles.get();
        let active = profiles.active();
        let install = s.game_dir().ok().and_then(|d| game::inspect(&d).ok());
        let list = crate::modlist::build(
            active.name.as_deref().unwrap_or("Pelican Mods"),
            &library::scan(&s.store_root),
            &active.enabled,
            &s.settings.get().github_repos,
            install.as_ref().and_then(|g| g.version.clone()),
            install.and_then(|g| g.smapi_version),
        );
        std::fs::write(
            &dest,
            serde_json::to_vec_pretty(&list).expect("list serialize"),
        )?;
        Ok(())
    })
    .await
}

/// Reads the mod list in `file` and says which of its mods the library has.
#[tauri::command]
pub async fn modlist_read(app: AppHandle, file: PathBuf) -> AppResult<crate::modlist::ModListView> {
    blocking(app, move |s| {
        let list = crate::modlist::read(&file)?;
        Ok(crate::modlist::status(
            list,
            &library::scan(&s.store_root),
            &s.profiles.get().active().enabled,
        ))
    })
    .await
}

/// Creates profile `name` with just the packages holding `unique_ids` and switches to it.
#[tauri::command]
pub async fn modlist_profile(
    app: AppHandle,
    name: String,
    unique_ids: Vec<String>,
) -> AppResult<LibraryView> {
    blocking(app, move |s| {
        let packages = crate::modlist::packages_for(&library::scan(&s.store_root), &unique_ids);
        s.profiles.update(|p| {
            let id = p.create(&name, None)?;
            p.activate(&id)?;
            for package in &packages {
                p.set_enabled(package, true);
            }
            Ok(())
        })?;
        Ok(build_view(s))
    })
    .await
}
