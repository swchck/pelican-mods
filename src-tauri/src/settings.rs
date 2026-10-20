//! User settings persisted as JSON in the app config folder.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::error::AppResult;

/// Persisted preferences. Missing fields fall back to defaults, so older files keep loading.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Folder holding `Stardew Valley.dll`; `None` until the user picks or confirms one.
    pub game_path: Option<PathBuf>,
    /// Every game folder the user picked, so a second copy (a beta, another store's)
    /// stays one click away after switching.
    #[serde(default)]
    pub game_paths: Vec<PathBuf>,
    /// UI language code; `None` follows the system language.
    pub locale: Option<String>,
    /// Keeps the companion mod out of the game; it's on unless turned off.
    pub companion_disabled: bool,
    /// Keeps the companion's start-of-day lines out of the game's HUD.
    #[serde(default)]
    pub briefing_disabled: bool,
    /// GitHub repos ("owner/repo") users attached to mods, by lowercased UniqueID; they
    /// win over a repo in the manifest.
    pub github_repos: std::collections::BTreeMap<String, String>,
    /// Where item names link: "official", "fandom" or "off"; `None` is the official wiki.
    pub wiki: Option<String>,
    /// A folder of the user's that changed saves are copied to after each session.
    pub backup_folder: Option<PathBuf>,
}

/// Settings plus the file they live in. Writes go straight to disk.
pub struct SettingsStore {
    file: PathBuf,
    current: Mutex<Settings>,
}

impl SettingsStore {
    /// Loads settings from `file`, or defaults when it is missing or unreadable.
    pub fn load(file: PathBuf) -> Self {
        let current = std::fs::read_to_string(&file)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Self {
            file,
            current: Mutex::new(current),
        }
    }

    /// Returns a copy of the current settings.
    pub fn get(&self) -> Settings {
        self.current.lock().unwrap().clone()
    }

    /// Applies `f` to the settings and saves the result.
    pub fn update(&self, f: impl FnOnce(&mut Settings)) -> AppResult<Settings> {
        let mut cur = self.current.lock().unwrap();
        let mut next = cur.clone();
        f(&mut next);
        write_atomic(
            &self.file,
            &serde_json::to_vec_pretty(&next).expect("settings serialize"),
        )?;
        *cur = next.clone();
        Ok(next)
    }
}

fn write_atomic(file: &Path, data: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = file.with_extension("json.tmp");
    std::fs::write(&tmp, data)?;
    std::fs::rename(tmp, file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("nested/settings.json");
        let store = SettingsStore::load(file.clone());
        assert!(store.get().game_path.is_none());
        store
            .update(|s| s.game_path = Some("/game".into()))
            .unwrap();
        let again = SettingsStore::load(file);
        assert_eq!(again.get().game_path, Some(PathBuf::from("/game")));
    }

    #[test]
    fn tolerates_garbage_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("settings.json");
        std::fs::write(&file, b"{not json").unwrap();
        assert!(SettingsStore::load(file).get().locale.is_none());
    }
}
