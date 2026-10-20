//! Tauri commands for the mod library and profiles.
//!
//! Every command returns the whole [`LibraryView`]: the library is small (hundreds of
//! packages at most) and one shape keeps the UI from patching partial state.

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

use super::{AppState, blocking, ensure_stopped, lock_library};
use crate::backups;
use crate::deps::{self, Issue};
use crate::error::{AppError, AppResult};
use crate::game;
use crate::library::{self, BrokenMod, Package};
use crate::manifest::{self, Manifest};
use crate::mod_config;
use crate::profiles::{self, Profiles};
use crate::updates;

/// Everything the library screen shows.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryView {
    pub packages: Vec<PackageView>,
    pub profiles: Profiles,
    /// Folders in the game's own `Mods` that are not in the store yet.
    pub importable: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageView {
    pub id: String,
    pub path: PathBuf,
    /// Bundled with SMAPI and always on.
    pub system: bool,
    pub enabled: bool,
    pub mods: Vec<ModView>,
    pub broken: Vec<BrokenMod>,
    /// Version of the newest backup, when the package can be rolled back.
    pub backup_version: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModView {
    pub rel_path: String,
    pub manifest: Manifest,
    pub source: Option<String>,
    pub issues: Vec<Issue>,
    /// Names of enabled mods that need this one.
    pub required_by: Vec<String>,
    pub has_config: bool,
    /// A content pack whose `content.json` declares a `ConfigSchema`.
    pub has_schema: bool,
    /// The GitHub repo updates come from first: the user's, else the manifest's.
    pub github: Option<String>,
    /// Whether `github` is one the user attached.
    pub github_custom: bool,
    /// The game's language code when the mod has translations but none in it.
    pub missing_translation: Option<String>,
}

// SMAPI's own mods ("SMAPI.ConsoleCommands", …) come with the installer and update with it
fn is_bundled(p: &Package) -> bool {
    !p.mods.is_empty()
        && p.mods
            .iter()
            .all(|m| m.manifest.unique_id.starts_with("SMAPI."))
}

fn game_mods(state: &AppState) -> (Vec<Package>, Vec<Package>) {
    let Ok(dir) = state.game_dir() else {
        return (Vec::new(), Vec::new());
    };
    library::scan(&dir.join("Mods"))
        .into_iter()
        .partition(is_bundled)
}

pub(crate) fn build_view(state: &AppState) -> LibraryView {
    let packages = library::scan(&state.store_root);
    let profiles = state.profiles.get();
    let repos = state.settings.get().github_repos;
    let language = std::env::var_os("HOME")
        .map(|h| std::path::PathBuf::from(h).join(".config/StardewValley"))
        .and_then(|d| crate::perfection::game_language(&d))
        .filter(|l| l != "en");
    let (bundled, others) = game_mods(state);
    let install = state.game_dir().ok().and_then(|d| game::inspect(&d).ok());
    let enabled = &profiles.active().enabled;
    let report = deps::analyze(
        &packages,
        &bundled,
        enabled,
        install.as_ref().and_then(|g| g.smapi_version.as_deref()),
        install.as_ref().and_then(|g| g.version.as_deref()),
    );

    let stored_ids: std::collections::HashSet<String> = packages
        .iter()
        .flat_map(|p| p.unique_ids().map(str::to_lowercase).collect::<Vec<_>>())
        .collect();
    let importable = others
        .iter()
        .filter(|p| {
            !p.unique_ids()
                .all(|id| stored_ids.contains(&id.to_lowercase()))
        })
        .map(|p| p.id.clone())
        .collect();

    let backup_of = |p: &Package| {
        let uid = &p.mods.first()?.manifest.unique_id;
        backups::list(&state.backup_root, uid)
            .into_iter()
            .next()
            .map(|b| b.version)
    };
    let view = |p: Package, system: bool| PackageView {
        backup_version: if system { None } else { backup_of(&p) },
        enabled: system || enabled.contains(&p.id),
        mods: p
            .mods
            .into_iter()
            .map(|m| ModView {
                issues: report
                    .issues
                    .get(&(p.id.clone(), m.rel_path.clone()))
                    .cloned()
                    .unwrap_or_default(),
                required_by: report
                    .dependents
                    .get(&m.manifest.unique_id.to_lowercase())
                    .cloned()
                    .unwrap_or_default(),
                has_config: p.path.join(&m.rel_path).join("config.json").is_file(),
                has_schema: m.manifest.content_pack_for.is_some()
                    && mod_config::declares_schema(&p.path.join(&m.rel_path)),
                source: manifest::source(&m.manifest),
                github_custom: repos.contains_key(&m.manifest.unique_id.to_lowercase()),
                missing_translation: language.clone().filter(|l| {
                    // only a mod that translates at all can be missing a language
                    let i18n = p.path.join(&m.rel_path).join("i18n");
                    i18n.is_dir() && !i18n.join(format!("{l}.json")).is_file()
                }),
                github: repos
                    .get(&m.manifest.unique_id.to_lowercase())
                    .cloned()
                    .or_else(|| updates::github_key(&m.manifest.update_keys)),
                rel_path: m.rel_path,
                manifest: m.manifest,
            })
            .collect(),
        id: p.id,
        path: p.path,
        system,
        broken: p.broken,
    };
    let packages = bundled
        .into_iter()
        .map(|p| view(p, true))
        .chain(packages.into_iter().map(|p| view(p, false)))
        .collect();
    LibraryView {
        packages,
        profiles,
        importable,
    }
}

/// Builds the active profile's symlink folder and returns it for `--mods-path`.
pub fn deploy_active(state: &AppState) -> AppResult<PathBuf> {
    let _guard = lock_library(state);
    let (bundled, _) = game_mods(state);
    let mut bundled: Vec<(String, PathBuf)> = bundled.into_iter().map(|p| (p.id, p.path)).collect();
    if state.companion_enabled() {
        let dir = state.companion_dir.get().expect("enabled implies built");
        bundled.push((crate::companion::FOLDER.to_owned(), dir.clone()));
    }
    profiles::deploy(
        state.profiles.get().active(),
        &state.store_root,
        &bundled,
        &state.deploy_root,
    )
}

pub(crate) fn record_installs(state: &AppState, installed: &[library::Installed]) -> AppResult<()> {
    state.profiles.update(|p| {
        for i in installed {
            if i.replaced.is_empty() {
                p.set_enabled(&i.id, true);
            } else {
                p.replace_package(&i.replaced, &i.id);
            }
        }
        Ok(())
    })?;
    for i in installed {
        let Some(pkg) = library::package_path(&state.store_root, &i.id)
            .ok()
            .and_then(|path| library::read_package(i.id.clone(), path))
        else {
            continue;
        };
        let Some(m) = pkg.mods.first() else { continue };
        let updated = !i.replaced.is_empty();
        // the install just backed up what it replaced, so the newest backup is that version
        let previous = updated
            .then(|| {
                backups::list(&state.backup_root, &m.manifest.unique_id)
                    .into_iter()
                    .next()
            })
            .flatten()
            .map(|b| b.version);
        state.history.push(crate::history::Entry {
            at: crate::history::now_ms(),
            kind: if updated {
                crate::history::Kind::Updated
            } else {
                crate::history::Kind::Installed
            },
            package: i.id.clone(),
            name: m.manifest.name.clone(),
            unique_id: m.manifest.unique_id.clone(),
            version: m.manifest.version.clone(),
            previous,
            stash: None,
            was_enabled: true,
            undone: false,
        })?;
    }
    Ok(())
}

#[tauri::command]
pub async fn library_get(app: AppHandle) -> AppResult<LibraryView> {
    blocking(app, |s| Ok(build_view(s))).await
}

/// Installs `.zip` archives and mod folders; new packages are enabled in the active profile.
#[tauri::command]
pub async fn library_install(app: AppHandle, paths: Vec<PathBuf>) -> AppResult<LibraryView> {
    blocking(app, move |s| {
        install_paths(s, &paths)?;
        Ok(build_view(s))
    })
    .await
}

/// Installs archives or mod folders into the store and enables new packages in the
/// active profile. Blocking; call from a blocking thread.
pub fn install_paths(s: &AppState, paths: &[PathBuf]) -> AppResult<()> {
    ensure_stopped(s)?;
    let _guard = lock_library(s);
    for path in paths {
        let mut done = Vec::new();
        let result = if path.is_dir() {
            library::install_dir_into(&s.store_root, &s.backup_root, path, &mut done)
        } else {
            library::install_archive_into(&s.store_root, &s.backup_root, path, &mut done)
        };
        // whatever made it into the store belongs in the profile, failure or not
        record_installs(s, &done)?;
        result?;
    }
    Ok(())
}

/// Copies folders from the game's `Mods` into the store, leaving the originals.
#[tauri::command]
pub async fn library_import(app: AppHandle, names: Vec<String>) -> AppResult<LibraryView> {
    blocking(app, move |s| {
        let mods_dir = s.game_dir()?.join("Mods");
        ensure_stopped(s)?;
        let _guard = lock_library(s);
        for name in &names {
            let src = library::package_path(&mods_dir, name)?;
            let mut done = Vec::new();
            let result = library::install_dir_into(&s.store_root, &s.backup_root, &src, &mut done);
            record_installs(s, &done)?;
            result?;
        }
        drop(_guard);
        Ok(build_view(s))
    })
    .await
}

#[tauri::command]
pub async fn library_remove(app: AppHandle, id: String) -> AppResult<LibraryView> {
    blocking(app, move |s| {
        ensure_stopped(s)?;
        let _guard = lock_library(s);
        let path = library::package_path(&s.store_root, &id)?;
        let pkg = library::read_package(id.clone(), path.clone());
        let ids: std::collections::HashSet<String> = pkg
            .as_ref()
            .map(|p| p.unique_ids().map(str::to_lowercase).collect())
            .unwrap_or_default();
        // an import copies from the game's Mods folder; the original would come back as
        // importable and still load when the game starts without the launcher
        let (_, others) = game_mods(s);
        for p in others.iter().filter(|p| {
            !p.mods.is_empty() && p.unique_ids().all(|u| ids.contains(&u.to_lowercase()))
        }) {
            trash::delete(&p.path).map_err(|e| AppError::Io(std::io::Error::other(e)))?;
        }
        // kept aside rather than deleted, so the history can put it back
        let at = crate::history::now_ms();
        let was_enabled = s.profiles.get().active().enabled.contains(&id);
        let stash = crate::history::stash(&path, &s.removed_root, at)?;
        s.profiles.update(|p| {
            p.forget_package(&id);
            Ok(())
        })?;
        let first = pkg.as_ref().and_then(|p| p.mods.first());
        s.history.push(crate::history::Entry {
            at,
            kind: crate::history::Kind::Removed,
            package: id.clone(),
            name: first
                .map(|m| m.manifest.name.clone())
                .unwrap_or_else(|| id.clone()),
            unique_id: first
                .map(|m| m.manifest.unique_id.clone())
                .unwrap_or_default(),
            version: first
                .map(|m| m.manifest.version.clone())
                .unwrap_or_default(),
            previous: None,
            stash: Some(stash),
            was_enabled,
            undone: false,
        })?;
        drop(_guard);
        Ok(build_view(s))
    })
    .await
}

#[tauri::command]
pub async fn library_toggle(app: AppHandle, id: String, enabled: bool) -> AppResult<LibraryView> {
    blocking(app, move |s| {
        library::package_path(&s.store_root, &id)?;
        s.profiles.update(|p| {
            p.set_enabled(&id, enabled);
            Ok(())
        })?;
        Ok(build_view(s))
    })
    .await
}

#[tauri::command]
pub fn library_open(app: AppHandle, path: PathBuf) -> AppResult<()> {
    let state = app.state::<AppState>();
    let allowed = [state.store_root.clone(), state.game_dir()?.join("Mods")];
    if !allowed.iter().any(|root| inside(root, &path)) {
        return Err(AppError::NotFound(path.display().to_string()));
    }
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(|e| AppError::Io(std::io::Error::other(e)))
}

fn inside(root: &Path, path: &Path) -> bool {
    match (root.canonicalize(), path.canonicalize()) {
        (Ok(r), Ok(p)) => p.starts_with(r),
        _ => false,
    }
}

#[tauri::command]
pub async fn profile_create(
    app: AppHandle,
    name: String,
    copy_from: Option<String>,
) -> AppResult<LibraryView> {
    blocking(app, move |s| {
        s.profiles.update(|p| {
            let id = p.create(&name, copy_from.as_deref())?;
            p.activate(&id)
        })?;
        Ok(build_view(s))
    })
    .await
}

#[tauri::command]
pub async fn profile_rename(app: AppHandle, id: String, name: String) -> AppResult<LibraryView> {
    blocking(app, move |s| {
        s.profiles.update(|p| p.rename(&id, &name))?;
        Ok(build_view(s))
    })
    .await
}

#[tauri::command]
pub async fn profile_delete(app: AppHandle, id: String) -> AppResult<LibraryView> {
    blocking(app, move |s| {
        s.profiles.update(|p| p.delete(&id))?;
        let _ = std::fs::remove_dir_all(s.deploy_root.join(&id));
        Ok(build_view(s))
    })
    .await
}

#[tauri::command]
pub async fn profile_activate(app: AppHandle, id: String) -> AppResult<LibraryView> {
    blocking(app, move |s| {
        s.profiles.update(|p| p.activate(&id))?;
        Ok(build_view(s))
    })
    .await
}

/// The library's installs, updates and removals, newest first.
#[tauri::command]
pub fn history_list(app: AppHandle) -> Vec<crate::history::Entry> {
    app.state::<AppState>().history.list()
}

/// Undoes history entry `at`: removes an install, rolls an update back, or puts a
/// removed package back where it was.
#[tauri::command]
pub async fn history_undo(app: AppHandle, at: u64) -> AppResult<LibraryView> {
    blocking(app, move |s| {
        ensure_stopped(s)?;
        let e = s
            .history
            .get(at)
            .filter(|e| !e.undone)
            .ok_or_else(|| AppError::NotFound(at.to_string()))?;
        let _guard = lock_library(s);
        match e.kind {
            crate::history::Kind::Installed => {
                let path = library::package_path(&s.store_root, &e.package)?;
                crate::history::stash(&path, &s.removed_root, crate::history::now_ms())?;
                s.profiles.update(|p| {
                    p.forget_package(&e.package);
                    Ok(())
                })?;
            }
            crate::history::Kind::Updated => {
                let backup = backups::list(&s.backup_root, &e.unique_id)
                    .into_iter()
                    .next()
                    .ok_or_else(|| AppError::NotFound(e.unique_id.clone()))?;
                let mut done = Vec::new();
                let result = library::install_dir_into(
                    &s.store_root,
                    &s.backup_root,
                    &backup.path,
                    &mut done,
                );
                s.profiles.update(|p| {
                    for i in &done {
                        p.replace_package(&i.replaced, &i.id);
                    }
                    Ok(())
                })?;
                result?;
                backups::discard(&s.backup_root, &backup)?;
            }
            crate::history::Kind::Removed => {
                let stash = e
                    .stash
                    .clone()
                    .ok_or_else(|| AppError::NotFound(e.package.clone()))?;
                let dest = s.store_root.join(&e.package);
                if dest.exists() {
                    return Err(AppError::Io(std::io::Error::other(format!(
                        "{} is installed again",
                        e.package
                    ))));
                }
                std::fs::rename(&stash, &dest)?;
                if e.was_enabled {
                    s.profiles.update(|p| {
                        p.set_enabled(&e.package, true);
                        Ok(())
                    })?;
                }
            }
        }
        s.history.mark_undone(at)?;
        drop(_guard);
        Ok(build_view(s))
    })
    .await
}

/// Records the active profile for the saves written since `since` (Unix seconds), so
/// the next launch can offer it back; called when a session ends.
#[tauri::command]
pub async fn saves_remember_profile(app: AppHandle, since: u64) -> AppResult<LibraryView> {
    let saves_dir = crate::commands::game_data_dir(&app)?.join("Saves");
    blocking(app, move |s| {
        let played: Vec<String> = crate::saves::list(&saves_dir)
            .into_iter()
            .filter(|save| save.modified >= since)
            .map(|save| save.id)
            .collect();
        if !played.is_empty() {
            s.profiles.update(|p| {
                p.remember_saves(&played);
                Ok(())
            })?;
        }
        Ok(build_view(s))
    })
    .await
}

/// Lists the saves that still use package `id`: its items and data, or its locations.
#[tauri::command]
pub async fn mod_save_usage(
    app: AppHandle,
    id: String,
) -> AppResult<Vec<crate::save_usage::SaveUsage>> {
    let saves_dir = crate::commands::game_data_dir(&app)?.join("Saves");
    blocking(app, move |s| {
        let path = library::package_path(&s.store_root, &id)?;
        let ids: Vec<String> = library::read_package(id.clone(), path.clone())
            .map(|p| p.unique_ids().map(str::to_owned).collect())
            .unwrap_or_default();
        Ok(crate::save_usage::check(&path, &ids, &saves_dir))
    })
    .await
}

/// Content packs in the active profile that load the same asset exclusively.
#[tauri::command]
pub async fn library_conflicts(app: AppHandle) -> AppResult<Vec<crate::conflicts::Conflict>> {
    blocking(app, |s| {
        Ok(crate::conflicts::find(
            &library::scan(&s.store_root),
            &s.profiles.get().active().enabled,
        ))
    })
    .await
}
