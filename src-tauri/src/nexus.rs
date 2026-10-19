//! Nexus Mods: browsing through the public GraphQL API, downloading through REST v1.
//!
//! Browsing needs no account. Download links need the user's personal API key, and for
//! non-Premium accounts also the one-time `key`/`expires` pair Nexus puts into the
//! `nxm://` link when the user clicks "Mod Manager Download" on the site. That is
//! Nexus' rule for every mod manager, not a choice of this app.

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::error::{AppError, AppResult};

const GRAPHQL: &str = "https://api.nexusmods.com/v2/graphql";
const REST: &str = "https://api.nexusmods.com/v1";
pub const GAME_DOMAIN: &str = "stardewvalley";
pub const GAME_ID: u32 = 1303;

const KEYRING_SERVICE: &str = "dev.swchck.pelican-mods";
const KEYRING_USER: &str = "nexus-api-key";

/// Result ordering for [`search`].
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Sort {
    Endorsements,
    Downloads,
    Updated,
    Created,
}

impl Sort {
    fn field(self) -> &'static str {
        match self {
            Self::Endorsements => "endorsements",
            Self::Downloads => "downloads",
            Self::Updated => "updatedAt",
            Self::Created => "createdAt",
        }
    }
}

/// A search result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModCard {
    pub mod_id: u64,
    pub name: String,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub thumbnail_url: Option<String>,
    #[serde(default)]
    pub endorsements: u64,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
}

/// One page of search results.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchPage {
    pub total: u64,
    pub items: Vec<ModCard>,
    /// Categories matching the query with their mod counts, largest first.
    pub categories: Vec<(String, u64)>,
}

/// Narrowing beyond the name query.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SearchFilters {
    pub category: Option<String>,
    /// Unix seconds; only mods updated at or after this.
    pub updated_since: Option<u64>,
    pub min_endorsements: Option<u64>,
}

fn filter_json(query: &str, f: &SearchFilters) -> serde_json::Value {
    let mut filter = json!({
        "gameDomainName": [{ "value": GAME_DOMAIN }],
        "adultContent": [{ "value": false }],
    });
    let query = query.trim();
    if !query.is_empty() {
        filter["name"] = json!([{ "value": query, "op": "WILDCARD" }]);
    }
    if let Some(c) = f.category.as_deref().filter(|c| !c.is_empty()) {
        filter["categoryName"] = json!([{ "value": c }]);
    }
    // the date filter wants unix seconds as a string; ISO dates match nothing
    if let Some(t) = f.updated_since {
        filter["updatedAt"] = json!([{ "value": t.to_string(), "op": "GTE" }]);
    }
    if let Some(n) = f.min_endorsements {
        filter["endorsements"] = json!([{ "value": n, "op": "GTE" }]);
    }
    filter
}

/// Everything the mod page shows.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModDetails {
    pub mod_id: u64,
    pub name: String,
    #[serde(default)]
    pub summary: Option<String>,
    /// Nexus BBCode, with `<br />` for line breaks.
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub picture_url: Option<String>,
    #[serde(default)]
    pub endorsements: u64,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub requirements: Vec<Requirement>,
    #[serde(default)]
    pub files: Vec<ModFile>,
}

/// A mod Nexus lists as required.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Requirement {
    #[serde(default, deserialize_with = "lenient_u64")]
    pub mod_id: Option<u64>,
    #[serde(default)]
    pub mod_name: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub external_requirement: bool,
}

/// A downloadable file of a mod.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModFile {
    pub file_id: u64,
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
    /// MAIN, OPTIONAL, UPDATE, MISCELLANEOUS, OLD_VERSION, ARCHIVED.
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default, deserialize_with = "lenient_u64")]
    pub size_in_bytes: Option<u64>,
    /// Unix seconds.
    #[serde(default)]
    pub date: u64,
    #[serde(default)]
    pub description: Option<String>,
}

/// The account behind an API key.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub name: String,
    pub is_premium: bool,
    pub profile_url: Option<String>,
}

// Nexus returns some numeric fields as strings ("sizeInBytes": "54629")
fn lenient_u64<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<u64>, D::Error> {
    Ok(match serde_json::Value::deserialize(d)? {
        serde_json::Value::Number(n) => n.as_u64(),
        serde_json::Value::String(s) => s.parse().ok(),
        _ => None,
    })
}

