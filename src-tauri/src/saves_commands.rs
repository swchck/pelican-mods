//! Tauri commands for save files.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use tauri::AppHandle;

use crate::commands::{AppState, game_data_dir};
use crate::error::{AppError, AppResult};
use crate::launcher::RunState;
use crate::mods_commands::blocking;
use crate::save_backups::{self, Reason, Snapshot};
use crate::saves::{self, Progress, SaveSummary};

/// Progress per save, valid while the save file's mtime is unchanged; parsing a big
/// save takes a second or two.
#[derive(Default)]
pub struct ProgressCache(Mutex<HashMap<String, (u64, Progress)>>);

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

/// Snapshots changed saves; called right before the game starts.
pub async fn snapshot_before_launch(app: &AppHandle) -> AppResult<usize> {
    let dir = saves_dir(app)?;
    blocking(app.clone(), move |s| {
        save_backups::snapshot_changed(&s.save_backup_root, &dir)
    })
    .await
}
