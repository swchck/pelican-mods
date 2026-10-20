//! Commands for the game install, SMAPI and running the game.

use std::path::PathBuf;
use std::sync::atomic::Ordering;

use serde::Deserialize;
use tauri::{AppHandle, Emitter, Manager, State};

use super::{AppState, EventSink, blocking, game_data_dir};
use crate::error::{AppError, AppResult};
use crate::game::{self, GameInstall};
use crate::launcher::{LogLine, RunState, SmapiRun};
use crate::library;
use crate::smapi::{self, Release};

/// The game's lists for calendars and fish: the companion's from the last game, or the
/// built-in ones in the game's language before it has sent any with these lists.
#[tauri::command]
pub fn game_catalog(app: AppHandle, state: State<AppState>) -> crate::perfection::Catalog {
    match state.companion.catalog() {
        Some(c) if !c.fish_info.is_empty() => c,
        _ => {
            let language = game_data_dir(&app)
                .ok()
                .and_then(|d| crate::perfection::game_language(&d))
                .unwrap_or_else(|| "en".to_owned());
            crate::perfection::Catalog::builtin(&language).clone()
        }
    }
}

/// What SMAPI's log from the last session says went wrong.
#[tauri::command]
pub fn smapi_log_report(app: AppHandle) -> AppResult<crate::smapi_log::LogReport> {
    crate::smapi_log::read(&game_data_dir(&app)?.join("ErrorLogs/SMAPI-latest.txt"))
}

#[tauri::command]
pub fn game_detect(app: AppHandle) -> AppResult<Vec<GameInstall>> {
    let home = app
        .path()
        .home_dir()
        .map_err(|e| AppError::Io(std::io::Error::other(e)))?;
    Ok(game::detect(&home)
        .iter()
        .filter_map(|p| game::inspect(p).ok())
        .collect())
}

#[tauri::command]
pub fn game_current(state: State<AppState>) -> Option<GameInstall> {
    let path = state.settings.get().game_path?;
    game::inspect(&path).ok()
}

#[tauri::command]
pub fn game_set_path(state: State<AppState>, path: PathBuf) -> AppResult<GameInstall> {
    let install = game::inspect(&path)?;
    state
        .settings
        .update(|s| s.game_path = Some(install.path.clone()))?;
    Ok(install)
}

