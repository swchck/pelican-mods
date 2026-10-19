//! Tauri commands for editing mod settings.

use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use serde_json::Value;
use tauri::{AppHandle, Manager};

use crate::commands::{AppState, EventSink, game_data_dir};
use crate::error::{AppError, AppResult};
use crate::launcher::{RunState, SmapiRun};
use crate::library;
use crate::mod_config::{self, ModConfig};
use crate::mods_commands::{blocking, deploy_active};

/// How long a config-generating run may take to reach "Mods loaded and ready".
const GENERATE_TIMEOUT: Duration = Duration::from_secs(90);

/// Resolves a mod folder inside a stored package, refusing paths that leave it.
fn mod_dir(state: &AppState, package: &str, rel_path: &str) -> AppResult<PathBuf> {
    let root = library::package_path(&state.store_root, package)?;
    let rel = Path::new(rel_path);
    if rel.components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err(AppError::NotFound(rel_path.to_owned()));
    }
    let dir = root.join(rel);
    if !dir.is_dir() {
        return Err(AppError::NotFound(rel_path.to_owned()));
    }
    Ok(dir)
}

#[tauri::command]
pub async fn mod_config_get(
    app: AppHandle,
    package: String,
    rel_path: String,
) -> AppResult<ModConfig> {
    blocking(app, move |s| {
        mod_config::load(&mod_dir(s, &package, &rel_path)?)
    })
    .await
}

#[tauri::command]
pub async fn mod_config_save(
    app: AppHandle,
    package: String,
    rel_path: String,
    values: Value,
) -> AppResult<ModConfig> {
    blocking(app, move |s| {
        let dir = mod_dir(s, &package, &rel_path)?;
        mod_config::save(&dir, &values)?;
        mod_config::load(&dir)
    })
    .await
}

/// Writes `config.json` from a content pack's schema defaults. Mods without a schema
/// keep their defaults in code and need [`mods_generate_configs`] instead.
#[tauri::command]
pub async fn mod_config_defaults(
    app: AppHandle,
    package: String,
    rel_path: String,
) -> AppResult<ModConfig> {
    blocking(app, move |s| {
        let dir = mod_dir(s, &package, &rel_path)?;
        let cfg = mod_config::load(&dir)?;
        let schema = cfg
            .schema
            .ok_or_else(|| AppError::BadConfig("no ConfigSchema to take defaults from".into()))?;
        mod_config::save(&dir, &mod_config::defaults(&schema))?;
        mod_config::load(&dir)
    })
    .await
}

/// Starts SMAPI with the active profile just long enough for every mod to load, which
/// is when mods write their default `config.json`, then stops the game.
#[tauri::command]
pub async fn mods_generate_configs(app: AppHandle) -> AppResult<()> {
    let state = app.state::<AppState>();
    let game_dir = state.game_dir()?;
    let mods_path = blocking(app.clone(), deploy_active).await?;
    state.launcher.launch(
        &game_dir,
        Some(SmapiRun {
            log: game_data_dir(&app)?.join("ErrorLogs/SMAPI-latest.txt"),
            mods_path: Some(mods_path),
        }),
        EventSink(app.clone()),
    )?;
    let started = std::time::Instant::now();
    let ready = loop {
        if state
            .launcher
            .log()
            .iter()
            .any(|l| l.text.contains("Mods loaded and ready"))
        {
            break true;
        }
        if !matches!(state.launcher.state(), RunState::Running { .. })
            || started.elapsed() > GENERATE_TIMEOUT
        {
            break false;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    };
    // mods write config.json during Entry, which finishes before SMAPI reports ready
    let _ = state.launcher.stop();
    while matches!(state.launcher.state(), RunState::Running { .. }) {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    if ready {
        Ok(())
    } else {
        Err(AppError::BadConfig(
            "SMAPI did not finish loading mods".into(),
        ))
    }
}
