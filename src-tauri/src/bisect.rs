//! Finding the mod behind a problem by halving: each round turns on half of the
//! suspects, the player says whether the problem is still there, and the suspects halve.
//!
//! A round runs in a profile of its own, so the player's profiles never change; the
//! search ends by switching back. A half always takes the required dependencies of its
//! mods along, or a mod would just be skipped by SMAPI and look innocent. Cleared mods stay
//! on, so two mods that only break together end the search on the second of them; a
//! partner search then pins it on and halves the cleared ones to find the first.

use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::AppResult;
use crate::library::Package;

/// A search in progress, saved between launches.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Bisect {
    /// The profile to go back to.
    pub original: String,
    /// The profile the rounds run in.
    pub profile: String,
    /// Package ids that may still be behind the problem.
    pub suspects: Vec<String>,
    /// Enabled packages cleared already; they stay on.
    pub cleared: Vec<String>,
    /// The suspects on in this round.
    pub testing: Vec<String>,
    pub round: u32,
    /// Always on: the mod a partner search looks for a partner of.
    #[serde(default)]
    pub pinned: Vec<String>,
    /// This round runs the pinned mod alone, to see whether it needs a partner at all.
    #[serde(default)]
    pub alone: bool,
}

/// What the search knows after an answer.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Step {
    /// Another round: these packages are on.
    Next { enabled: Vec<String> },
    /// One suspect is left; `with` is the mod it breaks together with, for a partner
    /// search, and `cleared` the mods a partner search would look among.
    Found {
        package: String,
        with: Option<String>,
        cleared: Vec<String>,
    },
    /// No suspect is left: the problem isn't in any mod that was on.
    Clean,
}

impl Bisect {
    /// Starts with every package in `enabled` under suspicion.
    pub fn start(
        original: String,
        profile: String,
        enabled: &[String],
        packages: &[Package],
    ) -> Self {
        let mut b = Self {
            original,
            profile,
            suspects: enabled.to_vec(),
            cleared: Vec::new(),
            testing: Vec::new(),
            round: 0,
            pinned: Vec::new(),
            alone: false,
        };
        b.split(packages);
        b
    }

    /// Looks for the mod `found` only breaks together with, among `candidates`: the first
    /// round runs `found` alone.
    pub fn partner(
        original: String,
        profile: String,
        found: String,
        candidates: &[String],
    ) -> Self {
        Self {
            original,
            profile,
            suspects: candidates
                .iter()
                .filter(|c| **c != found)
                .cloned()
                .collect(),
            cleared: Vec::new(),
            testing: Vec::new(),
            round: 1,
            pinned: vec![found],
            alone: true,
        }
    }

    /// The packages on in this round.
    pub fn enabled(&self) -> Vec<String> {
        self.pinned
            .iter()
            .chain(&self.cleared)
            .chain(&self.testing)
            .cloned()
            .collect()
    }

    /// Narrows the suspects by whether the problem showed with this round's packages.
    pub fn answer(&mut self, problem: bool, packages: &[Package]) -> Step {
        if std::mem::take(&mut self.alone) {
            if problem || self.suspects.is_empty() {
                return Step::Found {
                    package: self.pinned[0].clone(),
                    with: None,
                    cleared: Vec::new(),
                };
            }
            self.round = 0;
            self.split(packages);
            return Step::Next {
                enabled: self.enabled(),
            };
        }
        if problem {
            let testing: BTreeSet<&String> = self.testing.iter().collect();
            self.suspects.retain(|s| testing.contains(s));
        } else {
            // everything on this round is innocent
            let testing = std::mem::take(&mut self.testing);
            self.suspects.retain(|s| !testing.contains(s));
            self.cleared.extend(testing);
        }
        match self.suspects.len() {
            0 => Step::Clean,
            1 => match self.pinned.first() {
                Some(pinned) => Step::Found {
                    package: pinned.clone(),
                    with: Some(self.suspects[0].clone()),
                    cleared: Vec::new(),
                },
                None => Step::Found {
                    package: self.suspects[0].clone(),
                    with: None,
                    cleared: self.cleared.clone(),
                },
            },
            _ => {
                self.split(packages);
                Step::Next {
                    enabled: self.enabled(),
                }
            }
        }
    }

