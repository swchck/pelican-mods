//! Finds which saves still use a mod, so removing it doesn't silently cost the player.
//!
//! Two traces are enough in practice: game 1.6 data from mods carries the mod's
//! UniqueID ("Author.Mod_Item"), and map mods add locations ("Custom_AndyHouse") that
//! the save lists by name. Without the mod the game drops such locations on the next
//! save, NPCs living there included.

use std::collections::BTreeSet;
use std::path::Path;

use serde::Serialize;

use crate::saves;

/// Locations kept per save in the answer; the count says the rest.
const SHOWN: usize = 6;

/// How one save depends on the mod.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SaveUsage {
    pub save_id: String,
    pub farmer: String,
    pub farm_name: String,
    /// Mentions of the mod's UniqueIDs: its items, buildings and data.
    pub references: u32,
    /// Locations the mod names that the save has, the first few.
    pub locations: Vec<String>,
    pub location_count: u32,
}

/// Checks every save in `saves_dir` against the mod in `mod_dir` with `unique_ids`.
pub fn check(mod_dir: &Path, unique_ids: &[String], saves_dir: &Path) -> Vec<SaveUsage> {
    let ids: Vec<String> = unique_ids.iter().map(|u| u.to_lowercase()).collect();
    let names = declared_locations(mod_dir);
    saves::list(saves_dir)
        .into_iter()
        .filter_map(|s| {
            let text = std::fs::read_to_string(s.path.join(&s.id)).ok()?;
            let (references, found) = usage(&SaveText::new(&text), &ids, &names);
            (references > 0 || !found.is_empty()).then(|| SaveUsage {
                save_id: s.id.clone(),
                farmer: s.farmer.clone(),
                farm_name: s.farm_name.clone(),
                references,
                location_count: found.len() as u32,
                locations: found.into_iter().take(SHOWN).cloned().collect(),
            })
        })
        .collect()
}

/// Mentions of lowercased `ids` in the save, and which of the location `names` it has.
fn usage<'a>(
    save: &SaveText,
    ids: &[String],
    names: &'a BTreeSet<String>,
) -> (u32, Vec<&'a String>) {
    let references = ids
        .iter()
        .map(|id| save.lower.matches(id.as_str()).count() as u32)
        .sum();
    let found = names
        .iter()
        .filter(|n| save.locations.contains(*n))
        .collect();
    (references, found)
}

/// A save read once for many mods: lowercased for UniqueIDs, and its `Custom_`
/// location names, since checking each mod's names against 10 MB took seconds.
struct SaveText {
    lower: String,
    locations: BTreeSet<String>,
}

impl SaveText {
    fn new(text: &str) -> Self {
        let mut locations = BTreeSet::new();
        let mut rest = text;
        while let Some(at) = rest.find("<name>Custom_") {
            let tail = &rest[at + "<name>".len()..];
            let end = tail.find('<').unwrap_or(tail.len());
            locations.insert(tail[..end].to_owned());
            rest = &tail[end..];
        }
        Self {
            lower: text.to_lowercase(),
            locations,
        }
    }
}

/// A mod the save uses that won't be there when the game starts.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Missing {
    pub package: String,
    pub name: String,
    pub references: u32,
    pub location_count: u32,
    /// Turned off in the active profile; otherwise SMAPI skipped it last session.
    pub disabled: bool,
}

/// What the last played save would lose if the game started now.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Preflight {
    pub save_id: String,
    pub farmer: String,
    pub farm_name: String,
    pub missing: Vec<Missing>,
}

/// Checks the most recently played save against `packages`: those it uses that are
/// off in `enabled`, or whose mods SMAPI `skipped` (by name) last session.
///
/// Only the newest save: it's the one players usually load, and reading every save
/// before each launch would cost seconds on a big farm.
pub fn preflight(
    saves_dir: &Path,
    packages: &[crate::library::Package],
    enabled: &[String],
    skipped: &[String],
) -> Option<Preflight> {
    let save = saves::list(saves_dir)
        .into_iter()
        .max_by_key(|s| s.modified)?;
    let text = SaveText::new(&std::fs::read_to_string(save.path.join(&save.id)).ok()?);
    let missing: Vec<Missing> = packages
        .iter()
        .filter_map(|p| {
            let disabled = !enabled.contains(&p.id);
            let failed = p.mods.iter().any(|m| skipped.contains(&m.manifest.name));
            if !disabled && !failed {
                return None;
            }
            let ids: Vec<String> = p.unique_ids().map(str::to_lowercase).collect();
            let names = declared_locations(&p.path);
            let (references, found) = usage(&text, &ids, &names);
            (references > 0 || !found.is_empty()).then(|| Missing {
                package: p.id.clone(),
                name: p
                    .mods
                    .first()
                    .map(|m| m.manifest.name.clone())
                    .unwrap_or_else(|| p.id.clone()),
                references,
                location_count: found.len() as u32,
                disabled,
            })
        })
        .collect();
    (!missing.is_empty()).then_some(Preflight {
        save_id: save.id,
        farmer: save.farmer,
        farm_name: save.farm_name,
        missing,
    })
}

