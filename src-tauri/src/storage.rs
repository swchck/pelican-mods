//! What the launcher keeps on disk, and how to trim it.
//!
//! Mod version backups and save backups share a layout, a folder per mod or save with
//! entries named from their Unix-millisecond creation time, so one pruning rule fits both.

use std::path::Path;

use serde::Serialize;

use crate::error::AppResult;

/// How much one kind of kept data takes.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    /// "modBackups", "removed", "saveBackups" or "downloads".
    pub kind: &'static str,
    pub bytes: u64,
    /// Files inside, which is what scanning costs.
    pub files: u32,
}

/// Measures `dir`; a missing folder takes nothing.
pub fn measure(kind: &'static str, dir: &Path) -> Usage {
    let (mut bytes, mut files) = (0, 0);
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            // symlink_metadata: a profile's links must not count the mods they point at
            let Ok(meta) = e.path().symlink_metadata() else {
                continue;
            };
            if meta.is_dir() {
                stack.push(e.path());
            } else {
                bytes += meta.len();
                files += 1;
            }
        }
    }
    Usage { kind, bytes, files }
}

/// Keeps the `keep` newest entries in each folder under `root`; returns how many went.
pub fn prune(root: &Path, keep: usize) -> AppResult<u32> {
    let mut removed = 0;
    for group in std::fs::read_dir(root).into_iter().flatten().flatten() {
        if !group.path().is_dir() {
            continue;
        }
        let mut entries: Vec<(u64, std::path::PathBuf)> = std::fs::read_dir(group.path())?
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                let stamp = name.split(['@', '-']).next()?.parse().ok()?;
                Some((stamp, e.path()))
            })
            .collect();
        entries.sort_by_key(|(stamp, _)| std::cmp::Reverse(*stamp));
        for (_, path) in entries.into_iter().skip(keep) {
            if path.is_dir() {
                std::fs::remove_dir_all(&path)?;
            } else {
                std::fs::remove_file(&path)?;
            }
            removed += 1;
        }
    }
    Ok(removed)
}

/// Empties `dir`, keeping the folder itself.
pub fn clear(dir: &Path) -> AppResult<u32> {
    let mut removed = 0;
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = e.path();
        if path.is_dir() {
            std::fs::remove_dir_all(&path)?;
        } else {
            std::fs::remove_file(&path)?;
        }
        removed += 1;
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measures_prunes_and_clears() {
        let dir = tempfile::tempdir().unwrap();
        let saves = dir.path().join("save-backups/Farm_1");
        std::fs::create_dir_all(&saves).unwrap();
        for t in [100, 300, 200] {
            std::fs::write(saves.join(format!("{t}@launch.zip")), [0u8; 10]).unwrap();
        }
        let mods = dir.path().join("backups/pathoschild.automate");
        for t in [5, 9] {
            std::fs::create_dir_all(mods.join(format!("{t}@2.0.{t}/Automate"))).unwrap();
            std::fs::write(mods.join(format!("{t}@2.0.{t}/Automate/a.dll")), [0u8; 4]).unwrap();
        }
        let u = measure("saveBackups", &dir.path().join("save-backups"));
        assert_eq!((u.bytes, u.files), (30, 3));

        assert_eq!(prune(&dir.path().join("save-backups"), 1).unwrap(), 2);
        assert!(saves.join("300@launch.zip").exists());
        assert_eq!(prune(&dir.path().join("backups"), 1).unwrap(), 1);
        assert!(mods.join("9@2.0.9").exists() && !mods.join("5@2.0.5").exists());

        assert_eq!(clear(&dir.path().join("backups")).unwrap(), 1);
        assert_eq!(measure("x", &dir.path().join("backups")).files, 0);
        assert_eq!(measure("x", &dir.path().join("nowhere")).bytes, 0);
    }
}
