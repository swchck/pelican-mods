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
    /// The search for a problem's mod, while one runs.
    pub bisect: Option<crate::bisect::Bisect>,
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
    /// The player's labels for it.
    pub tags: Vec<String>,
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
        tags: profiles.tags.get(&p.id).cloned().unwrap_or_default(),
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
        bisect: state.bisect.get(),
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
    // an update brings the mod's folder fresh, without the player's translation in it
    crate::translations::reapply(&state.translations_root, &library::scan(&state.store_root));
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
        remove_package(s, &id)?;
        drop(_guard);
        Ok(build_view(s))
    })
    .await
}

// sets package `id` aside with a history entry that can bring it back; hold the library lock
fn remove_package(s: &AppState, id: &str) -> AppResult<()> {
    let path = library::package_path(&s.store_root, id)?;
    let pkg = library::read_package(id.to_owned(), path.clone());
    let at = crate::history::now_ms();
    let was_enabled = s.profiles.get().active().enabled.iter().any(|e| e == id);
    // kept aside rather than deleted, so the history can put it back
    let stash = crate::history::stash(&path, &s.removed_root, at)?;
    s.profiles.update(|p| {
        p.forget_package(id);
        Ok(())
    })?;
    let first = pkg.as_ref().and_then(|p| p.mods.first());
    s.history.push(crate::history::Entry {
        at,
        kind: crate::history::Kind::Removed,
        package: id.to_owned(),
        name: first.map_or_else(|| id.to_owned(), |m| m.manifest.name.clone()),
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
    // an import copies from the game's Mods folder; the original would come back as
    // importable and still load without the launcher. Only once the package is safely
    // aside: a failed Trash then just leaves it importable again
    let ids: std::collections::HashSet<String> = pkg
        .as_ref()
        .map(|p| p.unique_ids().map(str::to_lowercase).collect())
        .unwrap_or_default();
    let (_, others) = game_mods(s);
    for p in others
        .iter()
        .filter(|p| !p.mods.is_empty() && p.unique_ids().all(|u| ids.contains(&u.to_lowercase())))
    {
        let _ = trash::delete(&p.path);
    }
    Ok(())
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
        // its folder may be the one the running game reads its mods from
        ensure_stopped(s)?;
        s.profiles.update(|p| p.delete(&id))?;
        let _ = std::fs::remove_dir_all(s.deploy_root.join(&id));
        let _ = std::fs::remove_dir_all(s.profile_configs_root.join(&id));
        Ok(build_view(s))
    })
    .await
}

/// Labels package `id` with `tags`, replacing its old ones.
#[tauri::command]
pub async fn library_set_tags(
    app: AppHandle,
    id: String,
    tags: Vec<String>,
) -> AppResult<LibraryView> {
    blocking(app, move |s| {
        s.profiles.update(|p| {
            p.set_tags(&id, &tags);
            Ok(())
        })?;
        Ok(build_view(s))
    })
    .await
}

/// Starts looking for the mod behind a problem among the active profile's mods: the
/// first round's half goes into a profile of its own named `name`, which becomes active.
#[tauri::command]
pub async fn bisect_start(app: AppHandle, name: String) -> AppResult<LibraryView> {
    blocking(app, move |s| {
        ensure_stopped(s)?;
        let _guard = lock_library(s);
        if s.bisect.get().is_some() {
            return Err(AppError::Bisecting);
        }
        let packages = library::scan(&s.store_root);
        let profiles = s.profiles.get();
        let original = profiles.active.clone();
        let enabled = profiles.active().enabled.clone();
        let mut search = None;
        s.profiles.update(|p| {
            let id = p.create(&name, None)?;
            let b = crate::bisect::Bisect::start(original.clone(), id.clone(), &enabled, &packages);
            p.set_profile_enabled(&id, b.enabled())?;
            p.activate(&id)?;
            search = Some(b);
            Ok(())
        })?;
        s.bisect.set(search.as_ref())?;
        Ok(build_view(s))
    })
    .await
}

/// Starts looking for the mod that `package` only breaks together with, among
/// `candidates` (the mods the finished search cleared), in a profile named `name`.
#[tauri::command]
pub async fn bisect_partner(
    app: AppHandle,
    name: String,
    package: String,
    candidates: Vec<String>,
) -> AppResult<LibraryView> {
    blocking(app, move |s| {
        ensure_stopped(s)?;
        let _guard = lock_library(s);
        if s.bisect.get().is_some() {
            return Err(AppError::Bisecting);
        }
        let original = s.profiles.get().active.clone();
        let mut search = None;
        s.profiles.update(|p| {
            let id = p.create(&name, None)?;
            let b = crate::bisect::Bisect::partner(
                original.clone(),
                id.clone(),
                package.clone(),
                &candidates,
            );
            p.set_profile_enabled(&id, b.enabled())?;
            p.activate(&id)?;
            search = Some(b);
            Ok(())
        })?;
        s.bisect.set(search.as_ref())?;
        Ok(build_view(s))
    })
    .await
}

/// Where a search stands after an answer, with the library as it now is.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BisectOutcome {
    pub step: crate::bisect::Step,
    pub view: LibraryView,
}