// "Custom_…" is the convention content packs name their locations by
fn declared_locations(mod_dir: &Path) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for entry in walkdir(mod_dir) {
        let Ok(text) = std::fs::read_to_string(&entry) else {
            continue;
        };
        let mut rest = text.as_str();
        while let Some(at) = rest.find("Custom_") {
            let tail = &rest[at..];
            let end = tail
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .unwrap_or(tail.len());
            if end > "Custom_".len() {
                names.insert(tail[..end].to_owned());
            }
            rest = &tail[end.max(1)..];
        }
    }
    names
}

fn walkdir(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("json"))
            {
                out.push(p);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_saves_using_a_mod() {
        let dir = tempfile::tempdir().unwrap();
        let mod_dir = dir.path().join("SVE");
        std::fs::create_dir_all(mod_dir.join("data")).unwrap();
        std::fs::write(
            mod_dir.join("data/locations.json"),
            r#"{"CustomLocations":[{"Name":"Custom_AndyHouse"},{"Name":"Custom_Unused"}]}"#,
        )
        .unwrap();
        let saves_dir = dir.path().join("Saves");
        let save = |id: &str, body: &str| {
            let d = saves_dir.join(id);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join(id), body).unwrap();
            std::fs::write(
                d.join("SaveGameInfo"),
                "<Farmer><name>Ann</name><farmName>Daisy</farmName></Farmer>",
            )
            .unwrap();
        };
        save(
            "Uses_1",
            "<SaveGame><locations><GameLocation><name>Custom_AndyHouse</name></GameLocation>\
             </locations><item>FlashShifter.SVE_Fir</item></SaveGame>",
        );
        save(
            "Clean_2",
            "<SaveGame><locations><GameLocation><name>Farm</name></GameLocation></locations></SaveGame>",
        );
        let used = check(&mod_dir, &["flashshifter.sve".to_owned()], &saves_dir);
        assert_eq!(used.len(), 1);
        std::fs::write(
            mod_dir.join("manifest.json"),
            r#"{"Name":"SVE","Author":"a","Version":"1.0.0","Description":"","UniqueID":"FlashShifter.SVE","ContentPackFor":{"UniqueID":"Pathoschild.ContentPatcher"}}"#,
        )
        .unwrap();
        let packages = crate::library::scan(dir.path());
        let sve: Vec<_> = packages.iter().filter(|p| p.id == "SVE").cloned().collect();
        // the preflight reads only the newest save, and the clean one was written last
        std::fs::remove_dir_all(saves_dir.join("Clean_2")).unwrap();
        assert!(preflight(&saves_dir, &sve, &["SVE".into()], &[]).is_none());
        let off = preflight(&saves_dir, &sve, &[], &[]).expect("the save needs SVE");
        assert!(off.missing[0].disabled);
        let skipped = preflight(&saves_dir, &sve, &["SVE".into()], &["SVE".into()]).unwrap();
        assert!(!skipped.missing[0].disabled);
        assert_eq!(used[0].save_id, "Uses_1");
        assert_eq!(used[0].references, 1);
        assert_eq!(used[0].locations, ["Custom_AndyHouse"]);
    }

    // PELICAN_TEST_SAVES=<Saves> PELICAN_TEST_STORE=<mods> PELICAN_TEST_LOG=<SMAPI-latest.txt> \
    //   cargo test -- --ignored real_preflight --nocapture
    #[test]
    #[ignore]
    fn real_preflight() {
        let var = |k| std::path::PathBuf::from(std::env::var(k).unwrap());
        let packages = crate::library::scan(&var("PELICAN_TEST_STORE"));
        let skipped: Vec<String> = crate::smapi_log::read(&var("PELICAN_TEST_LOG"))
            .unwrap()
            .skipped
            .into_iter()
            .map(|s| s.name)
            .collect();
        let enabled: Vec<String> = packages.iter().map(|p| p.id.clone()).collect();
        let started = std::time::Instant::now();
        let pf = preflight(&var("PELICAN_TEST_SAVES"), &packages, &enabled, &skipped);
        eprintln!("{:?} in {:?}", pf, started.elapsed());
    }
}
