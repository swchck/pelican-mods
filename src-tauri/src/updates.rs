//! Mod update checks through the SMAPI web API (smapi.io).
//!
//! smapi.io resolves each mod's update keys against Nexus, GitHub, ModDrop and
//! CurseForge and adds the compatibility list's verdict, which is what SMAPI's own
//! console check uses. One request covers the whole library and needs no API keys.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::library::Package;

const API: &str = "https://smapi.io/api/v4.0.0/mods";

/// Results older than this are refreshed on the next non-forced check.
pub const MAX_AGE: Duration = Duration::from_secs(6 * 60 * 60);

/// What smapi.io knows about one installed mod.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    /// Newer version to install, if any.
    pub version: Option<String>,
    /// Page or release list for that version.
    pub url: Option<String>,
    /// Compatibility verdict ("Ok", "Optional", "Workaround", "Broken", "Obsolete", …).
    pub compat_status: Option<String>,
    pub compat_summary: Option<String>,
    pub nexus_id: Option<u64>,
    pub errors: Vec<String>,
}

/// A finished check, keyed by lowercased UniqueID.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateReport {
    /// Unix seconds; 0 means never checked.
    pub checked_at: u64,
    pub mods: HashMap<String, UpdateInfo>,
}

impl UpdateReport {
    /// Reports whether this report is recent enough to reuse.
    pub fn fresh(&self) -> bool {
        let now = now_secs();
        self.checked_at > 0 && now.saturating_sub(self.checked_at) < MAX_AGE.as_secs()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Request<'a> {
    mods: Vec<RequestMod<'a>>,
    api_version: &'a str,
    game_version: &'a str,
    platform: &'static str,
    include_extended_metadata: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RequestMod<'a> {
    id: &'a str,
    update_keys: Vec<String>,
    installed_version: &'a str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResponseMod {
    id: String,
    suggested_update: Option<Suggested>,
    metadata: Option<Metadata>,
    #[serde(default)]
    errors: Vec<String>,
}

#[derive(Deserialize)]
struct Suggested {
    version: String,
    url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Metadata {
    #[serde(default)]
    name: Option<String>,
    #[serde(rename = "nexusID")]
    nexus_id: Option<u64>,
    #[serde(default)]
    git_hub_repo: Option<String>,
    /// The newest release on the mod's main page.
    #[serde(default)]
    main: Option<Suggested>,
    compatibility_status: Option<String>,
    compatibility_summary: Option<String>,
}

/// Asks smapi.io about every mod in `packages`; `repos` adds user-attached GitHub
/// repos (by lowercased UniqueID) to the mods' own update keys.
pub async fn check(
    client: &reqwest::Client,
    packages: &[Package],
    repos: &std::collections::BTreeMap<String, String>,
    smapi_version: &str,
    game_version: &str,
) -> AppResult<UpdateReport> {
    let mods: Vec<RequestMod> = packages
        .iter()
        .flat_map(|p| &p.mods)
        .map(|m| {
            let mut update_keys = m.manifest.update_keys.clone();
            if let Some(repo) = repos.get(&m.manifest.unique_id.to_lowercase()) {
                update_keys.push(format!("GitHub:{repo}"));
            }
            RequestMod {
                id: &m.manifest.unique_id,
                update_keys,
                installed_version: &m.manifest.version,
            }
        })
        .collect();
    if mods.is_empty() {
        return Ok(UpdateReport {
            checked_at: now_secs(),
            mods: HashMap::new(),
        });
    }
    let body = Request {
        mods,
        api_version: smapi_version,
        game_version,
        platform: "Mac",
        include_extended_metadata: true,
    };
    let resp: Vec<ResponseMod> = client
        .post(API)
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(UpdateReport {
        checked_at: now_secs(),
        mods: resp.into_iter().map(convert).collect(),
    })
}

/// Where to get a mod the player doesn't have, as smapi.io's mod list knows it.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub name: String,
    pub version: String,
    pub url: String,
    pub nexus_id: Option<u64>,
    pub github: Option<String>,
}

/// Looks up mod `unique_id` on smapi.io; `None` when its list doesn't know the mod.
pub async fn lookup(
    client: &reqwest::Client,
    unique_id: &str,
    smapi_version: &str,
    game_version: &str,
) -> AppResult<Option<Found>> {
    let body = Request {
        mods: vec![RequestMod {
            id: unique_id,
            update_keys: Vec::new(),
            // smapi.io answers "0.0.0" with an empty list
            installed_version: "0.0.1",
        }],
        api_version: smapi_version,
        game_version,
        platform: "Mac",
        include_extended_metadata: true,
    };
    let resp: Vec<ResponseMod> = client
        .post(API)
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(resp.into_iter().next().and_then(found))
}

fn found(r: ResponseMod) -> Option<Found> {
    let meta = r.metadata?;
    let main = meta.main?;
    Some(Found {
        name: meta.name.unwrap_or_else(|| r.id.clone()),
        version: main.version,
        url: main.url,
        nexus_id: meta.nexus_id,
        github: meta.git_hub_repo.as_deref().and_then(parse_repo),
    })
}

fn convert(r: ResponseMod) -> (String, UpdateInfo) {
    let meta = r.metadata;
    let info = UpdateInfo {
        version: r.suggested_update.as_ref().map(|s| s.version.clone()),
        url: r.suggested_update.map(|s| s.url),
        compat_status: meta.as_ref().and_then(|m| m.compatibility_status.clone()),
        compat_summary: meta.as_ref().and_then(|m| m.compatibility_summary.clone()),
        nexus_id: meta.and_then(|m| m.nexus_id),
        errors: r.errors,
    };
    (r.id.to_lowercase(), info)
}

/// Reads the cached report, or an empty one.
pub fn load_cache(file: &Path) -> UpdateReport {
    std::fs::read_to_string(file)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_cache(file: &PathBuf, report: &UpdateReport) -> std::io::Result<()> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(file, serde_json::to_vec(report).expect("report serialize"))
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    assets: Vec<GhAsset>,
}

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

/// Downloads the release zip for `mod_name` `version` from GitHub `repo` into `dir`.
pub async fn download_github(
    client: &reqwest::Client,
    repo: &str,
    mod_name: &str,
    version: &str,
    dir: &Path,
) -> AppResult<PathBuf> {
    let url = format!("https://api.github.com/repos/{repo}/releases?per_page=20");
    let releases: Vec<GhRelease> = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let asset = pick_asset(&releases, mod_name, version)
        .ok_or_else(|| AppError::NoDownload(format!("{repo} {version}")))?;
    let bytes = client
        .get(&asset.browser_download_url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    tokio::fs::create_dir_all(dir).await?;
    let file = dir.join(crate::downloads::safe_file_name(&asset.name));
    tokio::fs::write(&file, bytes).await?;
    Ok(file)
}

// monorepos tag releases per mod ("GenericModConfigMenu/1.16.0") and attach one zip each,
// so match the version in the tag first and the mod name in the asset second
fn pick_asset<'a>(releases: &'a [GhRelease], mod_name: &str, version: &str) -> Option<&'a GhAsset> {
    let want = normalize(mod_name);
    // on a boundary, so version 1.5 doesn't take tag 2.1.5
    let release = releases.iter().find(|r| {
        r.tag_name.strip_suffix(version).is_some_and(|head| {
            head.is_empty() || head.ends_with(['/', 'v', 'V', '-', '_', ' ', '@'])
        })
    })?;
    let zips: Vec<&GhAsset> = release
        .assets
        .iter()
        .filter(|a| a.name.to_lowercase().ends_with(".zip"))
        .collect();
    match zips.as_slice() {
        [only] => Some(*only),
        many => many
            .iter()
            .find(|a| normalize(&a.name).contains(&want))
            .copied(),
    }
}

fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Returns `owner/repo` from a mod's "GitHub:owner/repo" update key, if it has one.
pub fn github_key(keys: &[String]) -> Option<String> {
    keys.iter().find_map(|k| {
        let (site, rest) = k.split_once(':')?;
        if !site.trim().eq_ignore_ascii_case("github") {
            return None;
        }
        // "@Subkey" picks one mod's files in a shared repo; the repo is what comes before
        let repo = rest.split('@').next()?.trim();
        let (owner, name) = repo.split_once('/')?;
        (!owner.is_empty() && !name.is_empty() && !name.contains('/')).then(|| repo.to_owned())
    })
}

/// Reads what a user typed for a mod's repo: "owner/repo" or a github.com link.
pub fn parse_repo(input: &str) -> Option<String> {
    let s = input.trim().trim_end_matches('/').trim_end_matches(".git");
    let s = s
        .strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))
        .unwrap_or(s);
    let s = s.strip_prefix("www.").unwrap_or(s);
    let s = s.strip_prefix("github.com/").unwrap_or(s);
    let mut parts = s.split('/');
    let (owner, repo) = (parts.next()?, parts.next()?);
    let ok = |p: &str, extra: &str| {
        !p.is_empty()
            && p.chars()
                .all(|c| c.is_ascii_alphanumeric() || extra.contains(c))
    };
    // GitHub user names have no dots, which also turns away "nexusmods.com/…"; a deeper
    // path ("…/releases") is fine
    (ok(owner, "-") && ok(repo, "-_.")).then(|| format!("{owner}/{repo}"))
}

