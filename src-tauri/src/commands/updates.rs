//! Commands that find, fetch and roll back mod updates.

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use super::library::{LibraryView, build_view, record_installs};
use super::{AppState, blocking, ensure_stopped, lock_library};
use crate::backups;
use crate::downloads;
use crate::error::{AppError, AppResult};
use crate::game;
use crate::library;
use crate::moddrop;
use crate::nexus;
use crate::updates::{self, UpdateReport};

/// SMAPI's compatibility verdicts for Nexus mods `ids`; fine mods are left out.
#[tauri::command]
pub async fn compat_for_nexus(
    state: State<'_, AppState>,
    ids: Vec<u64>,
) -> AppResult<std::collections::HashMap<u64, crate::compat::Verdict>> {
    let cache = state.updates_file.with_file_name("compat.jsonc");
    let mut all = crate::compat::verdicts(&state.http, &cache).await?;
    all.retain(|id, _| ids.contains(id));
    Ok(all)
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
    let premium = crate::commands::nexus::nexus_account(app.clone())
        .await
        .ok()
        .flatten()
        .is_some_and(|a| a.is_premium);
    if !premium {
        // "Mod Manager Download" on this page sends an nxm link back, which installs it
        crate::commands::nexus::expect_nxm(&app, mod_id, file.file_id);
        return Ok(UpdateOutcome::OpenPage {
            url: nexus::file_page(mod_id, file.file_id),
        });
    }
    crate::commands::nexus::start(app, mod_id, file.file_id, None);
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
    // keys and repos may come from a mod list someone shared: where smapi.io knows the
    // mod by its id alone, only its sources count, or a list could ship anyone's code
    // under a trusted mod's id
    let known = updates::lookup(&http, &unique_id, Vec::new(), &smapi, &game).await?;
    let (found, github) = match known {
        Some(f) => {
            let repo = f.github.clone();
            (Some(f), repo)
        }
        None => {
            let keys = update_keys.unwrap_or_default();
            let f = if keys.is_empty() {
                None
            } else {
                updates::lookup(&http, &unique_id, keys, &smapi, &game).await?
            };
            let repo = github.as_deref().and_then(updates::parse_repo);
            let repo = repo.or_else(|| f.as_ref().and_then(|f| f.github.clone()));
            (f, repo)
        }
    };
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
            github,
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
