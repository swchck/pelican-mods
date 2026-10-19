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
    pub(crate) settings: SettingsStore,
    pub(crate) launcher: Launcher,
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
    /// Zip snapshots of save folders.
    pub(crate) save_backup_root: PathBuf,
    pub(crate) save_progress: crate::saves_commands::ProgressCache,
    pub(crate) save_facts: crate::saves_commands::FactsCache,
    pub(crate) profiles: ProfilesStore,
    /// Serializes store and profile changes; installs run on blocking threads in parallel.
    pub(crate) library_lock: Mutex<()>,
    pub(crate) companion: std::sync::Arc<crate::companion::Companion>,
    /// Nexus files the app sent the user to fetch, with when; their nxm links install
    /// without a second question.
    pub(crate) nxm_expected: Mutex<std::collections::HashMap<(u64, u64), std::time::Instant>>,
    /// Unconfirmed nxm links the UI hasn't collected yet.
    pub(crate) nxm_pending: Mutex<Vec<serde_json::Value>>,
    /// The built companion mod, linked into every profile; `None` when this build lacks it.
    pub(crate) companion_dir: std::sync::OnceLock<PathBuf>,
}

impl AppState {
    /// Builds state rooted at the app's config, data and cache folders.
    pub fn new(config_dir: PathBuf, data_dir: PathBuf, cache_dir: PathBuf) -> Self {
        // no overall timeout: a 1 GB mod on a slow line is fine, a connection that stops
        // sending isn't, and without these it would hold the installing flag forever
        let http = reqwest::Client::builder()
            .user_agent(concat!("PelicanMods/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(std::time::Duration::from_secs(20))
            .read_timeout(std::time::Duration::from_secs(60))
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
            save_backup_root: data_dir.join("save-backups"),
            save_progress: Default::default(),
            save_facts: Default::default(),
            profiles: ProfilesStore::load(config_dir.join("profiles.json")),
            library_lock: Mutex::new(()),
            companion: std::sync::Arc::new(crate::companion::Companion::new(&cache_dir)),
            companion_dir: std::sync::OnceLock::new(),
            nxm_expected: Mutex::new(Default::default()),
            nxm_pending: Mutex::new(Vec::new()),
        }
    }

    /// Reports whether the companion mod ships with this build and the user keeps it on.
    pub(crate) fn companion_enabled(&self) -> bool {
        self.companion_dir.get().is_some() && !self.settings.get().companion_disabled
    }

    pub(crate) fn game_dir(&self) -> AppResult<PathBuf> {
        self.settings
            .get()
            .game_path
            .ok_or(AppError::GameNotConfigured)
    }
}

/// Sends console lines to the UI in batches: SMAPI writes thousands of lines while it
/// loads, and one IPC event plus one re-render per line kept the webview busy.
pub(crate) struct EventSink {
    app: AppHandle,
    batch: std::sync::Arc<Mutex<Vec<LogLine>>>,
}

/// How long lines collect before they go out together.
const LOG_BATCH: std::time::Duration = std::time::Duration::from_millis(80);

impl EventSink {
    pub(crate) fn new(app: AppHandle) -> Self {
        Self {
            app,
            batch: Default::default(),
        }
    }

    fn flush(app: &AppHandle, batch: &Mutex<Vec<LogLine>>) {
        let lines = std::mem::take(&mut *batch.lock().unwrap());
        if !lines.is_empty() {
            let _ = app.emit("game:log", lines);
        }
    }
}

impl Sink for EventSink {
    fn log(&self, line: &LogLine) {
        let first = {
            let mut batch = self.batch.lock().unwrap();
            batch.push(line.clone());
            batch.len() == 1
        };
        if first {
            let (app, batch) = (self.app.clone(), self.batch.clone());
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(LOG_BATCH).await;
                Self::flush(&app, &batch);
            });
        }
    }

    fn state(&self, state: &RunState) {
        // lines first: the UI treats "exited" as the end of the output
        Self::flush(&self.app, &self.batch);
        let _ = self.app.emit("game:state", state);
    }
}

#[tauri::command]
pub fn settings_get(state: State<AppState>) -> Settings {
    state.settings.get()
}

/// The language the game is set to, such as "ru", if it has ever started.
#[tauri::command]
pub fn game_language(app: AppHandle) -> Option<String> {
    crate::perfection::game_language(&game_data_dir(&app).ok()?)
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
    // the snapshot must not keep anyone from playing; a failure is reported, not fatal
    if let Err(e) = crate::saves_commands::snapshot_before_launch(&app).await {
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
            mods_path: Some(crate::mods_commands::deploy_active(&state)?),
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
pub(crate) fn game_data_dir(app: &AppHandle) -> AppResult<PathBuf> {
    let home = app
        .path()
        .home_dir()
        .map_err(|e| AppError::Io(std::io::Error::other(e)))?;
    Ok(home.join(".config/StardewValley"))
}

/// Fetches the changelog as published with release `version`, so an older app can show
/// what a newer one brings. `locale` "ru" picks the Russian file.
#[tauri::command]
pub async fn app_changelog(
    state: State<'_, AppState>,
    version: String,
    locale: String,
) -> AppResult<String> {
    let valid = !version.is_empty()
        && version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+'));
    if !valid {
        return Err(AppError::NotFound(version));
    }
    let file = if locale == "ru" {
        "CHANGELOG.ru.md"
    } else {
        "CHANGELOG.md"
    };
    let url = format!("https://raw.githubusercontent.com/{RELEASE_REPO}/v{version}/{file}");
    let resp = state.http.get(url).send().await?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(AppError::NotFound(format!("changelog for {version}")));
    }
    Ok(resp.error_for_status()?.text().await?)
}

/// Repository whose releases feed the updater; must match `plugins.updater.endpoints`.
const RELEASE_REPO: &str = "swchck/pelican-mods";
