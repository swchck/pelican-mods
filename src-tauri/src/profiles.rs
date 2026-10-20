//! Mod profiles: named sets of enabled packages, deployed as a folder of symlinks.
//!
//! The game never sees the store directly. Before a modded launch the active profile is
//! deployed to `<deploy>/<profile>/Mods`, one symlink per enabled package plus SMAPI's
//! bundled mods from the game folder, and SMAPI starts with `--mods-path` pointing there.
//! Switching profiles therefore copies nothing and leaves the game's own `Mods` alone.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

pub const DEFAULT_ID: &str = "default";

/// A named set of enabled packages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    /// `None` until the user renames it; the UI shows a translated default name.
    pub name: Option<String>,
    /// Package ids, in the order they were enabled.
    pub enabled: Vec<String>,
}

/// All profiles and which one is active.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Profiles {
    pub active: String,
    pub profiles: Vec<Profile>,
    /// The profile each save was last played with, by save id.
    #[serde(default)]
    pub save_profiles: BTreeMap<String, String>,
    /// The player's own labels for packages, by package id, shared by every profile.
    #[serde(default)]
    pub tags: BTreeMap<String, Vec<String>>,
}

/// Tags are chips in a filter row; longer ones are notes, not labels.
const MAX_TAG_CHARS: usize = 32;

impl Default for Profiles {
    fn default() -> Self {
        Self {
            active: DEFAULT_ID.into(),
            profiles: vec![Profile {
                id: DEFAULT_ID.into(),
                name: None,
                enabled: Vec::new(),
            }],
            save_profiles: BTreeMap::new(),
            tags: BTreeMap::new(),
        }
    }
}

impl Profiles {
    /// Labels package `id` with `tags`, trimmed, without repeats in any letter case;
    /// none at all forgets the package.
    pub fn set_tags(&mut self, id: &str, tags: &[String]) {
        let mut kept: Vec<String> = Vec::new();
        for t in tags.iter().map(|t| t.trim()).filter(|t| !t.is_empty()) {
            if !kept.iter().any(|k| k.to_lowercase() == t.to_lowercase()) {
                kept.push(t.chars().take(MAX_TAG_CHARS).collect());
            }
        }
        if kept.is_empty() {
            self.tags.remove(id);
        } else {
            self.tags.insert(id.to_owned(), kept);
        }
    }

    /// Records the active profile as the one `saves` were played with.
    pub fn remember_saves(&mut self, saves: &[String]) {
        for s in saves {
            self.save_profiles.insert(s.clone(), self.active.clone());
        }
    }

    /// Returns the active profile.
    pub fn active(&self) -> &Profile {
        self.profiles
            .iter()
            .find(|p| p.id == self.active)
            .unwrap_or(&self.profiles[0])
    }

