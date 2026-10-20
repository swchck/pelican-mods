//! Tauri commands, grouped by the screen or subsystem they serve, and the state they share.

pub mod app;
pub mod companion;
pub mod config;
pub mod game;
pub mod library;
pub mod modlist;
pub mod nexus;
pub mod saves;
pub mod updates;

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;

use tauri::{AppHandle, Emitter, Manager};

use crate::error::{AppError, AppResult};
use crate::launcher::{Launcher, LogLine, RunState, Sink};
use crate::profiles::ProfilesStore;
use crate::settings::SettingsStore;
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
    pub(crate) save_progress: crate::commands::saves::ProgressCache,
    pub(crate) save_facts: crate::commands::saves::FactsCache,
    pub(crate) profiles: ProfilesStore,
    /// Serializes store and profile changes; installs run on blocking threads in parallel.
    pub(crate) library_lock: Mutex<()>,
    pub(crate) companion: std::sync::Arc<crate::companion::Companion>,
    /// Nexus files the app sent the user to fetch, with when; their nxm links install
    /// without a second question.
    pub(crate) nxm_expected: Mutex<std::collections::HashMap<(u64, u64), std::time::Instant>>,
    /// Installs, updates and removals, for the library's history and undo.
    pub(crate) history: crate::history::History,
    /// Removed packages, kept so a removal can be undone.
    pub(crate) removed_root: PathBuf,
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
            history: crate::history::History::load(data_dir.join("history.json")),
            removed_root: data_dir.join("removed"),
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

/// How long lines collect before they go out together.
const LOG_BATCH: std::time::Duration = std::time::Duration::from_millis(80);

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

// the game keeps its data under ~/.config on macOS too, a Linux habit of MonoGame
pub(crate) fn game_data_dir(app: &AppHandle) -> AppResult<PathBuf> {
    let home = app
        .path()
        .home_dir()
        .map_err(|e| AppError::Io(std::io::Error::other(e)))?;
    Ok(home.join(".config/StardewValley"))
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
