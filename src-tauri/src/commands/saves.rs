//! Tauri commands for save files.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use tauri::AppHandle;

use crate::commands::blocking;
use crate::commands::{AppState, game_data_dir};
use crate::error::{AppError, AppResult};
use crate::launcher::RunState;
use crate::perfection::{self, Perfection};
use crate::save_backups::{self, Reason, Snapshot};
use crate::saves::{self, Progress, SaveSummary};

/// Progress per save, valid while the save file's mtime is unchanged; parsing a big
/// save takes a second or two.
#[derive(Default)]
pub struct ProgressCache(Mutex<HashMap<String, (u64, Progress)>>);

/// Perfection facts per save, under the same mtime rule as [`ProgressCache`].
#[derive(Default)]
pub struct FactsCache(Mutex<HashMap<String, (u64, perfection::Facts)>>);

fn saves_dir(app: &AppHandle) -> AppResult<PathBuf> {
    Ok(game_data_dir(app)?.join("Saves"))
}

fn save_path(app: &AppHandle, id: &str) -> AppResult<PathBuf> {
    let plain = !id.is_empty() && !id.starts_with('.') && !id.contains(['/', '\\']);
    let path = saves_dir(app)?.join(id);
    if !plain || !path.join(saves::INFO_FILE).is_file() {
        return Err(AppError::NotFound(id.to_owned()));
    }
    Ok(path)
}

// rewriting a save under a running game loses whichever side writes last
fn ensure_stopped(state: &AppState) -> AppResult<()> {
    if matches!(state.launcher.state(), RunState::Running { .. }) {
        return Err(AppError::AlreadyRunning);
    }
    Ok(())
}

#[tauri::command]
pub async fn saves_list(app: AppHandle) -> AppResult<Vec<SaveSummary>> {
    let dir = saves_dir(&app)?;
    blocking(app, move |_| Ok(saves::list(&dir))).await
}

/// Scores save `id` for perfection, against the catalog the companion last sent
/// or, before any game has, the built-in one.
#[tauri::command]
pub async fn saves_perfection(app: AppHandle, id: String) -> AppResult<Perfection> {
    let path = save_path(&app, &id)?;
    let language = game_language(&app);
    blocking(app, move |s| {
        let facts = cached_facts(s, &id, &path)?;
        let catalog = s.companion.catalog();
        Ok(perfection::evaluate(
            &facts,
            Some(
                catalog
                    .as_ref()
                    .unwrap_or(perfection::Catalog::builtin(&language)),
            ),
        ))
    })
    .await
}

/// The detailed progress report for save `id`: achievements, bundles, museum and the rest.
#[tauri::command]
pub async fn saves_checkup(app: AppHandle, id: String) -> AppResult<crate::checkup::Checkup> {
    let path = save_path(&app, &id)?;
    let language = game_language(&app);
    blocking(app, move |s| {
        let facts = cached_facts(s, &id, &path)?;
        let catalog = s.companion.catalog();
        let season = ["spring", "summer", "fall", "winter"]
            .get(saves::summary(&path)?.season as usize)
            .copied();
        Ok(crate::checkup::build(
            &facts,
            Some(
                catalog
                    .as_ref()
                    .unwrap_or(perfection::Catalog::builtin(&language)),
            ),
            season,
        ))
    })
    .await
}

/// Everything save `id` keeps in its inventory, chests, fridges and auto-grabbers, as of
/// its last save; names come in the game's language where the game's lists have them.
#[tauri::command]
pub async fn saves_items(app: AppHandle, id: String) -> AppResult<Vec<NamedItem>> {
    let path = save_path(&app, &id)?;
    let language = game_language(&app);
    blocking(app, move |s| {
        let live = s.companion.catalog();
        let catalog = live
            .as_ref()
            .unwrap_or(perfection::Catalog::builtin(&language));
        let names: HashMap<&str, &str> = catalog
            .items
            .iter()
            .map(|i| (i.id.as_str(), i.name.as_str()))
            .collect();
        Ok(crate::chests::in_save(&path)?
            .into_iter()
            .map(|item| {
                let name = item
                    .id
                    .strip_prefix("(O)")
                    .and_then(|id| names.get(id))
                    .map_or_else(|| item.name.clone(), |n| (*n).to_owned());
                let loc = &item.place.location;
                let location = catalog
                    .term(&format!("location.{loc}"))
                    .or_else(|| catalog.term(&format!("building.{loc}")))
                    .map(str::to_owned);
                NamedItem {
                    display: name,
                    location_name: location,
                    item,
                }
            })
            .collect())
    })
    .await
}