async fn graphql<T: for<'de> Deserialize<'de>>(
    client: &reqwest::Client,
    query: &str,
    variables: serde_json::Value,
) -> AppResult<T> {
    #[derive(Deserialize)]
    struct Resp<T> {
        data: Option<T>,
        #[serde(default)]
        errors: Vec<serde_json::Value>,
    }
    let resp: Resp<T> = client
        .post(GRAPHQL)
        .json(&json!({ "query": query, "variables": variables }))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    match (resp.data, resp.errors.first()) {
        (Some(d), None) => Ok(d),
        (_, Some(e)) => Err(AppError::Network(
            e.get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("GraphQL error")
                .to_owned(),
        )),
        (None, None) => Err(AppError::Network("empty GraphQL response".into())),
    }
}

/// Searches Stardew Valley mods by name; an empty `query` lists the whole catalogue.
pub async fn search(
    client: &reqwest::Client,
    query: &str,
    filters: &SearchFilters,
    sort: Sort,
    offset: u32,
    count: u32,
) -> AppResult<SearchPage> {
    #[derive(Deserialize)]
    struct Data {
        mods: Mods,
        facets: Mods,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Mods {
        total_count: u64,
        nodes: Vec<ModCard>,
        #[serde(default)]
        facets_data: Option<Facets>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Facets {
        #[serde(default)]
        category_name: std::collections::HashMap<String, u64>,
    }
    // facets ignore the category filter so the list keeps showing the other choices
    let facet_filters = SearchFilters {
        category: None,
        ..filters.clone()
    };
    let filter = filter_json(query, filters);
    let sort = json!([{ sort.field(): { "direction": "DESC" } }]);
    let data: Data = graphql(
        client,
        "query($f: ModsFilter, $ff: ModsFilter, $s: [ModsSort!], $o: Int, $c: Int) {
            mods(filter: $f, sort: $s, offset: $o, count: $c) {
                totalCount
                nodes { modId name summary author version thumbnailUrl endorsements downloads updatedAt category }
            }
            facets: mods(filter: $ff, count: 0, facets: { categoryName: [] }) { totalCount nodes { modId } facetsData }
        }",
        json!({
            "f": filter,
            "ff": filter_json(query, &facet_filters),
            "s": sort,
            "o": offset,
            "c": count.min(50),
        }),
    )
    .await?;
    let mut categories: Vec<(String, u64)> = data
        .facets
        .facets_data
        .map(|f| f.category_name.into_iter().collect())
        .unwrap_or_default();
    categories.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    Ok(SearchPage {
        total: data.mods.total_count,
        items: data.mods.nodes,
        categories,
    })
}

/// Loads a mod's page: description, requirements and files.
pub async fn details(client: &reqwest::Client, mod_id: u64) -> AppResult<ModDetails> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Data {
        r#mod: Option<RawMod>,
        mod_files: Option<Vec<ModFile>>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct RawMod {
        #[serde(flatten)]
        details: ModDetails,
        mod_requirements: Option<RawRequirements>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct RawRequirements {
        nexus_requirements: Option<RequirementPage>,
    }
    #[derive(Deserialize)]
    struct RequirementPage {
        nodes: Vec<Requirement>,
    }
    let data: Data = graphql(
        client,
        "query($m: ID!, $g: ID!) {
            mod(modId: $m, gameId: $g) {
                modId name summary description author version pictureUrl endorsements downloads updatedAt category
                modRequirements { nexusRequirements { nodes { modId modName url notes externalRequirement } } }
            }
            modFiles(modId: $m, gameId: $g) { fileId name version category sizeInBytes date description }
        }",
        json!({ "m": mod_id, "g": GAME_ID }),
    )
    .await?;
    let raw = data
        .r#mod
        .ok_or_else(|| AppError::NotFound(format!("nexus mod {mod_id}")))?;
    let mut details = raw.details;
    details.requirements = raw
        .mod_requirements
        .and_then(|r| r.nexus_requirements)
        .map(|p| p.nodes)
        .unwrap_or_default();
    let mut files = data.mod_files.unwrap_or_default();
    files.sort_by_key(|f| std::cmp::Reverse(f.date));
    details.files = files;
    Ok(details)
}

fn rest(client: &reqwest::Client, path: &str, key: &str) -> reqwest::RequestBuilder {
    client
        .get(format!("{REST}{path}"))
        .header("apikey", key)
        .header("Application-Name", "Pelican Mods")
        .header("Application-Version", env!("CARGO_PKG_VERSION"))
}

/// Checks an API key and returns its account.
pub async fn validate(client: &reqwest::Client, key: &str) -> AppResult<Account> {
    #[derive(Deserialize)]
    struct Raw {
        name: String,
        is_premium: bool,
        profile_url: Option<String>,
    }
    let resp = rest(client, "/users/validate.json", key).send().await?;
    if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(AppError::NexusKeyRejected);
    }
    let raw: Raw = resp.error_for_status()?.json().await?;
    Ok(Account {
        name: raw.name,
        is_premium: raw.is_premium,
        profile_url: raw.profile_url,
    })
}

