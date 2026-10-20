//! SMAPI's compatibility list, looked up by Nexus mod id for the mod catalog.
//!
//! smapi.io answers by UniqueID, which a Nexus search result doesn't carry; the list it
//! draws its verdicts from does carry Nexus ids, so the catalog reads that list itself.

use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

const LIST: &str =
    "https://raw.githubusercontent.com/Pathoschild/SmapiCompatibilityList/develop/data/mods.jsonc";
/// The list changes a few times a week; a day old is fresh enough for warnings.
const MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// What the list says about a mod that isn't simply fine.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Verdict {
    /// smapi.io's words: "Broken", "Abandoned", "Obsolete", "Workaround" or "Unofficial".
    pub status: String,
    pub summary: Option<String>,
}

#[derive(Deserialize)]
struct List {
    mods: Vec<Entry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    #[serde(default)]
    nexus: Option<u64>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    broke_in: Option<String>,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    unofficial_update: Option<serde_json::Value>,
}

/// Reads verdicts by Nexus id from the list's text.
pub fn parse(text: &str) -> AppResult<HashMap<u64, Verdict>> {
    let list: List =
        crate::jsonc::parse(text).map_err(|e| AppError::Io(std::io::Error::other(e)))?;
    Ok(list
        .mods
        .into_iter()
        .filter_map(|e| {
            let id = e.nexus?;
            // as smapi.io reads an entry: an explicit status wins, then a version it broke in
            let status = match (
                e.status.as_deref(),
                e.broke_in.is_some(),
                e.unofficial_update.is_some(),
            ) {
                (Some(s), _, _) => capitalize(s),
                (None, true, true) => "Unofficial".to_owned(),
                (None, true, false) => "Broken".to_owned(),
                (None, false, _) => return None,
            };
            if status == "Ok" || status == "Optional" {
                return None;
            }
            Some((
                id,
                Verdict {
                    status,
                    summary: e.summary,
                },
            ))
        })
        .collect())
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

/// The list's verdicts, from `cache` when it's fresh, else downloaded and cached.
pub async fn verdicts(client: &reqwest::Client, cache: &Path) -> AppResult<HashMap<u64, Verdict>> {
    let fresh = std::fs::metadata(cache)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .is_some_and(|age| age < MAX_AGE);
    if let Some(text) = fresh.then(|| std::fs::read_to_string(cache).ok()).flatten() {
        return parse(&text);
    }
    let text = client
        .get(LIST)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    if let Some(dir) = cache.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(cache, &text)?;
    parse(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_verdicts_like_smapi_io() {
        let text = r#"{ "$schema": "schema.json", "mods": [
            { "name": "Fine", "id": "a.Fine", "nexus": 1 },
            { "name": "Old", "id": "a.Old", "nexus": 2, "brokeIn": "SMAPI 3.0", "summary": "use X instead.", },
            { "name": "Patched", "id": "a.P", "nexus": 3, "brokeIn": "SMAPI 4.0",
              "unofficialUpdate": { "version": "1.0.1-unofficial", "url": "https://x" } },
            { "name": "Gone", "id": "a.G", "nexus": 4, "status": "abandoned" }, // comment
            { "name": "No page", "id": "a.N", "status": "broken" },
        ] }"#;
        let v = parse(text).unwrap();
        assert_eq!(v.len(), 3);
        assert_eq!(v[&2].status, "Broken");
        assert_eq!(v[&2].summary.as_deref(), Some("use X instead."));
        assert_eq!(v[&3].status, "Unofficial");
        assert_eq!(v[&4].status, "Abandoned");
    }

    // PELICAN_TEST_COMPAT=<mods.jsonc> cargo test -- --ignored real_compat_list --nocapture
    #[test]
    #[ignore]
    fn real_compat_list() {
        let text = std::fs::read_to_string(std::env::var("PELICAN_TEST_COMPAT").unwrap()).unwrap();
        let v = parse(&text).unwrap();
        let mut by: HashMap<&str, usize> = HashMap::new();
        for x in v.values() {
            *by.entry(x.status.as_str()).or_default() += 1;
        }
        eprintln!(
            "{} verdicts {:?}; SpaceCore {:?}",
            v.len(),
            by,
            v.get(&1348)
        );
    }
}