/// Extracts `owner/repo` from a GitHub URL smapi.io suggested.
pub fn github_repo(url: &str) -> Option<String> {
    let rest = url.strip_prefix("https://github.com/")?;
    let mut parts = rest.split('/');
    let owner = parts.next().filter(|s| !s.is_empty())?;
    let repo = parts.next().filter(|s| !s.is_empty())?;
    Some(format!("{owner}/{repo}"))
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_response() {
        let raw = r#"[{
            "id": "Pathoschild.LookupAnything",
            "suggestedUpdate": {"version": "1.55.0", "url": "https://www.nexusmods.com/stardewvalley/mods/541"},
            "metadata": {"nexusID": 541, "compatibilityStatus": "Ok", "compatibilitySummary": "use latest version."},
            "errors": []
        }, {"id": "x.y", "errors": ["Found no GitHub release for this ID."]}]"#;
        let resp: Vec<ResponseMod> = serde_json::from_str(raw).unwrap();
        let map: HashMap<String, UpdateInfo> = resp.into_iter().map(convert).collect();
        let la = &map["pathoschild.lookupanything"];
        assert_eq!(la.version.as_deref(), Some("1.55.0"));
        assert_eq!(la.nexus_id, Some(541));
        assert_eq!(la.compat_status.as_deref(), Some("Ok"));
        assert!(map["x.y"].version.is_none());
        assert_eq!(map["x.y"].errors.len(), 1);
    }

    fn release(tag: &str, assets: &[&str]) -> GhRelease {
        GhRelease {
            tag_name: tag.into(),
            assets: assets
                .iter()
                .map(|n| GhAsset {
                    name: (*n).into(),
                    browser_download_url: format!("https://x/{n}"),
                })
                .collect(),
        }
    }

    #[test]
    fn picks_release_asset() {
        let rels = [
            release(
                "GenericModConfigMenu/1.16.0",
                &["GenericModConfigMenu 1.16.0.zip"],
            ),
            release(
                "v2.1.0",
                &["Other.zip", "Generic Mod Config Menu.zip", "notes.txt"],
            ),
            release("3.0.0", &["a.zip", "b.zip"]),
        ];
        assert_eq!(
            pick_asset(&rels, "Generic Mod Config Menu", "1.16.0")
                .unwrap()
                .name,
            "GenericModConfigMenu 1.16.0.zip"
        );
        assert_eq!(
            pick_asset(&rels, "Generic Mod Config Menu", "2.1.0")
                .unwrap()
                .name,
            "Generic Mod Config Menu.zip"
        );
        assert!(pick_asset(&rels, "Nope", "3.0.0").is_none());
        assert!(pick_asset(&rels, "x", "9.9.9").is_none());
        assert!(
            pick_asset(&rels, "x", "1.0").is_none(),
            "1.0 must not match 2.1.0"
        );
    }

    #[test]
    fn reads_github_update_keys() {
        let keys = |k: &[&str]| k.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            github_key(&keys(&["Nexus:1915", "GitHub:Annosz/UIInfoSuite2"])).as_deref(),
            Some("Annosz/UIInfoSuite2")
        );
        assert_eq!(
            github_key(&keys(&[
                "github: spacechase0/StardewValleyMods@GenericModConfigMenu"
            ]))
            .as_deref(),
            Some("spacechase0/StardewValleyMods")
        );
        assert_eq!(github_key(&keys(&["Nexus:1915", "GitHub:broken"])), None);
    }

    // network: smapi.io finds a mod by UniqueID alone, and its GitHub release downloads
    #[test]
    #[ignore]
    fn finds_dependencies_online() {
        tauri::async_runtime::block_on(async {
            let client = reqwest::Client::builder()
                .user_agent("pelican-mods-test")
                .build()
                .unwrap();
            let f = lookup(&client, "Annosz.UiInfoSuite2", "4.5.2", "1.6.15")
                .await
                .unwrap()
                .expect("smapi.io knows UI Info Suite 2");
            eprintln!("{f:?}");
            let dir = tempfile::tempdir().unwrap();
            let repo = f.github.expect("a GitHub repo");
            let file = download_github(&client, &repo, &f.name, &f.version, dir.path())
                .await
                .unwrap();
            eprintln!("downloaded {}", file.display());
            assert!(file.is_file());
        });
    }

    #[test]
    fn reads_lookups() {
        let body = r#"[{"id":"spacechase0.SpaceCore","metadata":{"id":["spacechase0.SpaceCore"],
            "name":"SpaceCore","nexusID":1348,"gitHubRepo":"spacechase0/StardewValleyMods",
            "main":{"version":"1.28.0","url":"https://www.nexusmods.com/stardewvalley/mods/1348"}},
            "errors":[]}]"#;
        let resp: Vec<ResponseMod> = serde_json::from_str(body).unwrap();
        let f = found(resp.into_iter().next().unwrap()).unwrap();
        assert_eq!(f.name, "SpaceCore");
        assert_eq!(f.version, "1.28.0");
        assert_eq!(f.nexus_id, Some(1348));
        assert_eq!(f.github.as_deref(), Some("spacechase0/StardewValleyMods"));
    }

    #[test]
    fn parses_typed_repos() {
        for typed in [
            "Annosz/UIInfoSuite2",
            "https://github.com/Annosz/UIInfoSuite2",
            " github.com/Annosz/UIInfoSuite2/releases ",
            "https://github.com/Annosz/UIInfoSuite2.git",
        ] {
            assert_eq!(
                parse_repo(typed).as_deref(),
                Some("Annosz/UIInfoSuite2"),
                "{typed}"
            );
        }
        assert_eq!(parse_repo("UIInfoSuite2"), None);
        assert_eq!(parse_repo("https://nexusmods.com/x"), None);
        assert_eq!(parse_repo("owner/re po"), None);
    }

    #[test]
    fn parses_github_repo() {
        assert_eq!(
            github_repo("https://github.com/owner/Repo/releases").as_deref(),
            Some("owner/Repo")
        );
        assert_eq!(
            github_repo("https://www.nexusmods.com/stardewvalley/mods/1"),
            None
        );
        assert_eq!(github_repo("https://github.com/owner"), None);
    }

    #[test]
    fn caches_and_ages() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("u/updates.json");
        assert!(!load_cache(&file).fresh());
        let report = UpdateReport {
            checked_at: now_secs(),
            mods: HashMap::new(),
        };
        save_cache(&file, &report).unwrap();
        assert!(load_cache(&file).fresh());
        let old = UpdateReport {
            checked_at: now_secs() - MAX_AGE.as_secs() - 1,
            mods: HashMap::new(),
        };
        assert!(!old.fresh());
    }

    // network: smapi.io plus a real GitHub release download and install over an older copy
    // cargo test -- --ignored github_update_end_to_end
    #[test]
    #[ignore]
    fn github_update_end_to_end() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("store");
        let old = store.join("FarmTypeManager");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(
            old.join("manifest.json"),
            r#"{"Name":"Farm Type Manager","UniqueID":"Esca.FarmTypeManager","Version":"1.20.0",
                "EntryDll":"FarmTypeManager.dll","UpdateKeys":["GitHub:Esca-MMC/FarmTypeManager"]}"#,
        )
        .unwrap();
        std::fs::write(old.join("config.json"), r#"{"marker":true}"#).unwrap();
        let packages = crate::library::scan(&store);
        let client = reqwest::Client::builder()
            .user_agent("PelicanMods-test")
            .build()
            .unwrap();

        tauri::async_runtime::block_on(async {
            let report = check(&client, &packages, &Default::default(), "4.5.2", "1.6.15")
                .await
                .unwrap();
            let info = &report.mods["esca.farmtypemanager"];
            let version = info.version.clone().expect("an update is suggested");
            let repo = github_repo(info.url.as_deref().unwrap()).expect("GitHub url");
            let zip = download_github(&client, &repo, "Farm Type Manager", &version, dir.path())
                .await
                .unwrap();
            let installed =
                crate::library::install_archive(&store, &dir.path().join("backups"), &zip).unwrap();
            assert_eq!(installed[0].replaced, ["FarmTypeManager"]);
            let pkg = &crate::library::scan(&store)[0];
            assert_eq!(pkg.mods[0].manifest.version, version);
            let cfg = pkg.path.join(&pkg.mods[0].rel_path).join("config.json");
            assert!(std::fs::read_to_string(cfg).unwrap().contains("marker"));
            let saved = crate::backups::list(&dir.path().join("backups"), "Esca.FarmTypeManager");
            assert_eq!(saved[0].version, "1.20.0");
        });
    }
}
