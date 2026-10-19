//! Tauri commands for browsing Nexus, the account key, nxm links and downloads.

use tauri::{AppHandle, Emitter, Manager};

use crate::commands::AppState;
use crate::downloads::{self, Download, State};
use crate::error::{AppError, AppResult};
use crate::mods_commands;
use crate::nexus::{self, Account, LinkGrant, ModDetails, SearchFilters, SearchPage, Sort};

#[tauri::command]
pub async fn nexus_search(
    app: AppHandle,
    query: String,
    filters: SearchFilters,
    sort: Sort,
    offset: u32,
) -> AppResult<SearchPage> {
    let state = app.state::<AppState>();
    nexus::search(&state.http, &query, &filters, sort, offset, 20).await
}

#[tauri::command]
pub async fn nexus_mod(app: AppHandle, mod_id: u64) -> AppResult<ModDetails> {
    let state = app.state::<AppState>();
    nexus::details(&state.http, mod_id).await
}

/// Returns the account of the stored key, validating it once per session.
#[tauri::command]
pub async fn nexus_account(app: AppHandle) -> AppResult<Option<Account>> {
    let state = app.state::<AppState>();
    if let Some(a) = state.nexus_account.lock().unwrap().clone() {
        return Ok(Some(a));
    }
    let Some(key) = nexus::load_key()? else {
        return Ok(None);
    };
    let account = nexus::validate(&state.http, &key).await?;
    *state.nexus_account.lock().unwrap() = Some(account.clone());
    Ok(Some(account))
}

/// Validates `key` with Nexus and stores it in the Keychain.
#[tauri::command]
pub async fn nexus_set_key(app: AppHandle, key: String) -> AppResult<Account> {
    let state = app.state::<AppState>();
    let key = key.trim().to_owned();
    let account = nexus::validate(&state.http, &key).await?;
    nexus::save_key(&key)?;
    *state.nexus_account.lock().unwrap() = Some(account.clone());
    Ok(account)
}

#[tauri::command]
pub fn nexus_clear_key(app: AppHandle) -> AppResult<()> {
    nexus::clear_key()?;
    *app.state::<AppState>().nexus_account.lock().unwrap() = None;
    Ok(())
}

/// Starts a direct download, which Nexus allows only for Premium accounts; others get
/// [`AppError::NexusNeedsPremium`] and go through the site's "Mod Manager Download".
#[tauri::command]
pub async fn nexus_download(app: AppHandle, mod_id: u64, file_id: u64) -> AppResult<()> {
    let premium = nexus_account(app.clone())
        .await?
        .ok_or(AppError::NexusNoKey)?
        .is_premium;
    if !premium {
        return Err(AppError::NexusNeedsPremium);
    }
    start(app, mod_id, file_id, None);
    Ok(())
}

#[tauri::command]
pub fn nexus_file_page(mod_id: u64, file_id: u64) -> String {
    nexus::file_page(mod_id, file_id)
}

/// Handles an `nxm://` link, from the OS or pasted by the user.
#[tauri::command]
pub fn nxm_open(app: AppHandle, url: String) -> AppResult<()> {
    handle_nxm(&app, &url)
}

/// Starts the download an nxm link describes.
pub fn handle_nxm(app: &AppHandle, url: &str) -> AppResult<()> {
    let link = nexus::parse_nxm(url)?;
    start(app.clone(), link.mod_id, link.file_id, link.grant);
    Ok(())
}

#[tauri::command]
pub fn downloads_list(app: AppHandle) -> Vec<Download> {
    app.state::<AppState>().downloads.list()
}

#[tauri::command]
pub fn downloads_clear(app: AppHandle) -> Vec<Download> {
    let state = app.state::<AppState>();
    state.downloads.clear_finished();
    let list = state.downloads.list();
    let _ = app.emit("downloads:changed", &list);
    list
}

fn changed(app: &AppHandle) {
    let _ = app.emit(
        "downloads:changed",
        app.state::<AppState>().downloads.list(),
    );
}

/// Queues a Nexus file for download and install; progress goes out as `downloads:changed`.
pub(crate) fn start(app: AppHandle, mod_id: u64, file_id: u64, grant: Option<LinkGrant>) {
    let id = app
        .state::<AppState>()
        .downloads
        .add(format!("Nexus {mod_id}/{file_id}"), Some(mod_id));
    changed(&app);
    tauri::async_runtime::spawn(async move {
        let result = run(&app, id, mod_id, file_id, grant).await;
        let state = app.state::<AppState>();
        match result {
            Ok(()) => {
                state.downloads.set(id, State::Done);
                let _ = app.emit("library:changed", ());
            }
            Err(e) => state.downloads.set(
                id,
                State::Failed {
                    error: serde_json::to_value(&e).unwrap_or_default(),
                },
            ),
        }
        changed(&app);
    });
}

async fn run(
    app: &AppHandle,
    id: u64,
    mod_id: u64,
    file_id: u64,
    grant: Option<LinkGrant>,
) -> AppResult<()> {
    let state = app.state::<AppState>();
    let key = nexus::load_key()?.ok_or(AppError::NexusNoKey)?;
    let url = nexus::download_url(&state.http, &key, mod_id, file_id, grant.as_ref()).await?;
    let dir = state.work_dir.with_file_name("downloads");
    let file = {
        let app2 = app.clone();
        downloads::fetch(&state.http, &url, &dir, move |received, total| {
            app2.state::<AppState>()
                .downloads
                .set(id, State::Downloading { received, total });
            changed(&app2);
        })
        .await?
    };
    if let Some(name) = file.file_name() {
        state
            .downloads
            .rename(id, name.to_string_lossy().into_owned());
    }
    state.downloads.set(id, State::Installing);
    changed(app);
    let installed = mods_commands::blocking(app.clone(), {
        let file = file.clone();
        move |s| mods_commands::install_paths(s, std::slice::from_ref(&file))
    })
    .await;
    let _ = tokio::fs::remove_file(&file).await;
    installed
}