/// Which of Ginger Island's golden walnut sources save `id` has found, as of its last save.
#[tauri::command]
pub async fn saves_walnuts(app: AppHandle, id: String) -> AppResult<crate::walnuts::Walnuts> {
    let path = save_path(&app, &id)?;
    blocking(app, move |_| crate::walnuts::in_save(&path)).await
}

/// Compares two states of save `id`: each side is "current", "previous" (the day before
/// the last save, which the game keeps as `_old` files) or the path of one of its snapshots.
#[tauri::command]
pub async fn saves_compare(
    app: AppHandle,
    id: String,
    from: String,
    to: String,
) -> AppResult<crate::save_diff::SaveDiff> {
    let path = save_path(&app, &id)?;
    let language = game_language(&app);
    blocking(app, move |s| {
        // old states unpack into the cache, never next to the real save
        let tmp = s
            .work_dir
            .with_file_name(format!("compare-{}", crate::history::now_ms()));
        let result = compare_sides(s, &tmp, &path, &id, &from, &to, &language);
        let _ = std::fs::remove_dir_all(&tmp);
        result
    })
    .await
}

fn compare_sides(
    s: &AppState,
    tmp: &std::path::Path,
    path: &std::path::Path,
    id: &str,
    from: &str,
    to: &str,
    language: &str,
) -> AppResult<crate::save_diff::SaveDiff> {
    let side = |which: &str, slot: &str| -> AppResult<PathBuf> {
        match which {
            "current" => Ok(path.to_path_buf()),
            "previous" => {
                let dir = tmp.join(slot).join(id);
                std::fs::create_dir_all(&dir)?;
                std::fs::copy(path.join(format!("{id}_old")), dir.join(id))?;
                std::fs::copy(path.join("SaveGameInfo_old"), dir.join(saves::INFO_FILE))?;
                Ok(dir)
            }
            file => crate::save_backups::unpack(
                &s.save_backup_root,
                std::path::Path::new(file),
                &tmp.join(slot),
            ),
        }
    };
    let (a, b) = (side(from, "from")?, side(to, "to")?);
    let live = s.companion.catalog();
    let catalog = live
        .as_ref()
        .unwrap_or(perfection::Catalog::builtin(language));
    crate::save_diff::compare(&a, &b, catalog)
}

/// A stored stack with the names the UI shows.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NamedItem {
    #[serde(flatten)]
    pub item: crate::chests::StoredItem,
    pub display: String,
    /// The location as the game names it, when its lists know it.
    pub location_name: Option<String>,
}

/// The game's own language, so built-in names match the bundle names the save carries.
fn game_language(app: &AppHandle) -> String {
    game_data_dir(app)
        .ok()
        .and_then(|dir| perfection::game_language(&dir))
        .unwrap_or_else(|| "en".to_owned())
}

fn cached_facts(s: &AppState, id: &str, path: &std::path::Path) -> AppResult<perfection::Facts> {
    let modified = saves::summary(path)?.modified;
    let cached = s
        .save_facts
        .0
        .lock()
        .unwrap()
        .get(id)
        .filter(|(m, _)| *m == modified)
        .map(|(_, f)| f.clone());
    if let Some(f) = cached {
        return Ok(f);
    }
    let f = perfection::facts(path)?;
    s.save_facts
        .0
        .lock()
        .unwrap()
        .insert(id.to_owned(), (modified, f.clone()));
    Ok(f)
}