    // turns on about half the suspects, each with the suspects it needs
    fn split(&mut self, packages: &[Package]) {
        self.round += 1;
        let needs = requirements(packages);
        let suspects: BTreeSet<&String> = self.suspects.iter().collect();
        let closure = |s: &String| {
            let mut out: BTreeSet<String> = BTreeSet::new();
            let mut stack = vec![s.clone()];
            while let Some(p) = stack.pop() {
                if out.insert(p.clone()) {
                    for dep in needs.get(&p).into_iter().flatten() {
                        if suspects.contains(dep) {
                            stack.push(dep.clone());
                        }
                    }
                }
            }
            out
        };
        // small blocks first: frameworks alone before the mods that pull them in
        let mut blocks: Vec<BTreeSet<String>> = self.suspects.iter().map(closure).collect();
        blocks.sort_by_key(BTreeSet::len);
        let all = self.suspects.len();
        let half = all.div_ceil(2);
        let mut on: BTreeSet<String> = BTreeSet::new();
        for block in blocks {
            if on.len() >= half {
                break;
            }
            let grown: BTreeSet<String> = on.union(&block).cloned().collect();
            // a round with every suspect on can't narrow anything
            if grown.len() < all {
                on = grown;
            }
        }
        if on.is_empty() {
            // every suspect needs every other one: a dependency loop, so split it plainly
            on = self.suspects.iter().take(half).cloned().collect();
        }
        self.testing = self
            .suspects
            .iter()
            .filter(|s| on.contains(*s))
            .cloned()
            .collect();
    }
}

// package id → package ids it needs, from required dependencies and content-pack hosts
fn requirements(packages: &[Package]) -> HashMap<String, Vec<String>> {
    let owner: HashMap<String, &str> = packages
        .iter()
        .flat_map(|p| {
            p.unique_ids()
                .map(move |id| (id.to_lowercase(), p.id.as_str()))
        })
        .collect();
    packages
        .iter()
        .map(|p| {
            let mut deps: Vec<String> = p
                .mods
                .iter()
                .flat_map(|m| {
                    let required = m
                        .manifest
                        .dependencies
                        .iter()
                        .filter(|d| d.is_required)
                        .map(|d| &d.unique_id);
                    required.chain(m.manifest.content_pack_for.iter().map(|d| &d.unique_id))
                })
                .filter_map(|id| owner.get(&id.to_lowercase()).map(|o| (*o).to_owned()))
                .filter(|o| *o != p.id)
                .collect();
            deps.sort();
            deps.dedup();
            (p.id.clone(), deps)
        })
        .collect()
}

/// The search's file, beside the profiles.
pub struct BisectStore {
    file: PathBuf,
}

impl BisectStore {
    pub fn new(file: PathBuf) -> Self {
        Self { file }
    }

    pub fn get(&self) -> Option<Bisect> {
        std::fs::read(&self.file)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
    }

