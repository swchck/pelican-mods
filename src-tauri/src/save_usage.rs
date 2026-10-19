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
            let lower = text.to_lowercase();
            let references = ids
                .iter()
                .map(|id| lower.matches(id.as_str()).count() as u32)
                .sum();
            let found: Vec<&String> = names
                .iter()
                .filter(|n| text.contains(&format!("<name>{n}</name>")))
                .collect();
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
        assert_eq!(used[0].save_id, "Uses_1");
        assert_eq!(used[0].references, 1);
        assert_eq!(used[0].locations, ["Custom_AndyHouse"]);
    }
}