/// Answers whether the problem showed in this round; a finished search goes back to the
/// profile it started from.
#[tauri::command]
pub async fn bisect_answer(app: AppHandle, problem: bool) -> AppResult<BisectOutcome> {
    blocking(app, move |s| {
        ensure_stopped(s)?;
        let _guard = lock_library(s);
        let mut b = s
            .bisect
            .get()
            .ok_or_else(|| AppError::NotFound("bisect".into()))?;
        let step = b.answer(problem, &library::scan(&s.store_root));
        match &step {
            crate::bisect::Step::Next { enabled } => {
                s.profiles
                    .update(|p| p.set_profile_enabled(&b.profile, enabled.clone()))?;
                s.bisect.set(Some(&b))?;
            }
            _ => end_bisect(s, &b)?,
        }
        Ok(BisectOutcome {
            step,
            view: build_view(s),
        })
    })
    .await
}

/// Stops a search and goes back to the profile it started from.
#[tauri::command]
pub async fn bisect_cancel(app: AppHandle) -> AppResult<LibraryView> {
    blocking(app, move |s| {
        ensure_stopped(s)?;
        let _guard = lock_library(s);
        if let Some(b) = s.bisect.get() {
            end_bisect(s, &b)?;
        }
        Ok(build_view(s))
    })
    .await
}

fn end_bisect(s: &AppState, b: &crate::bisect::Bisect) -> AppResult<()> {
    s.profiles.update(|p| {
        // the original may be gone if the player deleted it mid-search
        let _ = p.activate(&b.original);
        let _ = p.delete(&b.profile);
        Ok(())
    })?;
    let _ = std::fs::remove_dir_all(s.deploy_root.join(&b.profile));
    s.bisect.set(None)
}

/// Makes profile `id` the active one; refused while the game runs, since the session's
/// saves are remembered with the active profile when it ends.
#[tauri::command]
pub async fn profile_activate(app: AppHandle, id: String) -> AppResult<LibraryView> {
    blocking(app, move |s| {
        ensure_stopped(s)?;
        // a search's rounds live in its own profile; switching away would lose track
        if s.bisect.get().is_some() {
            return Err(AppError::Bisecting);
        }
        let _guard = lock_library(s);
        let before = s.profiles.get();
        let packages = library::scan(&s.store_root);
        // the profile left keeps the settings it was played with
        if before.active().own_configs {
            crate::profile_configs::snapshot(
                &packages,
                &s.profile_configs_root.join(&before.active),
            )?;
        }
        s.profiles.update(|p| p.activate(&id))?;
        let after = s.profiles.get();
        if after.active().own_configs {
            crate::profile_configs::restore(
                &s.store_root,
                &s.profile_configs_root.join(&after.active),
            )?;
        }
        drop(_guard);
        Ok(build_view(s))
    })
    .await
}

/// Gives profile `id` mod settings of its own (starting from the current ones), or makes
/// it share them with the other profiles again.
#[tauri::command]
pub async fn profile_set_own_configs(
    app: AppHandle,
    id: String,
    on: bool,
) -> AppResult<LibraryView> {
    blocking(app, move |s| {
        ensure_stopped(s)?;
        let _guard = lock_library(s);
        s.profiles.update(|p| p.set_own_configs(&id, on))?;
        let dir = s.profile_configs_root.join(&id);
        if on {
            crate::profile_configs::snapshot(&library::scan(&s.store_root), &dir)?;
        } else {
            let _ = std::fs::remove_dir_all(dir);
        }
        drop(_guard);
        Ok(build_view(s))
    })
    .await
}

/// A history entry and whether [`history_undo`] can still take it back.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItem {
    #[serde(flatten)]
    pub entry: crate::history::Entry,
    pub can_undo: bool,
}

/// The library's installs, updates and removals, newest first.
#[tauri::command]
pub fn history_list(app: AppHandle) -> Vec<HistoryItem> {
    let s = app.state::<AppState>();
    s.history
        .list()
        .into_iter()
        .map(|entry| {
            let can_undo = !entry.undone
                && match entry.kind {
                    crate::history::Kind::Installed => true,
                    crate::history::Kind::Updated => previous_backup(&s, &entry).is_some(),
                    crate::history::Kind::Removed => {
                        entry.stash.as_ref().is_some_and(|p| p.exists())
                    }
                };
            HistoryItem { entry, can_undo }
        })
        .collect()
}

// the copy an update replaced, by version: the newest backup may belong to a later update
fn previous_backup(s: &AppState, e: &crate::history::Entry) -> Option<backups::Backup> {
    let previous = e.previous.as_deref()?;
    backups::list(&s.backup_root, &e.unique_id)
        .into_iter()
        .find(|b| b.version == previous)
}

/// Undoes history entry `at`: removes an install, rolls an update back, or puts a
/// removed package back where it was.
#[tauri::command]
pub async fn history_undo(app: AppHandle, at: u64) -> AppResult<LibraryView> {
    blocking(app, move |s| {
        ensure_stopped(s)?;
        // before reading the entry: a second click waiting here must see it undone
        let _guard = lock_library(s);
        let e = s
            .history
            .get(at)
            .filter(|e| !e.undone)
            .ok_or_else(|| AppError::NotFound(at.to_string()))?;
        match e.kind {
            // a removal of its own, so undoing the install can be undone too
            crate::history::Kind::Installed => remove_package(s, &e.package)?,
            crate::history::Kind::Updated => {
                let backup = previous_backup(s, &e).ok_or_else(|| {
                    AppError::NotFound(format!(
                        "{} {}",
                        e.unique_id,
                        e.previous.as_deref().unwrap_or("")
                    ))
                })?;
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
                let reinstalled = library::scan(&s.store_root).iter().any(|p| {
                    p.unique_ids()
                        .any(|id| id.eq_ignore_ascii_case(&e.unique_id))
                });
                if dest.exists() || reinstalled {
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
