//! ModDrop: the file list SMAPI's update check reads, and anonymous download links.
//!
//! Both endpoints are the ones moddrop.com's own pages call; neither needs an account.

use serde::Deserialize;
use serde_json::json;

use crate::error::{AppError, AppResult};

const API: &str = "https://www.moddrop.com/api/mods/data";
const DOWNLOAD: &str = "https://www.moddrop.com/rest/resource/mod/download";

/// A file on a ModDrop mod page.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct File {
    pub id: u64,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub is_default: bool,
    #[serde(default)]
    pub is_hidden: bool,
    #[serde(default)]
    pub is_deleted: bool,
    #[serde(default)]
    pub is_alternative: bool,
}

/// Extracts the mod id from `moddrop.com/stardew-valley/mods/<id>[-slug]` or `/mod/<id>`.
pub fn mod_page_id(url: &str) -> Option<u64> {
    let url = url::Url::parse(url).ok()?;
    if !url.host_str()?.ends_with("moddrop.com") {
        return None;
    }
    let parts: Vec<&str> = url.path_segments()?.collect();
    let [_, "mods" | "mod", id, ..] = parts.as_slice() else {
        return None;
    };
    id.split('-').next()?.parse().ok()
}

/// Lists the files of mod `mod_id`.
pub async fn files(client: &reqwest::Client, mod_id: u64) -> AppResult<Vec<File>> {
    #[derive(Deserialize)]
    struct Resp {
        mods: std::collections::HashMap<String, Entry>,
    }
    #[derive(Deserialize)]
    struct Entry {
        #[serde(default)]
        files: Vec<File>,
    }
    let resp: Resp = client
        .post(API)
        .json(&json!({ "ModIDs": [mod_id], "Files": true, "Mods": false }))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(resp
        .mods
        .into_values()
        .next()
        .map(|e| e.files)
        .unwrap_or_default())
}

/// Picks the visible file carrying `version`, preferring the page's default download
/// over alternatives.
pub fn pick<'a>(files: &'a [File], version: &str) -> Option<&'a File> {
    files
        .iter()
        .filter(|f| !f.is_hidden && !f.is_deleted)
        .filter(|f| {
            f.version
                .as_deref()
                .is_some_and(|v| crate::version::compare(v, version).is_eq())
        })
        .min_by_key(|f| (!f.is_default, f.is_alternative, std::cmp::Reverse(f.id)))
}

/// Returns a short-lived signed URL for file `file_id`.
pub async fn download_url(client: &reqwest::Client, file_id: u64) -> AppResult<String> {
    #[derive(Deserialize)]
    struct Resp {
        url: Option<String>,
    }
    let resp: Resp = client
        .post(DOWNLOAD)
        .json(&json!({ "id": file_id }))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    resp.url
        .filter(|u| u.starts_with("https://"))
        .ok_or_else(|| AppError::NoDownload(format!("moddrop {file_id}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(id: u64, version: &str, default: bool, hidden: bool) -> File {
        File {
            id,
            version: Some(version.into()),
            is_default: default,
            is_hidden: hidden,
            is_deleted: false,
            is_alternative: false,
        }
    }

    #[test]
    fn reads_page_ids() {
        assert_eq!(
            mod_page_id("https://www.moddrop.com/stardew-valley/mods/600967-adopt-n-skin"),
            Some(600967)
        );
        assert_eq!(
            mod_page_id("https://www.moddrop.com/stardew-valley/mod/600967"),
            Some(600967)
        );
        assert_eq!(
            mod_page_id("https://www.nexusmods.com/stardewvalley/mods/1"),
            None
        );
    }

    #[test]
    fn picks_default_visible_file() {
        let files = [
            file(1, "3.2.1", false, true),
            file(2, "3.2.1", false, false),
            file(3, "3.2.1", true, false),
            file(4, "3.2.0", true, false),
        ];
        assert_eq!(pick(&files, "3.2.1").unwrap().id, 3);
        assert_eq!(pick(&files[..2], "3.2.1").unwrap().id, 2);
        assert!(pick(&files, "9.0.0").is_none());
    }

    // network: moddrop.com
    #[test]
    #[ignore]
    fn resolves_download_live() {
        let client = reqwest::Client::builder()
            .user_agent("PelicanMods-test")
            .build()
            .unwrap();
        tauri::async_runtime::block_on(async {
            let files = files(&client, 600967).await.unwrap();
            let latest = files.iter().find(|f| f.is_default).expect("a default file");
            let picked = pick(&files, latest.version.as_deref().unwrap()).unwrap();
            assert_eq!(picked.id, latest.id);
            let url = download_url(&client, picked.id).await.unwrap();
            assert!(url.contains(".zip"), "{url}");
        });
    }
}