    fn get_mut(&mut self, id: &str) -> AppResult<&mut Profile> {
        self.profiles
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| AppError::NotFound(id.to_owned()))
    }

    fn check_name(&self, name: &str, except: Option<&str>) -> AppResult<String> {
        let name = name.trim();
        let taken = self.profiles.iter().any(|p| {
            Some(p.id.as_str()) != except
                && p.name
                    .as_deref()
                    .is_some_and(|n| n.eq_ignore_ascii_case(name))
        });
        if name.is_empty() || taken {
            return Err(AppError::BadProfileName(name.to_owned()));
        }
        Ok(name.to_owned())
    }

    /// Adds a profile, empty or copied from `copy_from`, and returns its id.
    pub fn create(&mut self, name: &str, copy_from: Option<&str>) -> AppResult<String> {
        let name = self.check_name(name, None)?;
        let enabled = match copy_from {
            Some(src) => self.get_mut(src)?.enabled.clone(),
            None => Vec::new(),
        };
        let id = (1..)
            .map(|i| format!("p{i}"))
            .find(|id| self.profiles.iter().all(|p| &p.id != id))
            .expect("some id is free");
        self.profiles.push(Profile {
            id: id.clone(),
            name: Some(name),
            enabled,
        });
        Ok(id)
    }

    pub fn rename(&mut self, id: &str, name: &str) -> AppResult<()> {
        let name = self.check_name(name, Some(id))?;
        self.get_mut(id)?.name = Some(name);
        Ok(())
    }

    pub fn delete(&mut self, id: &str) -> AppResult<()> {
        if self.profiles.len() == 1 {
            return Err(AppError::LastProfile);
        }
        self.get_mut(id)?;
        self.profiles.retain(|p| p.id != id);
        self.save_profiles.retain(|_, p| p != id);
        if self.active == id {
            self.active = self.profiles[0].id.clone();
        }
        Ok(())
    }

    pub fn activate(&mut self, id: &str) -> AppResult<()> {
        self.get_mut(id)?;
        self.active = id.to_owned();
        Ok(())
    }

    /// Replaces the packages enabled in profile `id`.
    pub fn set_profile_enabled(&mut self, id: &str, enabled: Vec<String>) -> AppResult<()> {
        self.get_mut(id)?.enabled = enabled;
        Ok(())
    }

    /// Turns `package` on or off in the active profile.
    pub fn set_enabled(&mut self, package: &str, on: bool) {
        let active = self.active.clone();
        let Ok(p) = self.get_mut(&active) else { return };
        let has = p.enabled.iter().any(|e| e == package);
        if on && !has {
            p.enabled.push(package.to_owned());
        } else if !on {
            p.enabled.retain(|e| e != package);
        }
    }

    /// Points every profile that enabled one of `old` at `new` instead.
    pub fn replace_package(&mut self, old: &[String], new: &str) {
        for p in &mut self.profiles {
            if p.enabled.iter().any(|e| old.contains(e)) {
                p.enabled.retain(|e| !old.contains(e) && e != new);
                p.enabled.push(new.to_owned());
            }
        }
    }

    /// Drops a deleted package from every profile.
    pub fn forget_package(&mut self, id: &str) {
        for p in &mut self.profiles {
            p.enabled.retain(|e| e != id);
        }
    }
}

/// Profiles plus the file they live in. Writes go straight to disk.
pub struct ProfilesStore {
    file: PathBuf,
    current: Mutex<Profiles>,
}

impl ProfilesStore {
    /// Loads profiles from `file`, or a single default profile when it is missing.
    pub fn load(file: PathBuf) -> Self {
        let current = std::fs::read_to_string(&file)
            .ok()
            .and_then(|s| serde_json::from_str::<Profiles>(&s).ok())
            .filter(|p| !p.profiles.is_empty())
            .unwrap_or_default();
        Self {
            file,
            current: Mutex::new(current),
        }
    }

    pub fn get(&self) -> Profiles {
        self.current.lock().unwrap().clone()
    }

    /// Applies `f` and saves the result; nothing is saved when `f` fails.
    pub fn update<T>(&self, f: impl FnOnce(&mut Profiles) -> AppResult<T>) -> AppResult<T> {
        let mut cur = self.current.lock().unwrap();
        let mut next = cur.clone();
        let out = f(&mut next)?;
        if let Some(dir) = self.file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = self.file.with_extension("json.tmp");
        std::fs::write(
            &tmp,
            serde_json::to_vec_pretty(&next).expect("profiles serialize"),
        )?;
        std::fs::rename(tmp, &self.file)?;
        *cur = next;
        Ok(out)
    }
}

