//! Tauri commands: the webview's whole view of the backend.

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Deserialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

use crate::error::{AppError, AppResult};
use crate::game::{self, GameInstall};
use crate::launcher::{Launcher, LogLine, RunState, Sink, SmapiRun};
use crate::profiles::ProfilesStore;
use crate::settings::{Settings, SettingsStore};
use crate::smapi::{self, Release};
use crate::updates::UpdateReport;

/// Shared backend state managed by Tauri.
pub struct AppState {
    settings: SettingsStore,
    launcher: Launcher,
    pub(crate) http: reqwest::Client,
    pub(crate) work_dir: PathBuf,
    installing: AtomicBool,
    /// Installed mod packages, one folder each.
    pub(crate) store_root: PathBuf,
    /// Per-profile symlink folders handed to SMAPI.
    pub(crate) deploy_root: PathBuf,
    /// Packages replaced by updates, for rollback.
    pub(crate) backup_root: PathBuf,
    /// Last smapi.io answer, also kept on disk between runs.
    pub(crate) updates: Mutex<UpdateReport>,
    pub(crate) updates_file: PathBuf,
    pub(crate) downloads: crate::downloads::Downloads,
    /// Account of the stored Nexus key, once validated this session.
    pub(crate) nexus_account: Mutex<Option<crate::nexus::Account>>,
    pub(crate) profiles: ProfilesStore,
    /// Serializes store and profile changes; installs run on blocking threads in parallel.
    pub(crate) library_lock: Mutex<()>,
}

impl AppState {
    /// Builds state rooted at the app's config, data and cache folders.
    pub fn new(config_dir: PathBuf, data_dir: PathBuf, cache_dir: PathBuf) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(concat!("PelicanMods/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("http client");
        Self {
            settings: SettingsStore::load(config_dir.join("settings.json")),
            launcher: Launcher::default(),
            http,
            work_dir: cache_dir.join("smapi-installer"),
            installing: AtomicBool::new(false),
            store_root: data_dir.join("mods"),
            deploy_root: data_dir.join("profiles"),
            backup_root: data_dir.join("backups"),
            updates: Mutex::new(crate::updates::load_cache(&cache_dir.join("updates.json"))),
            updates_file: cache_dir.join("updates.json"),
            downloads: Default::default(),
            nexus_account: Mutex::new(None),
            profiles: ProfilesStore::load(config_dir.join("profiles.json")),
            library_lock: Mutex::new(()),
        }
    }

    pub(crate) fn game_dir(&self) -> AppResult<PathBuf> {
        self.settings
            .get()
            .game_path
            .ok_or(AppError::GameNotConfigured)
    }
}

struct EventSink(AppHandle);

impl Sink for EventSink {
    fn log(&self, line: &LogLine) {
        let _ = self.0.emit("game:log", line);
    }
    fn state(&self, state: &RunState) {
        let _ = self.0.emit("game:state", state);
    }
}

#[tauri::command]
pub fn settings_get(state: State<AppState>) -> Settings {
    state.settings.get()
}

#[tauri::command]
pub fn settings_set_locale(state: State<AppState>, locale: Option<String>) -> AppResult<Settings> {
    state.settings.update(|s| s.locale = locale)
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
    let smapi = if modded {
        Some(SmapiRun {
            log: game_data_dir(&app)?.join("ErrorLogs/SMAPI-latest.txt"),
            mods_path: Some(crate::mods_commands::deploy_active(&state)?),
        })
    } else {
        None
    };
    state.launcher.launch(&game_dir, smapi, EventSink(app))
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

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Place {
    Game,
    Mods,
    SmapiLog,
    Saves,
}

#[tauri::command]
pub fn open_place(app: AppHandle, state: State<AppState>, place: Place) -> AppResult<()> {
    let data = game_data_dir(&app)?;
    let path = match place {
        Place::Game => state.game_dir()?,
        Place::Mods => state.game_dir()?.join("Mods"),
        Place::SmapiLog => data.join("ErrorLogs/SMAPI-latest.txt"),
        Place::Saves => data.join("Saves"),
    };
    if !path.exists() {
        return Err(AppError::GameNotFound(path.display().to_string()));
    }
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(|e| AppError::Io(std::io::Error::other(e)))
}

#[tauri::command]
pub async fn rosetta_install() -> AppResult<()> {
    crate::rosetta::install().await
}

// the game keeps its data under ~/.config on macOS too, a Linux habit of MonoGame
fn game_data_dir(app: &AppHandle) -> AppResult<PathBuf> {
    let home = app
        .path()
        .home_dir()
        .map_err(|e| AppError::Io(std::io::Error::other(e)))?;
    Ok(home.join(".config/StardewValley"))
}