    pub fn set(&self, b: Option<&Bisect>) -> AppResult<()> {
        match b {
            Some(b) => {
                if let Some(dir) = self.file.parent() {
                    std::fs::create_dir_all(dir)?;
                }
                std::fs::write(&self.file, serde_json::to_vec(b).expect("bisect serialize"))?;
            }
            None => {
                let _ = std::fs::remove_file(&self.file);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(dir: &std::path::Path, mods: &[(&str, &[&str])]) -> Vec<Package> {
        for (name, deps) in mods {
            let d = dir.join(name);
            std::fs::create_dir_all(&d).unwrap();
            let deps: Vec<String> = deps
                .iter()
                .map(|x| format!(r#"{{"UniqueID":"a.{x}"}}"#))
                .collect();
            std::fs::write(
                d.join("manifest.json"),
                format!(
                    r#"{{"Name":"{name}","Author":"a","Version":"1.0.0","Description":"","UniqueID":"a.{name}",
                    "EntryDll":"x.dll","Dependencies":[{}]}}"#,
                    deps.join(",")
                ),
            )
            .unwrap();
        }
        crate::library::scan(dir)
    }

    #[test]
    fn halves_down_to_the_culprit_keeping_dependencies_together() {
        let dir = tempfile::tempdir().unwrap();
        let packages = store(
            dir.path(),
            &[
                ("A", &[]),
                ("B", &["Core"]),
                ("C", &[]),
                ("Core", &[]),
                ("D", &[]),
                ("E", &[]),
            ],
        );
        let all: Vec<String> = packages.iter().map(|p| p.id.clone()).collect();
        for culprit in &all {
            let mut b = Bisect::start("default".into(), "bisect".into(), &all, &packages);
            let mut found = None;
            for _ in 0..10 {
                let on = b.enabled();
                if on.iter().any(|p| p == "B") {
                    assert!(
                        on.iter().any(|p| p == "Core"),
                        "B never runs without what it needs"
                    );
                }
                // the problem shows whenever the culprit actually runs
                let runs =
                    on.contains(culprit) && (culprit != "B" || on.iter().any(|p| p == "Core"));
                match b.answer(runs, &packages) {
                    Step::Found { package, .. } => {
                        found = Some(package);
                        break;
                    }
                    Step::Clean => break,
                    Step::Next { .. } => {}
                }
            }
            assert_eq!(found.as_ref(), Some(culprit));
            assert!(b.round <= 4, "{} rounds for 6 mods", b.round);
        }
    }

    #[test]
    fn clears_mods_that_ran_without_the_problem() {
        let dir = tempfile::tempdir().unwrap();
        let packages = store(dir.path(), &[("A", &[]), ("B", &[]), ("C", &[])]);
        let all: Vec<String> = packages.iter().map(|p| p.id.clone()).collect();
        let mut b = Bisect::start("default".into(), "bisect".into(), &all, &packages);
        assert_eq!(b.testing, ["A", "B"]);
        // the problem came with every mod on, so without A and B it's C
        assert_eq!(
            b.answer(false, &packages),
            Step::Found {
                package: "C".into(),
                with: None,
                cleared: vec!["A".into(), "B".into()],
            }
        );
        assert_eq!(b.cleared, ["A", "B"]);
    }

    #[test]
    fn finds_the_partner_of_a_mod_that_only_breaks_with_another() {
        let dir = tempfile::tempdir().unwrap();
        let packages = store(
            dir.path(),
            &[("A", &[]), ("B", &[]), ("C", &[]), ("D", &[]), ("E", &[])],
        );
        let all: Vec<String> = packages.iter().map(|p| p.id.clone()).collect();
        let broken = |on: &[String]| on.iter().any(|p| p == "B") && on.iter().any(|p| p == "D");
        let mut b = Bisect::start("default".into(), "bisect".into(), &all, &packages);
        let (found, cleared) = loop {
            if let Step::Found {
                package, cleared, ..
            } = b.answer(broken(&b.enabled()), &packages)
            {
                break (package, cleared);
            }
        };
        let mut p = Bisect::partner("default".into(), "bisect".into(), found.clone(), &cleared);
        assert_eq!(
            p.enabled(),
            std::slice::from_ref(&found),
            "the first round runs it alone"
        );
        let partner = loop {
            match p.answer(broken(&p.enabled()), &packages) {
                Step::Found { with, .. } => break with,
                Step::Clean => break None,
                Step::Next { .. } => {}
            }
        };
        let mut pair = [found, partner.unwrap()];
        pair.sort();
        assert_eq!(pair, ["B", "D"]);
    }
}
