//! Zip snapshots of save folders, plus export and import.
//!
//! Layout: `<root>/<save id>/<unix ms>@<reason>.zip`, each zip holding the save folder
//! under its own name, which is also the format export writes and import reads.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::archive;
use crate::error::{AppError, AppResult};
use crate::saves;

/// Snapshots kept per save and reason; the oldest go first. Per reason, so a run of
/// launches can't push out the backups the player made or a restore left behind.
pub const KEEP: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Reason {
    Manual,
    /// Taken automatically before the game starts.
    Launch,
    /// The state that a restore or import replaced.
    Replaced,
    /// Copied to the user's own backup folder after a session.
    Mirror,
}

impl Reason {
    fn tag(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Launch => "launch",
            Self::Replaced => "replaced",
            Self::Mirror => "mirror",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "manual" => Self::Manual,
            "launch" => Self::Launch,
            "replaced" => Self::Replaced,
            "mirror" => Self::Mirror,
            _ => return None,
        })
    }
}

/// One snapshot.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub save_id: String,
    /// Unix milliseconds.
    pub created: u64,
    pub reason: Reason,
    pub size: u64,
    pub file: PathBuf,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn save_id(dir: &Path) -> AppResult<String> {
    dir.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .ok_or_else(|| AppError::NotFound(dir.display().to_string()))
}

/// Zips the save folder `dir` into the backup area.
pub fn create(root: &Path, dir: &Path, reason: Reason) -> AppResult<Snapshot> {
    let id = save_id(dir)?;
    let folder = root.join(&id);
    std::fs::create_dir_all(&folder)?;
    let created = now_ms();
    let file = folder.join(format!("{created}@{}.zip", reason.tag()));
    archive::zip_dir(dir, &file, &id)?;
    for old in list(root, &id)
        .into_iter()
        .filter(|s| s.reason == reason)
        .skip(KEEP)
    {
        let _ = std::fs::remove_file(old.file);
    }
    Ok(Snapshot {
        size: std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0),
        save_id: id,
        created,
        reason,
        file,
    })
}

/// Lists snapshots of save `id`, newest first.
pub fn list(root: &Path, id: &str) -> Vec<Snapshot> {
    let mut out: Vec<Snapshot> = std::fs::read_dir(root.join(id))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let stem = name.strip_suffix(".zip")?;
            let (created, reason) = stem.split_once('@')?;
            Some(Snapshot {
                save_id: id.to_owned(),
                created: created.parse().ok()?,
                reason: Reason::parse(reason)?,
                size: e.metadata().map(|m| m.len()).unwrap_or(0),
                file: e.path(),
            })
        })
        .collect();
    out.sort_by_key(|s| std::cmp::Reverse(s.created));
    out
}

/// Backs up every save whose files changed since its newest snapshot; returns how many.
pub fn snapshot_changed(root: &Path, saves_dir: &Path) -> AppResult<usize> {
    let mut n = 0;
    for s in saves::list(saves_dir) {
        let newest = list(root, &s.id).first().map(|b| b.created / 1000);
        if newest.is_none_or(|t| s.modified > t) {
            create(root, &s.path, Reason::Launch)?;
            n += 1;
        }
    }
    Ok(n)
}

/// Copies every save changed since its newest copy in `folder` there, keeping the
/// `keep` newest per save; returns how many it copied.
///
/// `folder` is the user's (iCloud Drive, a backup disk), so its layout matches the
/// app's own snapshots and any copy imports back as a save.
pub fn mirror(folder: &Path, saves_dir: &Path, keep: usize) -> AppResult<usize> {
    let mut n = 0;
    for s in saves::list(saves_dir) {
        let newest = list(folder, &s.id).first().map(|b| b.created / 1000);
        if newest.is_none_or(|t| s.modified > t) {
            create(folder, &s.path, Reason::Mirror)?;
            n += 1;
        }
        for old in list(folder, &s.id).into_iter().skip(keep) {
            std::fs::remove_file(&old.file)?;
        }
    }
    Ok(n)
}

/// Writes save `dir` to a zip at `dest` for sharing or moving to another machine.
pub fn export(dir: &Path, dest: &Path) -> AppResult<()> {
    archive::zip_dir(dir, dest, &save_id(dir)?)
}

/// Replaces save folder contents with the snapshot at `file`; the current state is
/// snapshotted first. Returns the save id.
pub fn restore(root: &Path, saves_dir: &Path, file: &Path) -> AppResult<String> {
    // canonical: a lexical starts_with lets root/../anything.zip through
    let inside = match (file.canonicalize(), root.canonicalize()) {
        (Ok(f), Ok(r)) => f.starts_with(r),
        _ => false,
    };
    if !inside {
        return Err(AppError::NotFound(file.display().to_string()));
    }
    import(root, saves_dir, file)
}

