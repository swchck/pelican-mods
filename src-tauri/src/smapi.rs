//! Installing and removing SMAPI through its official installer.
//!
//! The release zip is downloaded from GitHub and its macOS installer is run headless
//! (`--no-prompt`), so SMAPI does its own file shuffling: launcher swap, deps.json copy,
//! bundled mods. Reimplementing that here would drift with every SMAPI release.

use std::io::Read;
use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

use crate::error::{AppError, AppResult};
use crate::game;
use crate::rosetta;

const LATEST_RELEASE: &str = "https://api.github.com/repos/Pathoschild/SMAPI/releases/latest";

/// A downloadable SMAPI installer.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    pub version: String,
    pub url: String,
    pub size: u64,
    pub notes_url: String,
}

/// Progress of an install, emitted as the `smapi:progress` event.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "stage", rename_all = "camelCase")]
pub enum Progress {
    Download { received: u64, total: u64 },
    Extract,
    Install,
}

#[derive(Debug, Clone, Copy)]
pub enum Action {
    Install,
    Uninstall,
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    html_url: String,
    assets: Vec<GhAsset>,
}

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    size: u64,
    browser_download_url: String,
}

/// Fetches the newest SMAPI release from GitHub.
pub async fn latest(client: &reqwest::Client) -> AppResult<Release> {
    let rel: GhRelease = client
        .get(LATEST_RELEASE)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    pick_asset(rel)
}

fn pick_asset(rel: GhRelease) -> AppResult<Release> {
    // the "double-zipped" asset exists for people whose unzip tool mangles permissions
    let asset = rel
        .assets
        .into_iter()
        .find(|a| a.name.ends_with("-installer.zip") && !a.name.contains("double"))
        .ok_or_else(|| AppError::BadArchive(format!("no installer asset in {}", rel.tag_name)))?;
    Ok(Release {
        version: rel.tag_name.trim_start_matches('v').to_owned(),
        url: asset.browser_download_url,
        size: asset.size,
        notes_url: rel.html_url,
    })
}

/// Downloads `release` into `work_dir` and runs its installer against `game_dir`.
pub async fn run(
    client: &reqwest::Client,
    release: &Release,
    action: Action,
    game_dir: &Path,
    work_dir: &Path,
    progress: impl Fn(Progress),
) -> AppResult<String> {
    if work_dir.exists() {
        tokio::fs::remove_dir_all(work_dir).await?;
    }
    tokio::fs::create_dir_all(work_dir).await?;
    let archive = work_dir.join("installer.zip");
    download(client, &release.url, &archive, &progress).await?;

    progress(Progress::Extract);
    let out = work_dir.join("unpacked");
    let out2 = out.clone();
    tokio::task::spawn_blocking(move || extract(&archive, &out2))
        .await
        .map_err(|e| AppError::BadArchive(e.to_string()))??;
    let installer = find_installer(&out)?;

    progress(Progress::Install);
    let log = run_installer(&installer, action, game_dir).await?;
    let installed = game::has_smapi(game_dir);
    match (action, installed) {
        (Action::Install, true) | (Action::Uninstall, false) => {}
        _ => return Err(AppError::InstallerFailed(tail(&log, 15))),
    }
    let _ = tokio::fs::remove_dir_all(work_dir).await;
    Ok(log)
}

async fn download(
    client: &reqwest::Client,
    url: &str,
    to: &Path,
    progress: &impl Fn(Progress),
) -> AppResult<()> {
    let resp = client.get(url).send().await?.error_for_status()?;
    let total = resp.content_length().unwrap_or(0);
    let mut file = tokio::fs::File::create(to).await?;
    let mut stream = resp.bytes_stream();
    let mut received = 0u64;
    let mut last_report = 0u64;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
        received += chunk.len() as u64;
        // ~40 MB archive: one event per 256 KB keeps the IPC channel quiet
        if received - last_report >= 256 * 1024 || received == total {
            last_report = received;
            progress(Progress::Download { received, total });
        }
    }
    file.flush().await?;
    Ok(())
}

fn extract(archive: &Path, out: &Path) -> AppResult<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut zip = zip::ZipArchive::new(std::fs::File::open(archive)?)?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let Some(rel) = entry.enclosed_name() else {
            return Err(AppError::BadArchive(format!(
                "unsafe path {}",
                entry.name_raw().escape_ascii()
            )));
        };
        let dest = out.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&dest)?;
            continue;
        }
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut buf = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut buf)?;
        std::fs::write(&dest, buf)?;
        if let Some(mode) = entry.unix_mode() {
            std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(mode & 0o777))?;
        }
    }
    Ok(())
}

