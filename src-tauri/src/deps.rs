//! Problems SMAPI would report at launch, worked out ahead of time for the active profile.

use std::collections::HashMap;

use serde::Serialize;

use crate::library::Package;
use crate::version;

/// Something that stops a mod from loading or makes it misbehave.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Issue {
    /// A required mod is not installed at all.
    MissingDependency {
        id: String,
        min: Option<String>,
    },
    /// A required mod is installed, but its package is off in this profile.
    DisabledDependency {
        id: String,
        package: String,
    },
    /// A dependency is on, but older than the manifest asks for.
    OutdatedDependency {
        id: String,
        need: String,
        have: String,
    },
    /// Another enabled package ships a mod with the same UniqueID; SMAPI loads neither.
    DuplicateId {
        package: String,
    },
    NeedsSmapi {
        need: String,
    },
    NeedsGame {
        need: String,
    },
}

/// Analysis of the active profile.
#[derive(Debug, Default)]
pub struct Report {
    /// Issues per mod, keyed by `(package id, mod rel path)`.
    pub issues: HashMap<(String, String), Vec<Issue>>,
    /// Names of enabled mods that require a UniqueID (lowercased), for "needed by" hints.
    pub dependents: HashMap<String, Vec<String>>,
}

/// Checks the enabled packages against each other and the installed SMAPI and game.
///
/// `enabled` decides which of `all` count; `bundled` are always on.
pub fn analyze(
    all: &[Package],
    bundled: &[Package],
    enabled: &[String],
    smapi: Option<&str>,
    game: Option<&str>,
) -> Report {
    let active: Vec<&Package> = bundled
        .iter()
        .chain(all.iter().filter(|p| enabled.contains(&p.id)))
        .collect();

    let mut loaded: HashMap<String, Vec<(&str, &str)>> = HashMap::new();
    for p in &active {
        for m in &p.mods {
            loaded
                .entry(m.manifest.unique_id.to_lowercase())
                .or_default()
                .push((p.id.as_str(), m.manifest.version.as_str()));
        }
    }
    let installed: HashMap<String, &str> = all
        .iter()
        .flat_map(|p| {
            p.mods
                .iter()
                .map(move |m| (m.manifest.unique_id.to_lowercase(), p.id.as_str()))
        })
        .collect();

    let mut report = Report::default();
    for p in &active {
        for m in &p.mods {
            let mf = &m.manifest;
            let mut issues = Vec::new();

            if let Some(owners) = loaded.get(&mf.unique_id.to_lowercase()) {
                for (other, _) in owners.iter().filter(|(id, _)| *id != p.id) {
                    issues.push(Issue::DuplicateId {
                        package: (*other).to_owned(),
                    });
                }
            }
            if let (Some(need), Some(have)) = (&mf.minimum_api_version, smapi)
                && !version::at_least(have, need)
            {
                issues.push(Issue::NeedsSmapi { need: need.clone() });
            }
            if let (Some(need), Some(have)) = (&mf.minimum_game_version, game)
                && !version::at_least(have, need)
            {
                issues.push(Issue::NeedsGame { need: need.clone() });
            }

            for dep in mf.content_pack_for.iter().chain(&mf.dependencies) {
                let key = dep.unique_id.to_lowercase();
                if dep.is_required {
                    report
                        .dependents
                        .entry(key.clone())
                        .or_default()
                        .push(mf.name.clone());
                }
                match loaded.get(&key).and_then(|v| v.first()) {
                    Some((_, have)) => {
                        if let Some(need) = &dep.minimum_version
                            && !version::at_least(have, need)
                        {
                            issues.push(Issue::OutdatedDependency {
                                id: dep.unique_id.clone(),
                                need: need.clone(),
                                have: (*have).to_owned(),
                            });
                        }
                    }
                    None if !dep.is_required => {}
                    None => match installed.get(&key) {
                        Some(pkg) => issues.push(Issue::DisabledDependency {
                            id: dep.unique_id.clone(),
                            package: (*pkg).to_owned(),
                        }),
                        None => issues.push(Issue::MissingDependency {
                            id: dep.unique_id.clone(),
                            min: dep.minimum_version.clone(),
                        }),
                    },
                }
            }
            if !issues.is_empty() {
                report
                    .issues
                    .insert((p.id.clone(), m.rel_path.clone()), issues);
            }
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::FoundMod;
    use crate::manifest;

    fn pkg(id: &str, manifest_json: &str) -> Package {
        Package {
            id: id.into(),
            path: id.into(),
            mods: vec![FoundMod {
                rel_path: String::new(),
                manifest: manifest::parse(manifest_json).unwrap(),
            }],
            broken: vec![],
        }
    }

    fn issues<'a>(r: &'a Report, id: &str) -> &'a [Issue] {
        r.issues
            .get(&(id.to_owned(), String::new()))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    #[test]
    fn finds_dependency_problems() {
        let cp = pkg(
            "CP",
            r#"{"Name":"Content Patcher","UniqueID":"P.CP","Version":"2.0.0"}"#,
        );
        let pack = pkg(
            "Pack",
            r#"{"Name":"Pack","UniqueID":"s.Pack","Version":"1.0","ContentPackFor":{"UniqueID":"p.cp","MinimumVersion":"2.5.0"}}"#,
        );
        let lookup = pkg(
            "Lookup",
            r#"{"Name":"Lookup","UniqueID":"P.Lookup","Version":"1.0","MinimumApiVersion":"4.6.0",
                "Dependencies":[{"UniqueID":"x.Gmcm","IsRequired":false},{"UniqueID":"x.Core"},{"UniqueID":"x.Off"}]}"#,
        );
        let off = pkg(
            "Off",
            r#"{"Name":"Off","UniqueID":"x.Off","Version":"1.0"}"#,
        );
        let all = [cp, pack, lookup, off];
        let enabled = ["CP", "Pack", "Lookup"].map(String::from);
        let r = analyze(&all, &[], &enabled, Some("4.5.2"), Some("1.6.15"));

        assert_eq!(
            issues(&r, "Pack"),
            [Issue::OutdatedDependency {
                id: "p.cp".into(),
                need: "2.5.0".into(),
                have: "2.0.0".into()
            }]
        );
        assert_eq!(
            issues(&r, "Lookup"),
            [
                Issue::NeedsSmapi {
                    need: "4.6.0".into()
                },
                Issue::MissingDependency {
                    id: "x.Core".into(),
                    min: None
                },
                Issue::DisabledDependency {
                    id: "x.Off".into(),
                    package: "Off".into()
                },
            ]
        );
        assert!(issues(&r, "CP").is_empty());
        assert!(
            issues(&r, "Off").is_empty(),
            "disabled mods are not checked"
        );
        assert_eq!(r.dependents["p.cp"], ["Pack"]);
    }

    #[test]
    fn flags_duplicate_ids_and_counts_bundled() {
        let a = pkg("A", r#"{"Name":"A","UniqueID":"same","Version":"1"}"#);
        let b = pkg("B", r#"{"Name":"B","UniqueID":"Same","Version":"1"}"#);
        let host = pkg(
            "ConsoleCommands",
            r#"{"Name":"CC","UniqueID":"SMAPI.ConsoleCommands","Version":"4.5.2"}"#,
        );
        let user = pkg(
            "U",
            r#"{"Name":"U","UniqueID":"u","Version":"1","Dependencies":[{"UniqueID":"SMAPI.ConsoleCommands"}]}"#,
        );
        let enabled = ["A", "B", "U"].map(String::from);
        let r = analyze(&[a, b, user], &[host], &enabled, None, None);
        assert_eq!(
            issues(&r, "A"),
            [Issue::DuplicateId {
                package: "B".into()
            }]
        );
        assert!(issues(&r, "U").is_empty());
    }
}
