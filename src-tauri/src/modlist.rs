//! Mod lists: a profile written to a file someone else can install from.
//!
//! A list names each mod by UniqueID with its version and the update keys and GitHub
//! repo it came with, which is everything the installer needs to find it again. It
//! carries no files, so sharing one never redistributes anybody's mod.

use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::library::Package;

const FORMAT: &str = "pelican-modlist";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModList {
    /// Always "pelican-modlist"; tells the file apart from any other JSON.
    pub format: String,
    pub version: u32,
    pub name: String,
    #[serde(default)]
    pub game: Option<String>,
    #[serde(default)]
    pub smapi: Option<String>,
    pub mods: Vec<ListedMod>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ListedMod {
    pub unique_id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub update_keys: Vec<String>,
    #[serde(default)]
    pub github: Option<String>,
}

/// A listed mod next to what the library has of it.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ListedStatus {
    #[serde(flatten)]
    pub listed: ListedMod,
    /// The installed version, if any.
    pub installed: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModListView {
    pub name: String,
    pub game: Option<String>,
    pub smapi: Option<String>,
    pub mods: Vec<ListedStatus>,
    /// Mods enabled here that the list lacks; in co-op the other side won't have them.
    pub extra: Vec<ListedMod>,
}

/// Lists the mods in `enabled` packages, with the GitHub repos users attached in `repos`.
pub fn build(
    name: &str,
    packages: &[Package],
    enabled: &[String],
    repos: &BTreeMap<String, String>,
    game: Option<String>,
    smapi: Option<String>,
) -> ModList {
    let mods = packages
        .iter()
        .filter(|p| enabled.contains(&p.id))
        .flat_map(|p| &p.mods)
        .map(|m| ListedMod {
            unique_id: m.manifest.unique_id.clone(),
            name: m.manifest.name.clone(),
            version: m.manifest.version.clone(),
            update_keys: m.manifest.update_keys.clone(),
            github: repos.get(&m.manifest.unique_id.to_lowercase()).cloned(),
        })
        .collect();
    ModList {
        format: FORMAT.to_owned(),
        version: 1,
        name: name.to_owned(),
        game,
        smapi,
        mods,
    }
}

/// Reads a list from `file`, refusing JSON that isn't one.
pub fn read(file: &Path) -> AppResult<ModList> {
    let text = std::fs::read_to_string(file)?;
    let list: ModList =
        serde_json::from_str(&text).map_err(|e| AppError::BadModList(e.to_string()))?;
    if list.format != FORMAT {
        return Err(AppError::BadModList(file.display().to_string()));
    }
    Ok(list)
}

/// Sets each listed mod against the library's `packages`, and names the mods in
/// `enabled` packages the list lacks.
pub fn status(list: ModList, packages: &[Package], enabled: &[String]) -> ModListView {
    let listed: HashSet<String> = list
        .mods
        .iter()
        .map(|m| m.unique_id.to_lowercase())
        .collect();
    let extra = packages
        .iter()
        .filter(|p| enabled.contains(&p.id))
        .flat_map(|p| &p.mods)
        .filter(|m| !listed.contains(&m.manifest.unique_id.to_lowercase()))
        .map(|m| ListedMod {
            unique_id: m.manifest.unique_id.clone(),
            name: m.manifest.name.clone(),
            version: m.manifest.version.clone(),
            update_keys: Vec::new(),
            github: None,
        })
        .collect();
    let installed: BTreeMap<String, String> = packages
        .iter()
        .flat_map(|p| &p.mods)
        .map(|m| {
            (
                m.manifest.unique_id.to_lowercase(),
                m.manifest.version.clone(),
            )
        })
        .collect();
    ModListView {
        name: list.name,
        game: list.game,
        smapi: list.smapi,
        mods: list
            .mods
            .into_iter()
            .map(|l| ListedStatus {
                installed: installed.get(&l.unique_id.to_lowercase()).cloned(),
                listed: l,
            })
            .collect(),
        extra,
    }
}

/// The packages holding any of `unique_ids`, for a profile made from a list.
pub fn packages_for(packages: &[Package], unique_ids: &[String]) -> Vec<String> {
    let want: HashSet<String> = unique_ids.iter().map(|u| u.to_lowercase()).collect();
    packages
        .iter()
        .filter(|p| p.unique_ids().any(|u| want.contains(&u.to_lowercase())))
        .map(|p| p.id.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_mod(root: &Path, folder: &str, id: &str, version: &str) {
        let dir = root.join(folder);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("manifest.json"),
            format!(
                r#"{{"Name":"{folder}","Author":"a","Version":"{version}","Description":"",
                "UniqueID":"{id}","EntryDll":"x.dll","UpdateKeys":["Nexus:1"]}}"#
            ),
        )
        .unwrap();
    }

    #[test]
    fn writes_and_reads_lists() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("mods");
        write_mod(&store, "Automate", "Pathoschild.Automate", "2.0.0");
        write_mod(&store, "Off", "Someone.Off", "1.0.0");
        let packages = crate::library::scan(&store);
        let repos = BTreeMap::from([("pathoschild.automate".to_owned(), "a/b".to_owned())]);
        let list = build(
            "Coop",
            &packages,
            &["Automate".to_owned()],
            &repos,
            Some("1.6.15".into()),
            None,
        );
        assert_eq!(list.mods.len(), 1);
        assert_eq!(list.mods[0].github.as_deref(), Some("a/b"));

        let file = dir.path().join("coop.json");
        std::fs::write(&file, serde_json::to_vec(&list).unwrap()).unwrap();
        let back = read(&file).unwrap();
        assert_eq!(back, list);

        let mut other = back.clone();
        other.mods.push(ListedMod {
            unique_id: "Missing.Mod".into(),
            name: "Missing".into(),
            version: "1.0.0".into(),
            update_keys: Vec::new(),
            github: None,
        });
        let view = status(other, &packages, &["Automate".into(), "Off".into()]);
        assert_eq!(view.mods[0].installed.as_deref(), Some("2.0.0"));
        assert_eq!(view.mods[1].installed, None);
        assert_eq!(view.extra.len(), 1);
        assert_eq!(view.extra[0].unique_id, "Someone.Off");
        assert_eq!(
            packages_for(&packages, &["pathoschild.automate".into()]),
            ["Automate"]
        );

        std::fs::write(
            &file,
            r#"{"format":"other","version":1,"name":"x","mods":[]}"#,
        )
        .unwrap();
        assert!(read(&file).is_err());
    }

    // round trip over a real library, read-only:
    // PELICAN_TEST_STORE=<app data>/mods cargo test -- --ignored real_list_round_trip --nocapture
    #[test]
    #[ignore]
    fn real_list_round_trip() {
        let packages =
            crate::library::scan(Path::new(&std::env::var("PELICAN_TEST_STORE").unwrap()));
        let enabled: Vec<String> = packages.iter().map(|p| p.id.clone()).collect();
        let list = build("Mine", &packages, &enabled, &BTreeMap::new(), None, None);
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("mine.json");
        std::fs::write(&file, serde_json::to_vec_pretty(&list).unwrap()).unwrap();
        let view = status(read(&file).unwrap(), &packages, &enabled);
        let missing = view.mods.iter().filter(|m| m.installed.is_none()).count();
        eprintln!(
            "{} mods listed, {} bytes, {} missing, {} extra",
            view.mods.len(),
            std::fs::metadata(&file).unwrap().len(),
            missing,
            view.extra.len()
        );
        assert_eq!(missing, 0);
        assert!(view.extra.is_empty());
    }
}
