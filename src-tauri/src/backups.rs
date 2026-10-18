//! Copies of packages that an install replaced, kept for rollback.
//!
//! Layout: `<root>/<uniqueid>/<unix ms>@<version>/<package folder>`. Backups are keyed by
//! the package's first UniqueID because the folder name may change between versions.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::error::{AppError, AppResult};
use crate::library::Package;

/// How many old versions to keep per mod.
pub const KEEP: usize = 3;

/// One stored version of a package.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Backup {
    pub version: String,
    /// Unix milliseconds when it was replaced.
    pub created: u64,
    /// The package folder inside the backup.
    pub path: PathBuf,
}

/// Moves `pkg` out of the store into the backup area, pruning old copies.
pub fn stash(root: &Path, pkg: &Package) -> AppResult<()> {
    let version = pkg
        .mods
        .first()
        .map(|m| m.manifest.version.clone())
        .unwrap_or_default();
    let created = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let dir = root
        .join(key(pkg))
        .join(format!("{created}@{}", clean(&version)));
    std::fs::create_dir_all(&dir)?;
    std::fs::rename(&pkg.path, dir.join(&pkg.id))?;
    for old in list_key(root, &key(pkg)).into_iter().skip(KEEP) {
        if let Some(entry) = old.path.parent() {
            let _ = std::fs::remove_dir_all(entry);
        }
    }
    Ok(())
}

/// Lists backups of the mod with `unique_id`, newest first.
pub fn list(root: &Path, unique_id: &str) -> Vec<Backup> {
    list_key(root, &clean(&unique_id.to_lowercase()))
}

/// Deletes one backup entry after it was restored.
pub fn discard(root: &Path, backup: &Backup) -> AppResult<()> {
    let entry = backup
        .path
        .parent()
        .ok_or_else(|| AppError::NotFound(backup.path.display().to_string()))?;
    if !entry.starts_with(root) {
        return Err(AppError::NotFound(entry.display().to_string()));
    }
    std::fs::remove_dir_all(entry)?;
    Ok(())
}

fn key(pkg: &Package) -> String {
    let id = pkg
        .mods
        .first()
        .map(|m| m.manifest.unique_id.to_lowercase())
        .unwrap_or_else(|| pkg.id.to_lowercase());
    clean(&id)
}

fn clean(s: &str) -> String {
    s.chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':' | '@') {
                '_'
            } else {
                c
            }
        })
        .collect::<String>()
        .trim_start_matches('.')
        .to_owned()
}

fn list_key(root: &Path, key: &str) -> Vec<Backup> {
    let mut out: Vec<Backup> = std::fs::read_dir(root.join(key))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let (created, version) = name.split_once('@')?;
            let created = created.parse().ok()?;
            let path = std::fs::read_dir(entry.path())
                .ok()?
                .flatten()
                .map(|e| e.path())
                .find(|p| p.is_dir())?;
            Some(Backup {
                version: version.to_owned(),
                created,
                path,
            })
        })
        .collect();
    out.sort_by_key(|b| std::cmp::Reverse(b.created));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library;

    fn package(store: &Path, version: &str) -> Package {
        let dir = store.join("Lookup");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("manifest.json"),
            format!(r#"{{"Name":"Lookup","UniqueID":"P.Lookup","Version":"{version}"}}"#),
        )
        .unwrap();
        library::read_package("Lookup".into(), dir).unwrap()
    }

    #[test]
    fn stashes_lists_and_prunes() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("store");
        let root = dir.path().join("backups");
        for v in ["1.0", "1.1", "1.2", "1.3"] {
            let pkg = package(&store, v);
            stash(&root, &pkg).unwrap();
            assert!(!store.join("Lookup").exists());
            std::thread::sleep(std::time::Duration::from_millis(3));
        }
        let all = list(&root, "P.Lookup");
        let versions: Vec<&str> = all.iter().map(|b| b.version.as_str()).collect();
        assert_eq!(versions, ["1.3", "1.2", "1.1"]);
        assert!(all[0].path.join("manifest.json").is_file());

        discard(&root, &all[0]).unwrap();
        assert_eq!(list(&root, "p.lookup").len(), 2);
    }
}