/// One-time credentials from an `nxm://` link; Premium accounts download without them.
#[derive(Debug, Clone, PartialEq)]
pub struct LinkGrant {
    pub key: String,
    pub expires: u64,
}

/// Resolves a CDN URL for a mod file.
pub async fn download_url(
    client: &reqwest::Client,
    api_key: &str,
    mod_id: u64,
    file_id: u64,
    grant: Option<&LinkGrant>,
) -> AppResult<String> {
    #[derive(Deserialize)]
    struct Mirror {
        #[serde(rename = "URI")]
        uri: String,
    }
    let mut path = format!("/games/{GAME_DOMAIN}/mods/{mod_id}/files/{file_id}/download_link.json");
    if let Some(g) = grant {
        // encoded: the key comes from a link any website can craft
        let query = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("key", &g.key)
            .append_pair("expires", &g.expires.to_string())
            .finish();
        path.push('?');
        path.push_str(&query);
    }
    let resp = rest(client, &path, api_key).send().await?;
    match resp.status() {
        reqwest::StatusCode::UNAUTHORIZED => return Err(AppError::NexusKeyRejected),
        // non-Premium without a grant, or an expired one
        reqwest::StatusCode::FORBIDDEN | reqwest::StatusCode::GONE => {
            return Err(AppError::NexusNeedsPremium);
        }
        _ => {}
    }
    let mirrors: Vec<Mirror> = resp.error_for_status()?.json().await?;
    mirrors
        .into_iter()
        .next()
        .map(|m| m.uri)
        .ok_or_else(|| AppError::NoDownload(format!("nexus {mod_id}/{file_id}")))
}

/// A parsed `nxm://stardewvalley/mods/<mod>/files/<file>?key=…&expires=…` link.
#[derive(Debug, Clone, PartialEq)]
pub struct NxmLink {
    pub mod_id: u64,
    pub file_id: u64,
    pub grant: Option<LinkGrant>,
}

/// Parses an nxm link for Stardew Valley; links for other games are refused.
pub fn parse_nxm(raw: &str) -> AppResult<NxmLink> {
    let bad = || AppError::BadNxm(raw.to_owned());
    let url = url::Url::parse(raw).map_err(|_| bad())?;
    if url.scheme() != "nxm"
        || !url
            .host_str()
            .is_some_and(|h| h.eq_ignore_ascii_case(GAME_DOMAIN))
    {
        return Err(bad());
    }
    let parts: Vec<&str> = url.path_segments().map(|s| s.collect()).unwrap_or_default();
    let ["mods", mod_id, "files", file_id] = parts.as_slice() else {
        return Err(bad());
    };
    let mod_id = mod_id.parse().map_err(|_| bad())?;
    let file_id = file_id.parse().map_err(|_| bad())?;
    let mut key = None;
    let mut expires = None;
    for (k, v) in url.query_pairs() {
        match k.as_ref() {
            "key" => key = Some(v.into_owned()),
            "expires" => expires = v.parse().ok(),
            _ => {}
        }
    }
    let grant = match (key, expires) {
        (Some(key), Some(expires)) => Some(LinkGrant { key, expires }),
        _ => None,
    };
    Ok(NxmLink {
        mod_id,
        file_id,
        grant,
    })
}

/// Page where a non-Premium user clicks "Mod Manager Download" for this file.
pub fn file_page(mod_id: u64, file_id: u64) -> String {
    format!(
        "https://www.nexusmods.com/{GAME_DOMAIN}/mods/{mod_id}?tab=files&file_id={file_id}&nmm=1"
    )
}

