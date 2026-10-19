//! Tauri commands for browsing Nexus, the account key, nxm links and downloads.

use tauri::{AppHandle, Emitter, Manager};

use crate::commands::AppState;
use crate::downloads::{self, Download, State};
use crate::error::{AppError, AppResult};
use crate::mods_commands;
use crate::nexus::{self, Account, LinkGrant, ModDetails, SearchFilters, SearchPage, Sort};

// the Keychain can stop to ask for the login password; that wait belongs on a blocking
// thread, not on an async worker or, for sync commands, the main thread
async fn keychain<T: Send + 'static>(
    f: impl FnOnce() -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| AppError::Keychain(e.to_string()))?
}

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
    let Some(key) = keychain(nexus::load_key).await? else {
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
    let stored = key.clone();
    keychain(move || nexus::save_key(&stored)).await?;
    *state.nexus_account.lock().unwrap() = Some(account.clone());
    Ok(account)
}

#[tauri::command]
pub async fn nexus_clear_key(app: AppHandle) -> AppResult<()> {
    keychain(nexus::clear_key).await?;
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

/// Returns the page where the user fetches this file without Premium; the nxm link it
/// sends back is then expected and installs without asking again.
#[tauri::command]
pub fn nexus_file_page(app: AppHandle, mod_id: u64, file_id: u64) -> String {
    expect_nxm(&app, mod_id, file_id);
    nexus::file_page(mod_id, file_id)
}

/// Starts the download of an `nxm://` link the user pasted or confirmed in the app.
#[tauri::command]
pub fn nxm_open(app: AppHandle, url: String) -> AppResult<()> {
    let link = nexus::parse_nxm(&url)?;
    start(app, link.mod_id, link.file_id, link.grant);
    Ok(())
}

/// How long after the app sent the user to a file page its nxm link counts as expected.
const EXPECT_FOR: std::time::Duration = std::time::Duration::from_secs(30 * 60);

pub(crate) fn expect_nxm(app: &AppHandle, mod_id: u64, file_id: u64) {
    let state = app.state::<AppState>();
    let mut expected = state.nxm_expected.lock().unwrap();
    expected.retain(|_, at| at.elapsed() < EXPECT_FOR);
    expected.insert((mod_id, file_id), std::time::Instant::now());
}

/// Handles an `nxm://` link the OS handed over. Any website can open one, and it ends in
/// a mod (code, for SMAPI mods) installed and enabled, so links the app didn't ask for go
/// to the UI for confirmation as `nxm:pending` instead of starting.
pub fn handle_nxm(app: &AppHandle, url: &str) -> AppResult<()> {
    let link = nexus::parse_nxm(url)?;
    let asked = {
        let state = app.state::<AppState>();
        let mut expected = state.nxm_expected.lock().unwrap();
        expected
            .remove(&(link.mod_id, link.file_id))
            .is_some_and(|at| at.elapsed() < EXPECT_FOR)
    };
    if asked {
        start(app.clone(), link.mod_id, link.file_id, link.grant);
    } else {
        let _ = app.emit(
            "nxm:pending",
            serde_json::json!({ "url": url, "modId": link.mod_id, "fileId": link.file_id }),
        );
    }
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
    let key = keychain(nexus::load_key)
        .await?
        .ok_or(AppError::NexusNoKey)?;
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
