//! Tauri glue for the companion link: events to the UI and the commands it calls.

use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};

use crate::commands::AppState;
use crate::companion::{self, Applied, Events, Status};
use crate::error::AppResult;
use crate::settings::Settings;

struct TauriEvents(AppHandle);

impl Events for TauriEvents {
    fn status(&self, status: &Status) {
        let _ = self.0.emit("companion:status", status);
    }

    fn gmcm(&self, ids: &[String]) {
        let _ = self.0.emit("companion:gmcm", ids);
    }
}

/// Finds the built companion and starts the server; call once from setup.
pub fn start(app: &AppHandle) {
    let state = app.state::<AppState>();
    if let Some(dir) = bundled_dir(app) {
        let _ = state.companion_dir.set(dir);
    }
    let server = state.companion.clone();
    let events = TauriEvents(app.clone());
    tauri::async_runtime::spawn(async move {
        if let Err(e) = server.serve(events).await {
            eprintln!("companion server stopped: {e}");
        }
    });
}

// release builds carry it as a resource; dev builds use the one `dotnet build` left in the repo
fn bundled_dir(app: &AppHandle) -> Option<std::path::PathBuf> {
    let resource = app
        .path()
        .resource_dir()
        .ok()
        .map(|d| d.join("companion").join(companion::FOLDER));
    let dev = cfg!(debug_assertions).then(|| {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../companion/dist")
            .join(companion::FOLDER)
    });
    [resource, dev]
        .into_iter()
        .flatten()
        .find(|d| d.join("manifest.json").is_file())
}

#[tauri::command]
pub fn companion_status(app: AppHandle) -> Status {
    app.state::<AppState>().companion.status()
}

/// Reports whether this build ships the companion mod at all.
#[tauri::command]
pub fn companion_available(app: AppHandle) -> bool {
    app.state::<AppState>().companion_dir.get().is_some()
}

#[tauri::command]
pub fn settings_set_companion(app: AppHandle, enabled: bool) -> AppResult<Settings> {
    app.state::<AppState>()
        .settings
        .update(|s| s.companion_disabled = !enabled)
}

/// Loads `values` into mod `unique_id` in the running game and, when that worked,
/// shows `notice` in the game's HUD.
#[tauri::command]
pub async fn companion_apply(
    app: AppHandle,
    unique_id: String,
    values: Value,
    notice: Option<String>,
) -> AppResult<Applied> {
    let companion = app.state::<AppState>().companion.clone();
    if !companion.has_mod(&unique_id) {
        return Ok(Applied {
            ok: false,
            method: None,
            error: Some("not_loaded".into()),
        });
    }
    let applied = companion.apply(&unique_id, &values).await?;
    if let (true, Some(text)) = (applied.ok, notice) {
        companion.notify(&text);
    }
    Ok(applied)
}