/// Rebuilds the symlink folder for `profile` under `deploy_root` and returns the path
/// SMAPI should get as `--mods-path`.
///
/// `bundled` are SMAPI's own mods in the game folder, always linked so SMAPI updates
/// reach every profile.
pub fn deploy(
    profile: &Profile,
    store_root: &Path,
    bundled: &[(String, PathBuf)],
    deploy_root: &Path,
) -> AppResult<PathBuf> {
    let out = deploy_root.join(&profile.id).join("Mods");
    if out.exists() {
        // only symlinks live here; remove_dir_all unlinks them without touching targets
        std::fs::remove_dir_all(&out)?;
    }
    std::fs::create_dir_all(&out)?;
    let links = bundled.iter().cloned().chain(
        profile
            .enabled
            .iter()
            .map(|id| (id.clone(), store_root.join(id))),
    );
    for (name, target) in links {
        let link = out.join(&name);
        if target.is_dir() && !link.exists() {
            std::os::unix::fs::symlink(&target, &link)?;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_packages() {
        let mut p = Profiles::default();
        let tags = ["  SVE ", "sve", "", &"x".repeat(40), "Visual"].map(String::from);
        p.set_tags("Stardew Valley Expanded", &tags);
        assert_eq!(
            p.tags["Stardew Valley Expanded"],
            ["SVE".to_owned(), "x".repeat(32), "Visual".to_owned()]
        );
        p.set_tags("Stardew Valley Expanded", &[]);
        assert!(p.tags.is_empty());
    }

    #[test]
    fn remembers_which_profile_played_a_save() {
        let mut p = Profiles::default();
        let sve = p.create("SVE", None).unwrap();
        p.activate(&sve).unwrap();
        p.remember_saves(&["Farm_1".to_owned()]);
        assert_eq!(p.save_profiles.get("Farm_1"), Some(&sve));
        p.activate(DEFAULT_ID).unwrap();
        p.delete(&sve).unwrap();
        assert!(p.save_profiles.is_empty());
    }

    #[test]
    fn manages_profiles() {
        let mut p = Profiles::default();
        p.set_enabled("Lookup", true);
        p.set_enabled("Lookup", true);
        assert_eq!(p.active().enabled, ["Lookup"]);

        let id = p.create("SVE run", Some(DEFAULT_ID)).unwrap();
        assert!(p.create("sve RUN", None).is_err());
        assert!(p.create("  ", None).is_err());
        p.activate(&id).unwrap();
        p.set_enabled("SVE", true);
        assert_eq!(p.active().enabled, ["Lookup", "SVE"]);

        p.replace_package(&["Lookup".into()], "Lookup Anything");
        assert_eq!(p.profiles[0].enabled, ["Lookup Anything"]);
        assert_eq!(p.active().enabled, ["SVE", "Lookup Anything"]);

        p.forget_package("SVE");
        assert_eq!(p.active().enabled, ["Lookup Anything"]);

        p.delete(&id).unwrap();
        assert_eq!(p.active, DEFAULT_ID);
        assert!(matches!(p.delete(DEFAULT_ID), Err(AppError::LastProfile)));
    }

    #[test]
    fn failed_update_is_not_saved() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("profiles.json");
        let store = ProfilesStore::load(file.clone());
        store.update(|p| p.create("A", None)).unwrap();
        assert!(store.update(|p| p.create("a", None)).is_err());
        let again = ProfilesStore::load(file);
        assert_eq!(again.get().profiles.len(), 2);
    }

    #[test]
    fn deploys_symlinks_and_rebuilds() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("store");
        let game_mods = dir.path().join("game/Mods/ConsoleCommands");
        for d in [
            store.join("Lookup"),
            store.join("Automate"),
            game_mods.clone(),
        ] {
            std::fs::create_dir_all(&d).unwrap();
        }
        let deploy_root = dir.path().join("deploy");
        let mut profile = Profile {
            id: "p1".into(),
            name: None,
            enabled: vec!["Lookup".into(), "Gone".into()],
        };
        let bundled = [("ConsoleCommands".to_owned(), game_mods.clone())];
        let out = deploy(&profile, &store, &bundled, &deploy_root).unwrap();
        let mut names: Vec<String> = std::fs::read_dir(&out)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, ["ConsoleCommands", "Lookup"]);
        assert_eq!(
            std::fs::read_link(out.join("Lookup")).unwrap(),
            store.join("Lookup")
        );

        profile.enabled = vec!["Automate".into()];
        deploy(&profile, &store, &bundled, &deploy_root).unwrap();
        assert!(!out.join("Lookup").exists());
        assert!(out.join("Automate").exists());
        assert!(store.join("Lookup").is_dir(), "targets survive a rebuild");
    }
}
