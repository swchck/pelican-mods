//! Mod settings that belong to a profile rather than to the mods.
//!
//! Mods keep `config.json` in their own folder, which every profile shares. A profile
//! with settings of its own gets a copy of every mod's `config.json` when the player
//! leaves it, and those copies go back into the mods when the player returns.

use std::path::{Component, Path};

use crate::error::AppResult;
use crate::library::Package;

const CONFIG: &str = "config.json";

/// Copies every mod's `config.json` in `packages` into `dir`, replacing what was there;
/// returns how many.
pub fn snapshot(packages: &[Package], dir: &Path) -> AppResult<u32> {
    let staging = dir.with_extension("new");
    let _ = std::fs::remove_dir_all(&staging);
    let mut n = 0;
    for p in packages {
        for m in &p.mods {
            let from = p.path.join(&m.rel_path).join(CONFIG);
            if !from.is_file() {
                continue;
            }
            let to = staging.join(&p.id).join(&m.rel_path).join(CONFIG);
            if let Some(parent) = to.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&from, &to)?;
            n += 1;
        }
    }
    // swapped in whole, so a failure halfway leaves the last good copy
    let _ = std::fs::remove_dir_all(dir);
    if staging.exists() {
        std::fs::rename(&staging, dir)?;
    }
    Ok(n)
}

/// Puts the copies in `dir` back into the mods of `store` that still exist; returns how many.
pub fn restore(store: &Path, dir: &Path) -> AppResult<u32> {
    let mut n = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let path = e.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let Ok(rel) = path.strip_prefix(dir) else {
                continue;
            };
            let safe = rel.components().all(|c| matches!(c, Component::Normal(_)));
            if !safe || rel.file_name().is_none_or(|f| f != CONFIG) {
                continue;
            }
            let target = store.join(rel);
            // a mod gone since, or moved inside its package: nothing to put it into
            if target.parent().is_some_and(Path::is_dir) {
                std::fs::copy(&path, &target)?;
                n += 1;
            }
        }
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_and_brings_back_each_mods_settings() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("mods");
        for name in ["Automate", "Gone"] {
            let m = store.join(name);
            std::fs::create_dir_all(&m).unwrap();
            std::fs::write(
                m.join("manifest.json"),
                format!(r#"{{"Name":"{name}","Author":"a","Version":"1.0.0","Description":"","UniqueID":"a.{name}","EntryDll":"x.dll"}}"#),
            )
            .unwrap();
            std::fs::write(m.join(CONFIG), r#"{"Speed": 1}"#).unwrap();
        }
        let kept = dir.path().join("profile-configs/p1");
        assert_eq!(snapshot(&crate::library::scan(&store), &kept).unwrap(), 2);

        std::fs::write(store.join("Automate").join(CONFIG), r#"{"Speed": 9}"#).unwrap();
        std::fs::remove_dir_all(store.join("Gone")).unwrap();
        assert_eq!(restore(&store, &kept).unwrap(), 1, "only mods still there");
        assert_eq!(
            std::fs::read_to_string(store.join("Automate").join(CONFIG)).unwrap(),
            r#"{"Speed": 1}"#
        );
    }
}