#[tauri::command]
pub async fn smapi_latest(state: State<'_, AppState>) -> AppResult<Release> {
    smapi::latest(&state.http).await
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SmapiAction {
    Install,
    Uninstall,
}

#[tauri::command]
pub async fn smapi_run(
    app: AppHandle,
    state: State<'_, AppState>,
    action: SmapiAction,
) -> AppResult<GameInstall> {
    let game_dir = state.game_dir()?;
    if matches!(state.launcher.state(), RunState::Running { .. }) {
        return Err(AppError::AlreadyRunning);
    }
    if state.installing.swap(true, Ordering::SeqCst) {
        return Err(AppError::Busy);
    }
    let res = async {
        let release = smapi::latest(&state.http).await?;
        let action = match action {
            SmapiAction::Install => smapi::Action::Install,
            SmapiAction::Uninstall => smapi::Action::Uninstall,
        };
        smapi::run(
            &state.http,
            &release,
            action,
            &game_dir,
            &state.work_dir,
            |p| {
                let _ = app.emit("smapi:progress", p);
            },
        )
        .await?;
        game::inspect(&game_dir)
    }
    .await;
    state.installing.store(false, Ordering::SeqCst);
    res
}

// async so tokio::process has a reactor; sync commands run on the main thread
#[tauri::command]
pub async fn game_launch(
    app: AppHandle,
    state: State<'_, AppState>,
    modded: bool,
) -> AppResult<RunState> {
    let game_dir = state.game_dir()?;
    if state.installing.load(Ordering::SeqCst) {
        return Err(AppError::Busy);
    }
    // the snapshot must not keep anyone from playing; a failure is reported, not fatal
    if let Err(e) = crate::commands::saves::snapshot_before_launch(&app).await {
        let _ = app.emit("saves:backup-failed", &e);
    }
    let smapi = if modded {
        let companion = state
            .companion_enabled()
            .then(|| state.companion.url())
            .flatten();
        Some(SmapiRun {
            env: companion
                .map(|url| (crate::companion::URL_VAR.to_owned(), url.to_owned()))
                .into_iter()
                .collect(),
            log: game_data_dir(&app)?.join("ErrorLogs/SMAPI-latest.txt"),
            mods_path: Some(crate::commands::library::deploy_active(&state)?),
        })
    } else {
        None
    };
    state.launcher.launch(&game_dir, smapi, EventSink::new(app))
}

#[tauri::command]
pub fn game_stop(state: State<AppState>) -> AppResult<()> {
    state.launcher.stop()
}

#[tauri::command]
pub fn game_state(state: State<AppState>) -> RunState {
    state.launcher.state()
}

#[tauri::command]
pub fn game_log(state: State<AppState>) -> Vec<LogLine> {
    state.launcher.log()
}

#[tauri::command]
pub async fn game_console(state: State<'_, AppState>, line: String) -> AppResult<()> {
    state.launcher.send(&line).await
}

/// The error the last session ended on, from SMAPI's crash log when it wrote one.
#[tauri::command]
pub async fn game_crash(app: AppHandle) -> AppResult<Option<crate::smapi_log::Crash>> {
    let logs = crate::commands::game_data_dir(&app)?.join("ErrorLogs");
    blocking(app, move |s| {
        let modified = |f: &str| {
            std::fs::metadata(logs.join(f))
                .and_then(|m| m.modified())
                .ok()
        };
        // SMAPI copies the log to SMAPI-crash.txt when the game dies; an older copy is
        // from some earlier crash
        let file = match (modified("SMAPI-crash.txt"), modified("SMAPI-latest.txt")) {
            (Some(crash), Some(latest)) if crash >= latest => "SMAPI-crash.txt",
            (Some(_), None) => "SMAPI-crash.txt",
            _ => "SMAPI-latest.txt",
        };
        let Ok(text) = std::fs::read_to_string(logs.join(file)) else {
            return Ok(None);
        };
        let assemblies: Vec<(String, String)> = library::scan(&s.store_root)
            .iter()
            .flat_map(|p| &p.mods)
            .filter_map(|m| {
                let dll = m.manifest.entry_dll.as_deref()?;
                let stem = dll.strip_suffix(".dll").unwrap_or(dll);
                Some((stem.to_owned(), m.manifest.name.clone()))
            })
            .collect();
        Ok(crate::smapi_log::last_error(&text, &assemblies))
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
        let profiles = s.profiles.get();
        let saves_dir = data.join("Saves");
        let mut found = crate::save_usage::preflight(
            &saves_dir,
            &library::scan(&s.store_root),
            &profiles.active().enabled,
            &skipped,
        );
        // the newest save remembers its own profile; offer it even when nothing's missing
        let newest = crate::saves::list(&saves_dir)
            .into_iter()
            .max_by_key(|s| s.modified);
        let other = newest.as_ref().and_then(|save| {
            profiles
                .save_profiles
                .get(&save.id)
                .filter(|p| **p != profiles.active && profiles.profiles.iter().any(|x| x.id == **p))
                .cloned()
        });
        if let Some(profile) = other {
            let save = newest.expect("a profile was found for it");
            let pf = found.get_or_insert_with(|| crate::save_usage::Preflight {
                save_id: save.id,
                farmer: save.farmer,
                farm_name: save.farm_name,
                missing: Vec::new(),
                switch_to: None,
            });
            pf.switch_to = Some(profile);
        }
        Ok(found)
    })
    .await
}
