//! What the library went through: installs, updates and removals, newest first, with
//! what it takes to undo each.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::error::AppResult;

/// Entries kept; older ones fall off, and so do their stashed removals.
const KEEP: usize = 200;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    Installed,
    Updated,
    Removed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    /// Unix milliseconds; also the entry's id.
    pub at: u64,
    pub kind: Kind,
    /// Package folder name.
    pub package: String,
    pub name: String,
    pub unique_id: String,
    pub version: String,
    /// The version an update replaced.
    #[serde(default)]
    pub previous: Option<String>,
    /// Where a removed package waits to be put back.
    #[serde(default)]
    pub stash: Option<PathBuf>,
    /// Whether the removed package was enabled in the active profile.
    #[serde(default)]
    pub was_enabled: bool,
    #[serde(default)]
    pub undone: bool,
}

/// The journal, saved as JSON after every change.
pub struct History {
    file: PathBuf,
    entries: Mutex<Vec<Entry>>,
}

impl History {
    pub fn load(file: PathBuf) -> Self {
        let entries = std::fs::read(&file)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Self {
            file,
            entries: Mutex::new(entries),
        }
    }

    /// Newest first.
    pub fn list(&self) -> Vec<Entry> {
        let mut out = self.entries.lock().unwrap().clone();
        out.reverse();
        out
    }

    pub fn get(&self, at: u64) -> Option<Entry> {
        self.entries
            .lock()
            .unwrap()
            .iter()
            .find(|e| e.at == at)
            .cloned()
    }

    /// Adds `entry`, dropping the oldest past [`KEEP`] along with their stashes.
    pub fn push(&self, entry: Entry) -> AppResult<()> {
        let mut entries = self.entries.lock().unwrap();
        entries.push(entry);
        let excess = entries.len().saturating_sub(KEEP);
        for old in entries.drain(..excess) {
            if let Some(stash) = old.stash {
                let _ = std::fs::remove_dir_all(stash);
            }
        }
        self.save(&entries)
    }

    /// Forgets stashed removals whose folder is gone, as after the stash folder was
    /// emptied; those can't be undone any more.
    pub fn forget_stashes(&self) -> AppResult<()> {
        let mut entries = self.entries.lock().unwrap();
        for e in entries.iter_mut() {
            if e.stash.as_ref().is_some_and(|s| !s.exists()) {
                e.stash = None;
            }
        }
        self.save(&entries)
    }

    pub fn mark_undone(&self, at: u64) -> AppResult<()> {
        let mut entries = self.entries.lock().unwrap();
        if let Some(e) = entries.iter_mut().find(|e| e.at == at) {
            e.undone = true;
            e.stash = None;
        }
        self.save(&entries)
    }

    fn save(&self, entries: &[Entry]) -> AppResult<()> {
        if let Some(dir) = self.file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(
            &self.file,
            serde_json::to_vec(entries).expect("history serialize"),
        )?;
        Ok(())
    }
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Moves package folder `dir` into `stash_root` instead of deleting it; returns where.
pub fn stash(dir: &Path, stash_root: &Path, at: u64) -> AppResult<PathBuf> {
    std::fs::create_dir_all(stash_root)?;
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let dest = stash_root.join(format!("{at}-{name}"));
    std::fs::rename(dir, &dest)?;
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(at: u64, stash: Option<PathBuf>) -> Entry {
        Entry {
            at,
            kind: Kind::Removed,
            package: "Automate".into(),
            name: "Automate".into(),
            unique_id: "Pathoschild.Automate".into(),
            version: "2.0.0".into(),
            previous: None,
            stash,
            was_enabled: true,
            undone: false,
        }
    }

    #[test]
    fn keeps_a_bounded_journal() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("history.json");
        let h = History::load(file.clone());
        let old_stash = dir.path().join("removed/1-Automate");
        std::fs::create_dir_all(&old_stash).unwrap();
        h.push(entry(1, Some(old_stash.clone()))).unwrap();
        for at in 2..=(KEEP as u64 + 1) {
            h.push(entry(at, None)).unwrap();
        }
        assert_eq!(h.list().len(), KEEP);
        assert_eq!(h.list()[0].at, KEEP as u64 + 1);
        assert!(
            !old_stash.exists(),
            "the dropped entry's stash goes with it"
        );
        h.mark_undone(5).unwrap();
        assert!(History::load(file).get(5).unwrap().undone);
    }

    #[test]
    fn stashes_a_package() {
        let dir = tempfile::tempdir().unwrap();
        let pkg = dir.path().join("mods/Automate");
        std::fs::create_dir_all(&pkg).unwrap();
        let to = stash(&pkg, &dir.path().join("removed"), 7).unwrap();
        assert!(!pkg.exists());
        assert!(to.ends_with("7-Automate"));
    }
}