/// Brings a save from a zip (exported here, or a plain zipped save folder) into
/// `saves_dir`. A save with the same id is snapshotted, then replaced. Returns the id.
pub fn import(root: &Path, saves_dir: &Path, file: &Path) -> AppResult<String> {
    std::fs::create_dir_all(saves_dir)?;
    // unpack next to the target so the final move is a rename on the same volume
    let staging = saves_dir.join(format!(".import-{}", now_ms()));
    let result = (|| {
        archive::extract(file, &staging)?;
        let found = find_save(&staging, 0).ok_or_else(|| {
            AppError::BadSave("no SaveGameInfo with a matching save file in the archive".into())
        })?;
        let id = save_id(&found)?;
        saves::summary(&found)?;
        let target = saves_dir.join(&id);
        // the old save moves aside and only goes once the new one is in place
        let aside = saves_dir.join(format!(".replaced-{}", now_ms()));
        if target.exists() {
            create(root, &target, Reason::Replaced)?;
            std::fs::rename(&target, &aside)?;
        }
        if let Err(e) = std::fs::rename(&found, &target) {
            if aside.exists() {
                let _ = std::fs::rename(&aside, &target);
            }
            return Err(e.into());
        }
        let _ = std::fs::remove_dir_all(&aside);
        Ok(id)
    })();
    let _ = std::fs::remove_dir_all(&staging);
    result
}

// a save folder is one holding both SaveGameInfo and a file named like the folder
fn find_save(dir: &Path, depth: usize) -> Option<PathBuf> {
    let name = dir.file_name()?.to_string_lossy().into_owned();
    if dir.join(saves::INFO_FILE).is_file() && dir.join(&name).is_file() {
        return Some(dir.to_path_buf());
    }
    if depth >= 3 {
        return None;
    }
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|e| e.path().is_dir() && !e.file_name().to_string_lossy().starts_with("__MACOSX"))
        .find_map(|e| find_save(&e.path(), depth + 1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::saves::tests::write_save;

    #[test]
    fn snapshots_and_prunes() {
        let tmp = tempfile::tempdir().unwrap();
        let saves = tmp.path().join("Saves");
        let root = tmp.path().join("backups");
        let dir = write_save(&saves, "Anya_1");
        for _ in 0..KEEP + 2 {
            create(&root, &dir, Reason::Manual).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        let all = list(&root, "Anya_1");
        assert_eq!(all.len(), KEEP);
        assert!(all.windows(2).all(|w| w[0].created > w[1].created));
    }

    #[test]
    fn snapshots_only_changed_saves() {
        let tmp = tempfile::tempdir().unwrap();
        let saves = tmp.path().join("Saves");
        let root = tmp.path().join("backups");
        write_save(&saves, "Anya_1");
        assert_eq!(snapshot_changed(&root, &saves).unwrap(), 1);
        assert_eq!(snapshot_changed(&root, &saves).unwrap(), 0);
        assert_eq!(list(&root, "Anya_1")[0].reason, Reason::Launch);
    }

    #[test]
    fn restore_replaces_and_keeps_the_replaced_state() {
        let tmp = tempfile::tempdir().unwrap();
        let saves = tmp.path().join("Saves");
        let root = tmp.path().join("backups");
        let dir = write_save(&saves, "Anya_1");
        let snap = create(&root, &dir, Reason::Manual).unwrap();
        std::fs::write(dir.join("Anya_1"), "<SaveGame>later</SaveGame>").unwrap();

        assert_eq!(restore(&root, &saves, &snap.file).unwrap(), "Anya_1");
        let world = std::fs::read_to_string(dir.join("Anya_1")).unwrap();
        assert!(world.contains("<player>"));
        assert_eq!(list(&root, "Anya_1")[0].reason, Reason::Replaced);
        assert!(!std::fs::read_dir(&saves).unwrap().any(|e| {
            e.unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".import")
        }));
        assert!(restore(&root, &saves, &tmp.path().join("elsewhere.zip")).is_err());
    }

    #[test]
    fn imports_nested_zip_and_rejects_non_saves() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        let dir = write_save(&src.join("wrapper"), "Farm_9");
        let zip = tmp.path().join("farm.zip");
        archive::zip_dir(dir.parent().unwrap(), &zip, "wrapper").unwrap();
        let saves = tmp.path().join("Saves");
        assert_eq!(
            import(&tmp.path().join("b"), &saves, &zip).unwrap(),
            "Farm_9"
        );
        assert!(saves.join("Farm_9/SaveGameInfo").is_file());

        let junk = tmp.path().join("junk.zip");
        crate::archive::tests::make_zip(&junk, &[("readme.txt", "hi")]);
        assert!(import(&tmp.path().join("b"), &saves, &junk).is_err());
    }

    #[test]
    fn mirrors_changed_saves_and_prunes() {
        let tmp = tempfile::tempdir().unwrap();
        let saves = tmp.path().join("Saves");
        write_save(&saves, "Farm_1");
        let folder = tmp.path().join("iCloud");
        std::fs::create_dir_all(&folder).unwrap();
        assert_eq!(mirror(&folder, &saves, 2).unwrap(), 1);
        // nothing changed since: no new copy
        assert_eq!(mirror(&folder, &saves, 2).unwrap(), 0);
        for _ in 0..3 {
            create(&folder, &saves.join("Farm_1"), Reason::Mirror).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        mirror(&folder, &saves, 2).unwrap();
        assert_eq!(list(&folder, "Farm_1").len(), 2);
    }
}
