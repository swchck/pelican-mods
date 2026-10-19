//! Tauri commands for the mod library and profiles.
//!
//! Every command returns the whole [`LibraryView`]: the library is small (hundreds of
//! packages at most) and one shape keeps the UI from patching partial state.

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

use crate::backups;
use crate::commands::AppState;
use crate::deps::{self, Issue};
use crate::downloads;
use crate::error::{AppError, AppResult};
use crate::game;
use crate::library::{self, BrokenMod, Package};
use crate::manifest::{self, Manifest};
use crate::mod_config;
use crate::moddrop;
use crate::nexus;
use crate::nexus_commands;
use crate::profiles::{self, Profiles};
use crate::updates::{self, UpdateReport};

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
pub(crate) async fn blocking<T: Send + 'static>(
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
        install_paths(s, &paths)?;
        Ok(build_view(s))
    })
    .await
}

/// Installs archives or mod folders into the store and enables new packages in the
/// active profile. Blocking; call from a blocking thread.
pub fn install_paths(s: &AppState, paths: &[PathBuf]) -> AppResult<()> {
    let _guard = s.library_lock.lock().unwrap();
    for path in paths {
        let installed = if path.is_dir() {
            library::install_dir(&s.store_root, &s.backup_root, path)?
        } else {
            library::install_archive(&s.store_root, &s.backup_root, path)?
        };
        record_installs(s, &installed)?;
    }
    Ok(())
}

/// Copies folders from the game's `Mods` into the store, leaving the originals.
#[tauri::command]
pub async fn library_import(app: AppHandle, names: Vec<String>) -> AppResult<LibraryView> {
    blocking(app, move |s| {
        let mods_dir = s.game_dir()?.join("Mods");
        let _guard = s.library_lock.lock().unwrap();
        for name in &names {
            let src = library::package_path(&mods_dir, name)?;
            let installed = library::install_dir(&s.store_root, &s.backup_root, &src)?;
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

fn versions(state: &AppState) -> (String, String) {
    let install = state.game_dir().ok().and_then(|d| game::inspect(&d).ok());
    let smapi = install
        .as_ref()
        .and_then(|g| g.smapi_version.clone())
        .unwrap_or_else(|| "4.0.0".into());
    let game = install
        .and_then(|g| g.version)
        .unwrap_or_else(|| "1.6.0".into());
    (smapi, game)
}

/// Returns update info for the library, asking smapi.io when `force` is set or the
/// cached answer is stale or misses an installed mod.
#[tauri::command]
pub async fn updates_check(app: AppHandle, force: bool) -> AppResult<UpdateReport> {
    let (packages, cached, (smapi, game)) = blocking(app.clone(), |s| {
        Ok((
            library::scan(&s.store_root),
            s.updates.lock().unwrap().clone(),
            versions(s),
        ))
    })
    .await?;
    let covered = packages
        .iter()
        .flat_map(|p| p.unique_ids())
        .all(|id| cached.mods.contains_key(&id.to_lowercase()));
    if !force && cached.fresh() && covered {
        return Ok(cached);
    }
    let state = app.state::<AppState>();
    let report = updates::check(&state.http, &packages, &smapi, &game).await?;
    *state.updates.lock().unwrap() = report.clone();
    let _ = updates::save_cache(&state.updates_file, &report);
    Ok(report)
}

/// Downloads and installs the suggested update of package `id`, when smapi.io points
/// at a GitHub release. Other sites need a browser (or Nexus' nxm links) to download.
/// How [`mod_update`] went: installed in place, queued as a download, or left for
/// the browser because only the site can hand out the file.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum UpdateOutcome {
    Installed { view: LibraryView },
    Queued,
    OpenPage { url: String },
}

/// Updates package `id` to the version smapi.io suggested: GitHub and ModDrop files
/// install right away, Nexus files go through the download queue for Premium accounts.
#[tauri::command]
pub async fn mod_update(app: AppHandle, id: String) -> AppResult<UpdateOutcome> {
    let state = app.state::<AppState>();
    let pkg = library::read_package(id.clone(), library::package_path(&state.store_root, &id)?)
        .ok_or_else(|| AppError::NotFound(id.clone()))?;
    let found = {
        let report = state.updates.lock().unwrap();
        pkg.mods.iter().find_map(|m| {
            let info = report.mods.get(&m.manifest.unique_id.to_lowercase())?;
            Some((
                m.manifest.name.clone(),
                info.version.clone()?,
                info.url.clone()?,
                info.nexus_id,
            ))
        })
    };
    let (name, version, url, nexus_id) = found.ok_or_else(|| AppError::NoDownload(id.clone()))?;
    let dir = state.work_dir.with_file_name("downloads");
    let file = if let Some(repo) = updates::github_repo(&url) {
        Some(updates::download_github(&state.http, &repo, &name, &version, &dir).await?)
    } else if let Some(mod_id) = moddrop::mod_page_id(&url) {
        let files = moddrop::files(&state.http, mod_id).await?;
        match moddrop::pick(&files, &version) {
            Some(f) => {
                let link = moddrop::download_url(&state.http, f.id).await?;
                Some(downloads::fetch(&state.http, &link, &dir, |_, _| {}).await?)
            }
            None => None,
        }
    } else {
        None
    };
    if let Some(file) = file {
        let view = blocking(app, move |s| {
            let _guard = s.library_lock.lock().unwrap();
            let installed = library::install_archive(&s.store_root, &s.backup_root, &file);
            let _ = std::fs::remove_file(&file);
            record_installs(s, &installed?)?;
            drop(_guard);
            Ok(build_view(s))
        })
        .await?;
        return Ok(UpdateOutcome::Installed { view });
    }
    let Some(mod_id) =
        nexus::mod_page_id(&url).or(nexus_id.filter(|_| url.contains("nexusmods.com")))
    else {
        return Ok(UpdateOutcome::OpenPage { url });
    };
    let details = nexus::details(&state.http, mod_id).await?;
    let Some(file) = nexus::pick_update_file(&details.files, &version, &name) else {
        return Ok(UpdateOutcome::OpenPage { url });
    };
    let premium = nexus_commands::nexus_account(app.clone())
        .await
        .ok()
        .flatten()
        .is_some_and(|a| a.is_premium);
    if !premium {
        // "Mod Manager Download" on this page sends an nxm link back, which installs it
        return Ok(UpdateOutcome::OpenPage {
            url: nexus::file_page(mod_id, file.file_id),
        });
    }
    nexus_commands::start(app, mod_id, file.file_id, None);
    Ok(UpdateOutcome::Queued)
}

/// Restores the newest backup of the mod `unique_id`; the current version is backed up.
#[tauri::command]
pub async fn mod_rollback(app: AppHandle, unique_id: String) -> AppResult<LibraryView> {
    blocking(app, move |s| {
        let _guard = s.library_lock.lock().unwrap();
        let backup = backups::list(&s.backup_root, &unique_id)
            .into_iter()
            .next()
            .ok_or_else(|| AppError::NotFound(unique_id.clone()))?;
        let installed = library::install_dir(&s.store_root, &s.backup_root, &backup.path)?;
        backups::discard(&s.backup_root, &backup)?;
        record_installs(s, &installed)?;
        drop(_guard);
        Ok(build_view(s))
    })
    .await
}
