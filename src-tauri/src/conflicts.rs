//! Content packs that would fight over the same asset, found before the game does.
//!
//! Content Patcher lets one pack replace an asset outright (`"Action": "Load"`), and two
//! packs doing it with the default `Exclusive` priority makes it drop both with an error
//! in the log. Each enabled pack's patches, includes followed, say so up front.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::Serialize;
use serde_json::Value;

use crate::library::Package;

const CONTENT_PATCHER: &str = "pathoschild.contentpatcher";
/// Include chains this deep are a loop or a mistake; Content Patcher's own packs stay shallow.
const MAX_INCLUDE_DEPTH: u32 = 6;

/// Two or more packs loading one asset exclusively.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Conflict {
    /// The asset, as the patches name it ("Animals/horse").
    pub target: String,
    /// Package ids.
    pub packages: Vec<String>,
    /// Some of the patches only apply under conditions, so the packs may never meet.
    pub conditional: bool,
}

#[derive(Default)]
struct Loads {
    packages: BTreeSet<String>,
    conditional: bool,
    shown: String,
}

/// Finds exclusive loads of the same asset among the `enabled` packages.
pub fn find(packages: &[Package], enabled: &[String]) -> Vec<Conflict> {
    let mut by_target: BTreeMap<String, Loads> = BTreeMap::new();
    for p in packages.iter().filter(|p| enabled.contains(&p.id)) {
        for m in &p.mods {
            let for_cp = m
                .manifest
                .content_pack_for
                .as_ref()
                .is_some_and(|d| d.unique_id.eq_ignore_ascii_case(CONTENT_PATCHER));
            if !for_cp {
                continue;
            }
            let dir = p.path.join(&m.rel_path);
            let mut loads = Vec::new();
            collect(&dir, Path::new("content.json"), 0, &mut loads);
            for (target, conditional) in loads {
                let entry = by_target
                    .entry(target.to_lowercase().replace('\\', "/"))
                    .or_default();
                if entry.shown.is_empty() {
                    entry.shown = target.replace('\\', "/");
                }
                entry.packages.insert(p.id.clone());
                entry.conditional |= conditional;
            }
        }
    }
    by_target
        .into_values()
        .filter(|l| l.packages.len() > 1)
        .map(|l| Conflict {
            target: l.shown,
            packages: l.packages.into_iter().collect(),
            conditional: l.conditional,
        })
        .collect()
}

// pushes (target, has conditions) for every exclusive Load in `file` and what it includes
fn collect(dir: &Path, file: &Path, depth: u32, out: &mut Vec<(String, bool)>) {
    if depth > MAX_INCLUDE_DEPTH {
        return;
    }
    // FromFile is the pack's word; keep it inside the pack ("/dev/zero" would read forever)
    let inside = file
        .components()
        .all(|c| matches!(c, std::path::Component::Normal(_)));
    if !inside {
        return;
    }
    let Ok(text) = crate::jsonc::read(&dir.join(file)) else {
        return;
    };
    let Ok(json) = crate::jsonc::parse::<Value>(&text) else {
        return;
    };
    let Some(changes) = json.get("Changes").and_then(Value::as_array) else {
        return;
    };
    for change in changes {
        let field = |k: &str| change.get(k).and_then(Value::as_str);
        let action = field("Action").unwrap_or("");
        let conditional = change
            .get("When")
            .is_some_and(|w| w.as_object().is_some_and(|o| !o.is_empty()));
        // tokens are resolved in game; a name we can't read can't be compared
        let names = |v: &str| {
            v.split(',')
                .map(str::trim)
                .filter(|t| !t.is_empty() && !t.contains("{{"))
                .map(str::to_owned)
                .collect::<Vec<_>>()
        };
        if action.eq_ignore_ascii_case("Include") {
            for from in names(field("FromFile").unwrap_or("")) {
                collect(dir, Path::new(&from), depth + 1, out);
            }
        } else if action.eq_ignore_ascii_case("Load") {
            let exclusive = field("Priority").is_none_or(|p| p.eq_ignore_ascii_case("Exclusive"));
            if exclusive {
                for target in names(field("Target").unwrap_or("")) {
                    out.push((target, conditional));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pack(root: &Path, folder: &str, content: &str, extra: Option<(&str, &str)>) {
        let dir = root.join(folder);
        std::fs::create_dir_all(dir.join("data")).unwrap();
        std::fs::write(
            dir.join("manifest.json"),
            format!(
                r#"{{"Name":"{folder}","Author":"a","Version":"1.0.0","Description":"","UniqueID":"a.{folder}",
                "ContentPackFor":{{"UniqueID":"Pathoschild.ContentPatcher"}}}}"#
            ),
        )
        .unwrap();
        std::fs::write(dir.join("content.json"), content).unwrap();
        if let Some((name, body)) = extra {
            std::fs::write(dir.join(name), body).unwrap();
        }
    }

    #[test]
    fn finds_exclusive_loads_of_one_asset() {
        let dir = tempfile::tempdir().unwrap();
        // comments and trailing commas, as content packs write them
        pack(
            dir.path(),
            "CuterHorses",
            r#"{ "Format": "2.0.0", // horses
                "Changes": [ { "Action": "Load", "Target": "Animals/horse", "FromFile": "a.png" }, ] }"#,
            None,
        );
        pack(
            dir.path(),
            "NewHorses",
            r#"{ "Changes": [ { "Action": "Include", "FromFile": "data/horses.json" } ] }"#,
            Some((
                "data/horses.json",
                r#"{ "Changes": [ { "Action": "Load", "Target": "Animals\\horse, Animals/Cat", "FromFile": "b.png" } ] }"#,
            )),
        );
        pack(
            dir.path(),
            "Polite",
            r#"{ "Changes": [ { "Action": "Load", "Target": "Animals/Cat", "Priority": "Low", "FromFile": "c.png" },
                              { "Action": "Load", "Target": "{{Season}}/Thing", "FromFile": "d.png" } ] }"#,
            None,
        );
        let packages = crate::library::scan(dir.path());
        let all: Vec<String> = packages.iter().map(|p| p.id.clone()).collect();
        let found = find(&packages, &all);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].target, "Animals/horse");
        assert_eq!(found[0].packages, ["CuterHorses", "NewHorses"]);
        assert!(!found[0].conditional);
        assert!(find(&packages, &["CuterHorses".into()]).is_empty());
    }

    // PELICAN_TEST_STORE=<app data>/mods cargo test -- --ignored real_conflicts --nocapture
    #[test]
    #[ignore]
    fn real_conflicts() {
        let packages =
            crate::library::scan(Path::new(&std::env::var("PELICAN_TEST_STORE").unwrap()));
        let all: Vec<String> = packages.iter().map(|p| p.id.clone()).collect();
        let started = std::time::Instant::now();
        let found = find(&packages, &all);
        eprintln!("{} conflicts in {:?}", found.len(), started.elapsed());
        for c in &found {
            eprintln!(
                "  {} {:?} conditional={}",
                c.target, c.packages, c.conditional
            );
        }
    }
}
