//! Tauri commands for the mod library and profiles.
//!
//! Every command returns the whole [`LibraryView`]: the library is small (hundreds of
//! packages at most) and one shape keeps the UI from patching partial state.

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

use crate::commands::AppState;
use crate::deps::{self, Issue};
use crate::error::{AppError, AppResult};
use crate::game;
use crate::library::{self, BrokenMod, Package};
use crate::manifest::{self, Manifest};
use crate::profiles::{self, Profiles};

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

fn build_view(state: &AppState) -> LibraryView {
    let packages = library::scan(&state.store_root);
    let profiles = state.profiles.get();
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

    let view = |p: Package, system: bool| PackageView {
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
                source: manifest::source(&m.manifest),
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
    let _guard = state.library_lock.lock().unwrap();
    let (bundled, _) = game_mods(state);
    let bundled: Vec<(String, PathBuf)> = bundled.into_iter().map(|p| (p.id, p.path)).collect();
    profiles::deploy(
        state.profiles.get().active(),
        &state.store_root,
        &bundled,
        &state.deploy_root,
    )
}

// store changes touch disk and may unpack hundreds of megabytes; keep them off the main thread
async fn blocking<T: Send + 'static>(
    app: AppHandle,
    f: impl FnOnce(&AppState) -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(move || f(&app.state::<AppState>()))
        .await
        .map_err(|e| AppError::Io(std::io::Error::other(e)))?
}

fn record_installs(state: &AppState, installed: &[library::Installed]) -> AppResult<()> {
    state.profiles.update(|p| {
        for i in installed {
            if i.replaced.is_empty() {
                p.set_enabled(&i.id, true);
            } else {
                p.replace_package(&i.replaced, &i.id);
            }
        }
        Ok(())
    })
}

#[tauri::command]
pub async fn library_get(app: AppHandle) -> AppResult<LibraryView> {
    blocking(app, |s| Ok(build_view(s))).await
}

/// Installs `.zip` archives and mod folders; new packages are enabled in the active profile.
#[tauri::command]
pub async fn library_install(app: AppHandle, paths: Vec<PathBuf>) -> AppResult<LibraryView> {
    blocking(app, move |s| {
        let _guard = s.library_lock.lock().unwrap();
        for path in &paths {
            let installed = if path.is_dir() {
                library::install_dir(&s.store_root, path)?
            } else {
                library::install_zip(&s.store_root, path)?
            };
            record_installs(s, &installed)?;
        }
        drop(_guard);
        Ok(build_view(s))
    })
    .await
}

/// Copies folders from the game's `Mods` into the store, leaving the originals.
#[tauri::command]
pub async fn library_import(app: AppHandle, names: Vec<String>) -> AppResult<LibraryView> {
    blocking(app, move |s| {
        let mods_dir = s.game_dir()?.join("Mods");
        let _guard = s.library_lock.lock().unwrap();
        for name in &names {
            let src = library::package_path(&mods_dir, name)?;
            let installed = library::install_dir(&s.store_root, &src)?;
            record_installs(s, &installed)?;
        }
        drop(_guard);
        Ok(build_view(s))
    })
    .await
}

#[tauri::command]
pub async fn library_remove(app: AppHandle, id: String) -> AppResult<LibraryView> {
    blocking(app, move |s| {
        let _guard = s.library_lock.lock().unwrap();
        library::remove(&s.store_root, &id)?;
        s.profiles.update(|p| {
            p.forget_package(&id);
            Ok(())
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