/// Extracts the mod id from a `nexusmods.com/stardewvalley/mods/<id>` page URL.
pub fn mod_page_id(url: &str) -> Option<u64> {
    let url = url::Url::parse(url).ok()?;
    if !url.host_str()?.ends_with("nexusmods.com") {
        return None;
    }
    let parts: Vec<&str> = url.path_segments()?.collect();
    match parts.as_slice() {
        [game, "mods", id, ..] if game.eq_ignore_ascii_case(GAME_DOMAIN) => id.parse().ok(),
        _ => None,
    }
}

/// Picks the file that carries `version` of the mod called `mod_name`.
///
/// Prefers MAIN files of exactly that version; when a mod ships several (one per
/// mod in a pack, or per platform), the one whose name contains `mod_name` wins.
pub fn pick_update_file<'a>(
    files: &'a [ModFile],
    version: &str,
    mod_name: &str,
) -> Option<&'a ModFile> {
    let main = |f: &&ModFile| {
        f.category
            .as_deref()
            .is_none_or(|c| c.eq_ignore_ascii_case("MAIN"))
    };
    let exact: Vec<&ModFile> = files
        .iter()
        .filter(main)
        .filter(|f| {
            f.version
                .as_deref()
                .is_some_and(|v| crate::version::compare(v, version).is_eq())
        })
        .collect();
    // some authors never fill in a file's version; a lone MAIN file is still the one
    let pool = if exact.is_empty() {
        let mains: Vec<&ModFile> = files.iter().filter(main).collect();
        if mains.len() != 1 {
            return None;
        }
        mains
    } else {
        exact
    };
    let want = compact(mod_name);
    pool.iter()
        .find(|f| compact(&f.name).contains(&want))
        .or_else(|| pool.first())
        .copied()
}

fn compact(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn keyring() -> AppResult<keyring::Entry> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .map_err(|e| AppError::Keychain(e.to_string()))
}

