//! Commands for mod list files: export a profile, read one, make a profile from it.

use std::path::PathBuf;

use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Manager};

use super::library::{LibraryView, build_view};
use super::{AppState, blocking, game_data_dir};
use crate::error::AppResult;
use crate::game;
use crate::library;

fn active_list(s: &AppState) -> crate::modlist::ModList {
    let profiles = s.profiles.get();
    let active = profiles.active();
    let install = s.game_dir().ok().and_then(|d| game::inspect(&d).ok());
    crate::modlist::build(
        active.name.as_deref().unwrap_or("Pelican Mods"),
        &library::scan(&s.store_root),
        &active.enabled,
        &s.settings.get().github_repos,
        install.as_ref().and_then(|g| g.version.clone()),
        install.and_then(|g| g.smapi_version),
    )
}

/// Writes the active profile's mods to `dest` as a mod list.
#[tauri::command]
pub async fn modlist_export(app: AppHandle, dest: PathBuf) -> AppResult<()> {
    blocking(app, move |s| {
        std::fs::write(
            &dest,
            serde_json::to_vec_pretty(&active_list(s)).expect("list serialize"),
        )?;
        Ok(())
    })
    .await
}

/// Reads the mod list in `file` and says which of its mods the library has.
#[tauri::command]
pub async fn modlist_read(app: AppHandle, file: PathBuf) -> AppResult<crate::modlist::ModListView> {
    blocking(app, move |s| {
        let list = crate::modlist::read(&file)?;
        Ok(crate::modlist::status(
            list,
            &library::scan(&s.store_root),
            &s.profiles.get().active().enabled,
        ))
    })
    .await
}

/// Creates profile `name` with just the packages holding `unique_ids` and switches to it.
#[tauri::command]
pub async fn modlist_profile(
    app: AppHandle,
    name: String,
    unique_ids: Vec<String>,
) -> AppResult<LibraryView> {
    blocking(app, move |s| {
        let packages = crate::modlist::packages_for(&library::scan(&s.store_root), &unique_ids);
        s.profiles.update(|p| {
            let id = p.create(&name, None)?;
            p.activate(&id)?;
            for package in &packages {
                p.set_enabled(package, true);
            }
            Ok(())
        })?;
        Ok(build_view(s))
    })
    .await
}

/// Starts handing the active profile's list to partners on the local network who look
/// for it; the list is built anew for each, so it follows changes made meanwhile.
#[tauri::command]
pub async fn lan_share_start(app: AppHandle) -> AppResult<()> {
    let farmer = game_data_dir(&app)
        .ok()
        .and_then(|d| crate::saves::list(&d.join("Saves")).into_iter().next())
        .map(|s| s.farmer);
    let state = app.state::<AppState>();
    let profile = state
        .profiles
        .get()
        .active()
        .name
        .clone()
        .unwrap_or_else(|| "Pelican Mods".to_owned());
    let name = match farmer {
        Some(f) if !f.is_empty() => format!("{f} · {profile}"),
        _ => profile,
    };
    let handle = app.clone();
    let list: crate::lan::ListSource = Arc::new(move || {
        let s = handle.state::<AppState>();
        Ok(serde_json::to_vec(&active_list(&s)).expect("list serialize"))
    });
    let share = crate::lan::Share::start(crate::lan::PORT, name, list).await?;
    *state.lan_share.lock().expect("lan lock") = Some(share);
    Ok(())
}

/// Stops sharing the list.
#[tauri::command]
pub fn lan_share_stop(app: AppHandle) {
    app.state::<AppState>()
        .lan_share
        .lock()
        .expect("lan lock")
        .take();
}

/// Hosts on the local network sharing a list, after a second and a half of listening.
#[tauri::command]
pub async fn lan_discover() -> AppResult<Vec<crate::lan::Host>> {
    let everyone = (std::net::Ipv4Addr::BROADCAST, crate::lan::PORT).into();
    crate::lan::discover(everyone, Duration::from_millis(1500)).await
}

#[derive(Serialize)]
pub struct FetchedList {
    /// Where the list was kept, for reading it again after installs.
    file: PathBuf,
    view: crate::modlist::ModListView,
}

/// Fetches the list host `addr`:`port` shares and sets it against the library.
#[tauri::command]
pub async fn lan_fetch(app: AppHandle, addr: String, port: u16) -> AppResult<FetchedList> {
    let ip: std::net::IpAddr = addr
        .parse()
        .map_err(|_| crate::error::AppError::NotFound(addr.clone()))?;
    let list = crate::lan::fetch((ip, port).into()).await?;
    blocking(app, move |s| {
        let dir = s.work_dir.with_file_name("lan");
        std::fs::create_dir_all(&dir)?;
        let file = dir.join(format!(
            "{}-{port}.json",
            ip.to_string().replace([':', '.'], "_")
        ));
        std::fs::write(&file, serde_json::to_vec(&list).expect("list serialize"))?;
        let view = crate::modlist::status(
            list,
            &library::scan(&s.store_root),
            &s.profiles.get().active().enabled,
        );
        Ok(FetchedList { file, view })
    })
    .await
}