// the zip wraps everything in "SMAPI <version> installer/"
fn find_installer(root: &Path) -> AppResult<PathBuf> {
    for entry in std::fs::read_dir(root)?.flatten() {
        let candidate = entry.path().join("internal/macOS/SMAPI.Installer");
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(AppError::BadArchive(
        "internal/macOS/SMAPI.Installer not found".into(),
    ))
}

async fn run_installer(installer: &Path, action: Action, game_dir: &Path) -> AppResult<String> {
    use std::os::unix::fs::PermissionsExt;

    // zip tools that drop unix modes would leave the apphost non-executable
    std::fs::set_permissions(installer, std::fs::Permissions::from_mode(0o755))?;
    // same cwd as SMAPI's own "install on macOS.command", which cds into the bundle root
    let bundle_root = installer
        .ancestors()
        .nth(3)
        .ok_or_else(|| AppError::BadArchive("installer path too shallow".into()))?;
    let flag = match action {
        Action::Install => "--install",
        Action::Uninstall => "--uninstall",
    };
    let out = tokio::process::Command::new(installer)
        .args([flag, "--no-prompt", "--game-path"])
        .arg(game_dir)
        .current_dir(bundle_root)
        .stdin(std::process::Stdio::null())
        .output()
        .await
        .map_err(rosetta::classify)?;
    let mut log = String::from_utf8_lossy(&out.stdout).into_owned();
    log.push_str(&String::from_utf8_lossy(&out.stderr));
    if !out.status.success() {
        return Err(AppError::InstallerFailed(tail(&log, 15)));
    }
    Ok(log)
}

fn tail(log: &str, lines: usize) -> String {
    let all: Vec<&str> = log.lines().filter(|l| !l.trim().is_empty()).collect();
    all[all.len().saturating_sub(lines)..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn asset(name: &str) -> GhAsset {
        GhAsset {
            name: name.into(),
            size: 1,
            browser_download_url: format!("https://x/{name}"),
        }
    }

    #[test]
    fn picks_plain_installer_asset() {
        let rel = GhRelease {
            tag_name: "4.5.2".into(),
            html_url: "https://x".into(),
            assets: vec![
                asset("SMAPI-4.5.2-installer-double-zipped.zip"),
                asset("SMAPI-4.5.2-installer.zip"),
            ],
        };
        let r = pick_asset(rel).unwrap();
        assert_eq!(r.version, "4.5.2");
        assert!(r.url.ends_with("SMAPI-4.5.2-installer.zip"));
    }

    #[test]
    fn extracts_and_finds_installer_keeping_modes() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("a.zip");
        let mut w = zip::ZipWriter::new(std::fs::File::create(&archive).unwrap());
        let opts = zip::write::SimpleFileOptions::default().unix_permissions(0o755);
        w.start_file("SMAPI 4.5.2 installer/internal/macOS/SMAPI.Installer", opts)
            .unwrap();
        w.write_all(b"#!/bin/sh\n").unwrap();
        w.finish().unwrap();

        let out = dir.path().join("out");
        extract(&archive, &out).unwrap();
        let inst = find_installer(&out).unwrap();
        let mode = std::fs::metadata(&inst).unwrap().permissions().mode();
        assert_eq!(mode & 0o111, 0o111);
    }

    #[test]
    fn rejects_path_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("a.zip");
        let mut w = zip::ZipWriter::new(std::fs::File::create(&archive).unwrap());
        w.start_file("../evil", zip::write::SimpleFileOptions::default())
            .unwrap();
        w.write_all(b"x").unwrap();
        w.finish().unwrap();
        assert!(extract(&archive, &dir.path().join("out")).is_err());
    }

    // touches the network and a real game folder, so it only runs on request:
    // PELICAN_TEST_GAME_DIR=<copy of Contents/MacOS> cargo test -- --ignored
    #[test]
    #[ignore]
    fn installs_and_removes_smapi_in_real_game_copy() {
        let game_dir = PathBuf::from(std::env::var("PELICAN_TEST_GAME_DIR").unwrap());
        let work = tempfile::tempdir().unwrap();
        let client = reqwest::Client::builder()
            .user_agent("PelicanMods-test")
            .build()
            .unwrap();
        tauri::async_runtime::block_on(async {
            let rel = latest(&client).await.unwrap();
            run(
                &client,
                &rel,
                Action::Install,
                &game_dir,
                &work.path().join("w"),
                |_| {},
            )
            .await
            .unwrap();
            let g = game::inspect(&game_dir).unwrap();
            assert_eq!(g.smapi_version.as_deref(), Some(rel.version.as_str()));
            assert!(g.mods_path.join("ConsoleCommands/manifest.json").is_file());
            run(
                &client,
                &rel,
                Action::Uninstall,
                &game_dir,
                &work.path().join("w"),
                |_| {},
            )
            .await
            .unwrap();
            assert!(!game::has_smapi(&game_dir));
        });
    }
}
