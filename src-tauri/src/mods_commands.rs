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

fn build_view(state: &AppState) -> LibraryView {
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

/// Takes the library lock. A panic inside an archive library poisons it; the store on disk
/// is still consistent then, so later installs and launches carry on instead of panicking.
pub(crate) fn lock_library(s: &AppState) -> std::sync::MutexGuard<'_, ()> {
    s.library_lock
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

// moving packages under a running SMAPI breaks the symlinks it reads assets through
fn ensure_stopped(s: &AppState) -> AppResult<()> {
    if matches!(
        s.launcher.state(),
        crate::launcher::RunState::Running { .. }
    ) {
        return Err(AppError::GameRunning);
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
        let ids: std::collections::HashSet<String> = library::read_package(id.clone(), path)
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
    let repos = state.settings.get().github_repos;
    let report = updates::check(&state.http, &packages, &repos, &smapi, &game).await?;
    *state.updates.lock().unwrap() = report.clone();
    let _ = updates::save_cache(&state.updates_file, &report);
    Ok(report)
}

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
/// A mod that also names a GitHub repo is fetched from its releases first, wherever
/// smapi.io pointed.
#[tauri::command]
pub async fn mod_update(app: AppHandle, id: String) -> AppResult<UpdateOutcome> {
    let state = app.state::<AppState>();
    let pkg = library::read_package(id.clone(), library::package_path(&state.store_root, &id)?)
        .ok_or_else(|| AppError::NotFound(id.clone()))?;
    let repos = state.settings.get().github_repos;
    let found = {
        let report = state.updates.lock().unwrap();
        pkg.mods.iter().find_map(|m| {
            let info = report.mods.get(&m.manifest.unique_id.to_lowercase())?;
            Some((
                m.manifest.name.clone(),
                info.version.clone()?,
                info.url.clone()?,
                info.nexus_id,
                repos
                    .get(&m.manifest.unique_id.to_lowercase())
                    .cloned()
                    .or_else(|| updates::github_key(&m.manifest.update_keys)),
                nexus::update_subkey(&m.manifest.update_keys),
            ))
        })
    };
    let (name, version, url, nexus_id, github, subkey) =
        found.ok_or_else(|| AppError::NoDownload(id.clone()))?;
    fetch_and_install(
        app,
        Wanted {
            name,
            version,
            url,
            nexus_id,
            github,
            subkey,
        },
    )
    .await
}

/// A mod version to fetch, and every lead on where it lives.
struct Wanted {
    name: String,
    version: String,
    /// The page smapi.io points at.
    url: String,
    nexus_id: Option<u64>,
    github: Option<String>,
    /// A Nexus subkey for mods sharing a page ("@GF").
    subkey: Option<String>,
}

/// Installs `w` right away from GitHub or ModDrop, queues it from Nexus for Premium,
/// or hands back the page where only the user can start the download.
async fn fetch_and_install(app: AppHandle, w: Wanted) -> AppResult<UpdateOutcome> {
    let Wanted {
        name,
        version,
        url,
        nexus_id,
        github,
        subkey,
    } = w;
    let state = app.state::<AppState>();
    let dir = state.work_dir.with_file_name("downloads");
    let mut file = None;
    if let Some(repo) = updates::github_repo(&url) {
        file = Some(updates::download_github(&state.http, &repo, &name, &version, &dir).await?);
    } else if let Some(repo) = github {
        // some authors release on Nexus only and keep the repo for code; then the
        // suggested site still has the file
        file = updates::download_github(&state.http, &repo, &name, &version, &dir)
            .await
            .ok();
    }
    let file = if file.is_some() {
        file
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
            ensure_stopped(s)?;
            let _guard = lock_library(s);
            let mut done = Vec::new();
            let result =
                library::install_archive_into(&s.store_root, &s.backup_root, &file, &mut done);
            let _ = std::fs::remove_file(&file);
            record_installs(s, &done)?;
            result?;
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
    let Some(file) = nexus::pick_update_file(&details.files, &version, &name, subkey.as_deref())
    else {
        return Ok(UpdateOutcome::OpenPage { url });
    };
    let premium = nexus_commands::nexus_account(app.clone())
        .await
        .ok()
        .flatten()
        .is_some_and(|a| a.is_premium);
    if !premium {
        // "Mod Manager Download" on this page sends an nxm link back, which installs it
        nexus_commands::expect_nxm(&app, mod_id, file.file_id);
        return Ok(UpdateOutcome::OpenPage {
            url: nexus::file_page(mod_id, file.file_id),
        });
    }
    nexus_commands::start(app, mod_id, file.file_id, None);
    Ok(UpdateOutcome::Queued)
}

/// What changed between the installed version of a mod and its update.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Changelog {
    /// "github" or "nexus".
    pub source: &'static str,
    /// Newest first.
    pub versions: Vec<ChangelogVersion>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangelogVersion {
    pub version: String,
    pub lines: Vec<String>,
}

/// Lines kept per version; release notes can run to whole manuals.
const CHANGELOG_LINES: usize = 40;

/// The changes since the installed version of package `id`, from GitHub release notes
/// or the Nexus changelog; `None` when neither has any.
#[tauri::command]
pub async fn mod_changelog(app: AppHandle, id: String) -> AppResult<Option<Changelog>> {
    let state = app.state::<AppState>();
    let pkg = library::read_package(id.clone(), library::package_path(&state.store_root, &id)?)
        .ok_or_else(|| AppError::NotFound(id.clone()))?;
    let repos = state.settings.get().github_repos;
    let found = {
        let report = state.updates.lock().unwrap();
        pkg.mods.iter().find_map(|m| {
            let info = report.mods.get(&m.manifest.unique_id.to_lowercase())?;
            Some((
                m.manifest.version.clone(),
                info.version.clone()?,
                info.nexus_id
                    .or_else(|| info.url.as_deref().and_then(nexus::mod_page_id)),
                repos
                    .get(&m.manifest.unique_id.to_lowercase())
                    .cloned()
                    .or_else(|| updates::github_key(&m.manifest.update_keys))
                    .or_else(|| info.url.as_deref().and_then(updates::github_repo)),
            ))
        })
    };
    let Some((installed, target, nexus_id, github)) = found else {
        return Ok(None);
    };
    let notes = match &github {
        Some(repo) => updates::github_notes(&state.http, repo, &target)
            .await
            .ok()
            .flatten(),
        None => None,
    };
    if let Some(notes) = notes {
        return Ok(Some(Changelog {
            source: "github",
            versions: vec![ChangelogVersion {
                version: target,
                lines: notes
                    .lines()
                    .take(CHANGELOG_LINES)
                    .map(str::to_owned)
                    .collect(),
            }],
        }));
    }
    let (Some(mod_id), Some(key)) = (nexus_id, nexus::load_key()?) else {
        return Ok(None);
    };
    let all = nexus::changelog(&state.http, &key, mod_id).await?;
    let newer = |v: &str| {
        crate::version::compare(v, &installed).is_gt()
            && crate::version::compare(v, &target).is_le()
    };
    let mut versions: Vec<ChangelogVersion> = all
        .into_iter()
        .filter(|(v, _)| newer(v))
        .map(|(version, lines)| ChangelogVersion {
            version,
            lines: lines.into_iter().take(CHANGELOG_LINES).collect(),
        })
        .collect();
    versions.sort_by(|a, b| crate::version::compare(&b.version, &a.version));
    Ok((!versions.is_empty()).then_some(Changelog {
        source: "nexus",
        versions,
    }))
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

/// What the last played save would lose if the game started now with the active profile.
#[tauri::command]
pub async fn game_preflight(app: AppHandle) -> AppResult<Option<crate::save_usage::Preflight>> {
    let data = crate::commands::game_data_dir(&app)?;
    blocking(app, move |s| {
        let skipped: Vec<String> =
            crate::smapi_log::read(&data.join("ErrorLogs/SMAPI-latest.txt"))?
                .skipped
                .into_iter()
                .map(|m| m.name)
                .collect();
        Ok(crate::save_usage::preflight(
            &data.join("Saves"),
            &library::scan(&s.store_root),
            &s.profiles.get().active().enabled,
            &skipped,
        ))
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
        Ok(crate::modlist::status(list, &library::scan(&s.store_root)))
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

/// Finds mod `unique_id`, which another mod or a mod list needs, and installs it like an
/// update; a list passes the `update_keys` and `github` repo it recorded.
#[tauri::command]
pub async fn mod_install_dependency(
    app: AppHandle,
    unique_id: String,
    update_keys: Option<Vec<String>>,
    github: Option<String>,
) -> AppResult<UpdateOutcome> {
    let (smapi, game) = {
        let s = app.state::<AppState>();
        versions(&s)
    };
    let http = app.state::<AppState>().http.clone();
    let found = updates::lookup(
        &http,
        &unique_id,
        update_keys.unwrap_or_default(),
        &smapi,
        &game,
    )
    .await?;
    let Some(f) = found else {
        return Ok(UpdateOutcome::OpenPage {
            url: "https://smapi.io/mods".to_owned(),
        });
    };
    fetch_and_install(
        app,
        Wanted {
            name: f.name,
            version: f.version,
            url: f.url,
            nexus_id: f.nexus_id,
            github: github.or(f.github),
            subkey: None,
        },
    )
    .await
}

/// Attaches GitHub repo `repo` ("owner/repo" or a github.com link) to the mod
/// `unique_id`, or with `None` detaches it.
#[tauri::command]
pub async fn mod_set_github(
    app: AppHandle,
    unique_id: String,
    repo: Option<String>,
) -> AppResult<LibraryView> {
    let repo = match repo.as_deref().map(str::trim).filter(|r| !r.is_empty()) {
        Some(typed) => {
            Some(updates::parse_repo(typed).ok_or_else(|| AppError::BadRepo(typed.to_owned()))?)
        }
        None => None,
    };
    blocking(app, move |s| {
        let key = unique_id.to_lowercase();
        s.settings.update(|st| match repo {
            Some(r) => {
                st.github_repos.insert(key, r);
            }
            None => {
                st.github_repos.remove(&key);
            }
        })?;
        Ok(build_view(s))
    })
    .await
}

/// Restores the newest backup of the mod `unique_id`; the current version is backed up.
#[tauri::command]
pub async fn mod_rollback(app: AppHandle, unique_id: String) -> AppResult<LibraryView> {
    blocking(app, move |s| {
        ensure_stopped(s)?;
        let _guard = lock_library(s);
        let backup = backups::list(&s.backup_root, &unique_id)
            .into_iter()
            .next()
            .ok_or_else(|| AppError::NotFound(unique_id.clone()))?;
        let mut done = Vec::new();
        let result =
            library::install_dir_into(&s.store_root, &s.backup_root, &backup.path, &mut done);
        record_installs(s, &done)?;
        result?;
        backups::discard(&s.backup_root, &backup)?;
        drop(_guard);
        Ok(build_view(s))
    })
    .await
}