/// Reads the stored API key from the macOS Keychain.
pub fn load_key() -> AppResult<Option<String>> {
    match keyring()?.get_password() {
        Ok(k) => Ok(Some(k)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(AppError::Keychain(e.to_string())),
    }
}

pub fn save_key(key: &str) -> AppResult<()> {
    keyring()?
        .set_password(key)
        .map_err(|e| AppError::Keychain(e.to_string()))
}

pub fn clear_key() -> AppResult<()> {
    match keyring()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(AppError::Keychain(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(id: u64, name: &str, version: Option<&str>, category: &str) -> ModFile {
        ModFile {
            file_id: id,
            name: name.into(),
            version: version.map(Into::into),
            category: Some(category.into()),
            size_in_bytes: None,
            date: 0,
            description: None,
        }
    }

    #[test]
    fn picks_update_files() {
        let files = [
            file(5, "Lookup Anything", Some("1.55.0"), "MAIN"),
            file(4, "Lookup Anything", Some("1.54.0"), "OLD_VERSION"),
            file(3, "Extra assets", Some("1.55.0"), "OPTIONAL"),
        ];
        assert_eq!(
            pick_update_file(&files, "1.55.0", "Lookup Anything")
                .unwrap()
                .file_id,
            5
        );
        assert_eq!(pick_update_file(&files, "v1.55", "x").unwrap().file_id, 5);
        let pack = [
            file(9, "Content Patcher pack", Some("2.0.0"), "MAIN"),
            file(8, "Farm Type Manager", Some("2.0.0"), "MAIN"),
        ];
        assert_eq!(
            pick_update_file(&pack, "2.0.0", "Farm Type Manager")
                .unwrap()
                .file_id,
            8
        );
        let unversioned = [
            file(1, "Mod", None, "MAIN"),
            file(2, "Old", None, "OLD_VERSION"),
        ];
        assert_eq!(
            pick_update_file(&unversioned, "3.0.0", "Mod")
                .unwrap()
                .file_id,
            1
        );
        let ambiguous = [file(1, "A", None, "MAIN"), file(2, "B", None, "MAIN")];
        assert!(pick_update_file(&ambiguous, "3.0.0", "C").is_none());
    }

    #[test]
    fn reads_mod_page_ids() {
        assert_eq!(
            mod_page_id("https://www.nexusmods.com/stardewvalley/mods/541"),
            Some(541)
        );
        assert_eq!(
            mod_page_id("https://www.nexusmods.com/stardewvalley/mods/541?tab=files"),
            Some(541)
        );
        assert_eq!(
            mod_page_id("https://www.nexusmods.com/skyrim/mods/541"),
            None
        );
        assert_eq!(
            mod_page_id("https://github.com/stardewvalley/mods/541"),
            None
        );
    }

    #[test]
    fn parses_nxm_links() {
        let l = parse_nxm(
            "nxm://stardewvalley/mods/541/files/12345?key=abc&expires=1700000000&user_id=9",
        )
        .unwrap();
        assert_eq!(l.mod_id, 541);
        assert_eq!(l.file_id, 12345);
        assert_eq!(
            l.grant,
            Some(LinkGrant {
                key: "abc".into(),
                expires: 1_700_000_000
            })
        );
        assert!(
            parse_nxm("nxm://StardewValley/mods/1/files/2")
                .unwrap()
                .grant
                .is_none()
        );
        assert!(parse_nxm("nxm://skyrim/mods/1/files/2").is_err());
        assert!(parse_nxm("nxm://stardewvalley/collections/x").is_err());
        assert!(parse_nxm("https://stardewvalley/mods/1/files/2").is_err());
    }

    #[test]
    fn reads_details_with_string_numbers() {
        let raw = r#"{"modId":541,"name":"Lookup Anything","endorsements":5,
            "files":[{"fileId":1,"name":"a.zip","sizeInBytes":"54629","date":10}],
            "requirements":[{"modId":"2400","modName":"SMAPI","externalRequirement":false}]}"#;
        let d: ModDetails = serde_json::from_str(raw).unwrap();
        assert_eq!(d.files[0].size_in_bytes, Some(54629));
        assert_eq!(d.requirements[0].mod_id, Some(2400));
    }

    // network: the public GraphQL API
    #[test]
    #[ignore]
    fn searches_and_loads_details_live() {
        let client = reqwest::Client::builder()
            .user_agent("PelicanMods-test")
            .build()
            .unwrap();
        tauri::async_runtime::block_on(async {
            let page = search(
                &client,
                "lookup anything",
                &SearchFilters::default(),
                Sort::Endorsements,
                0,
                5,
            )
            .await
            .unwrap();
            assert!(page.items.iter().any(|m| m.mod_id == 541), "{page:?}");
            let d = details(&client, 541).await.unwrap();
            assert_eq!(d.name, "Lookup Anything");
            assert!(d.description.unwrap().contains("[url="));
            assert!(
                d.files
                    .iter()
                    .any(|f| f.category.as_deref() == Some("MAIN"))
            );
            assert!(d.files.windows(2).all(|w| w[0].date >= w[1].date));
            let latest = d.version.clone().unwrap();
            let pick = pick_update_file(&d.files, &latest, "Lookup Anything").unwrap();
            assert_eq!(pick.category.as_deref(), Some("MAIN"));
        });
    }

    #[test]
    fn builds_filters() {
        let f = filter_json(
            " fish ",
            &SearchFilters {
                category: Some("Fishing".into()),
                updated_since: Some(1_790_812_800),
                min_endorsements: Some(100),
            },
        );
        assert_eq!(f["name"][0]["value"], "fish");
        assert_eq!(f["categoryName"][0]["value"], "Fishing");
        assert_eq!(f["updatedAt"][0]["value"], "1790812800");
        assert_eq!(f["endorsements"][0]["op"], "GTE");
        let empty = filter_json("", &SearchFilters::default());
        assert!(empty.get("name").is_none() && empty.get("categoryName").is_none());
    }

    // network: filters and facets against the live API
    #[test]
    #[ignore]
    fn filters_live() {
        let client = reqwest::Client::builder()
            .user_agent("PelicanMods-test")
            .build()
            .unwrap();
        tauri::async_runtime::block_on(async {
            let f = SearchFilters {
                category: Some("Fishing".into()),
                ..Default::default()
            };
            let page = search(&client, "", &f, Sort::Endorsements, 0, 5)
                .await
                .unwrap();
            assert!(
                page.items
                    .iter()
                    .all(|m| m.category.as_deref() == Some("Fishing"))
            );
            assert!(
                page.categories.len() > 5,
                "facets ignore the category filter"
            );
        });
    }
}