#[tauri::command]
pub async fn saves_progress(app: AppHandle, id: String) -> AppResult<Progress> {
    let path = save_path(&app, &id)?;
    blocking(app, move |s| {
        let modified = saves::summary(&path)?.modified;
        if let Some((m, p)) = s.save_progress.0.lock().unwrap().get(&id)
            && *m == modified
        {
            return Ok(p.clone());
        }
        let p = saves::progress(&path)?;
        s.save_progress
            .0
            .lock()
            .unwrap()
            .insert(id, (modified, p.clone()));
        Ok(p)
    })
    .await
}

#[tauri::command]
pub async fn saves_snapshots(app: AppHandle, id: String) -> AppResult<Vec<Snapshot>> {
    // validates the id as a real save folder name before it becomes a path
    save_path(&app, &id)?;
    blocking(app, move |s| {
        Ok(save_backups::list(&s.save_backup_root, &id))
    })
    .await
}

#[tauri::command]
pub async fn saves_backup(app: AppHandle, id: String) -> AppResult<Snapshot> {
    let path = save_path(&app, &id)?;
    blocking(app, move |s| {
        save_backups::create(&s.save_backup_root, &path, Reason::Manual)
    })
    .await
}

#[tauri::command]
pub async fn saves_restore(app: AppHandle, file: PathBuf) -> AppResult<String> {
    let dir = saves_dir(&app)?;
    blocking(app, move |s| {
        ensure_stopped(s)?;
        save_backups::restore(&s.save_backup_root, &dir, &file)
    })
    .await
}

#[tauri::command]
pub async fn saves_export(app: AppHandle, id: String, dest: PathBuf) -> AppResult<()> {
    let path = save_path(&app, &id)?;
    blocking(app, move |_| save_backups::export(&path, &dest)).await
}

#[tauri::command]
pub async fn saves_import(app: AppHandle, file: PathBuf) -> AppResult<String> {
    let dir = saves_dir(&app)?;
    blocking(app, move |s| {
        ensure_stopped(s)?;
        save_backups::import(&s.save_backup_root, &dir, &file)
    })
    .await
}

/// Moves save `id` to the Trash, where Finder can put it back; its backups stay.
#[tauri::command]
pub async fn saves_delete(app: AppHandle, id: String) -> AppResult<()> {
    let path = save_path(&app, &id)?;
    blocking(app, move |s| {
        ensure_stopped(s)?;
        trash::delete(&path).map_err(|e| AppError::Io(std::io::Error::other(e)))
    })
    .await
}

/// How many copies of each save the backup folder keeps.
const MIRROR_KEEP: usize = 10;

/// Copies changed saves to the backup folder from the settings; 0 without one.
#[tauri::command]
pub async fn saves_mirror(app: AppHandle) -> AppResult<usize> {
    let dir = saves_dir(&app)?;
    blocking(app, move |s| match s.settings.get().backup_folder {
        Some(folder) => save_backups::mirror(&folder, &dir, MIRROR_KEEP),
        None => Ok(0),
    })
    .await
}

/// Picks the folder saves are copied to after each session, or with `None` stops copying.
#[tauri::command]
pub fn settings_set_backup_folder(
    state: tauri::State<AppState>,
    folder: Option<PathBuf>,
) -> AppResult<crate::settings::Settings> {
    if let Some(f) = folder.as_ref().filter(|f| !f.is_dir()) {
        return Err(AppError::NotFound(f.display().to_string()));
    }
    state.settings.update(|s| s.backup_folder = folder)
}

/// Snapshots changed saves; called right before the game starts.
pub async fn snapshot_before_launch(app: &AppHandle) -> AppResult<usize> {
    let dir = saves_dir(app)?;
    blocking(app.clone(), move |s| {
        save_backups::snapshot_changed(&s.save_backup_root, &dir)
    })
    .await
}
