//! A farm's money and progress over in-game time, for the history chart.
//!
//! Points come from every look at a save: the save list notes each save's date and money,
//! a perfection check adds its score, and the snapshots fill in days played before the
//! launcher kept a history. One file per save, one point per in-game day.

use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::AppResult;
use crate::saves::SaveSummary;

// a save's info file is a few KB; anything bigger in a snapshot isn't one
const INFO_MAX: u64 = 1 << 20;

/// The farm on one in-game day.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Point {
    pub year: u32,
    /// 0 spring … 3 winter.
    pub season: u8,
    pub day: u8,
    pub money: u64,
    pub total_earned: u64,
    pub play_time_ms: u64,
    /// The five skill levels summed, 0–50.
    pub skills: u32,
    /// 0–100; known for days a perfection check ran on.
    pub perfection: Option<f32>,
}

impl Point {
    pub fn of(s: &SaveSummary) -> Self {
        let k = &s.skills;
        Self {
            year: s.year,
            season: s.season,
            day: s.day,
            money: s.money,
            total_earned: s.total_earned,
            play_time_ms: s.play_time_ms,
            skills: [k.farming, k.mining, k.foraging, k.fishing, k.combat]
                .iter()
                .map(|&l| u32::from(l))
                .sum(),
            perfection: None,
        }
    }

    fn date(&self) -> (u32, u8, u8) {
        (self.year, self.season, self.day)
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Store {
    points: Vec<Point>,
    /// Snapshot file names already read.
    #[serde(default)]
    seen: Vec<String>,
}

fn file(root: &Path, id: &str) -> PathBuf {
    root.join(format!("{id}.json"))
}

fn load(root: &Path, id: &str) -> Store {
    std::fs::read(file(root, id))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn store(root: &Path, id: &str, s: &Store) -> AppResult<()> {
    std::fs::create_dir_all(root)?;
    let tmp = file(root, id).with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_vec(s).expect("history serialize"))?;
    std::fs::rename(tmp, file(root, id))?;
    Ok(())
}

// a later look at the same day wins, but a score from an earlier one stays
fn upsert(points: &mut Vec<Point>, mut p: Point) -> bool {
    match points.binary_search_by_key(&p.date(), Point::date) {
        Ok(i) => {
            p.perfection = p.perfection.or(points[i].perfection);
            let changed = points[i] != p;
            points[i] = p;
            changed
        }
        Err(i) => {
            points.insert(i, p);
            true
        }
    }
}

/// Notes `p` as the state of save `id` on its day.
pub fn record(root: &Path, id: &str, p: Point) -> AppResult<()> {
    let mut s = load(root, id);
    if upsert(&mut s.points, p) {
        store(root, id, &s)?;
    }
    Ok(())
}

/// The history of save `id`, oldest day first, after reading any of `snapshots` not read
/// before; `scratch` is a folder it may use and leaves empty.
pub fn history(
    root: &Path,
    id: &str,
    snapshots: &[PathBuf],
    scratch: &Path,
) -> AppResult<Vec<Point>> {
    let mut s = load(root, id);
    let mut changed = false;
    for zip in snapshots {
        let Some(name) = zip.file_name().map(|n| n.to_string_lossy().into_owned()) else {
            continue;
        };
        if s.seen.contains(&name) {
            continue;
        }
        // a snapshot that can't be read now won't be readable later either
        if let Ok(p) = snapshot_point(zip, id, scratch) {
            upsert(&mut s.points, p);
        }
        s.seen.push(name);
        changed = true;
    }
    let _ = std::fs::remove_dir_all(scratch);
    if changed {
        store(root, id, &s)?;
    }
    Ok(s.points)
}

// only the small info file comes out; the save itself can run to tens of MB
fn snapshot_point(zip: &Path, id: &str, scratch: &Path) -> AppResult<Point> {
    let mut archive = zip::ZipArchive::new(std::fs::File::open(zip)?)?;
    let mut entry = archive.by_name(&format!("{id}/{}", crate::saves::INFO_FILE))?;
    let mut info = Vec::new();
    (&mut entry).take(INFO_MAX).read_to_end(&mut info)?;
    let dir = scratch.join(id);
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join(crate::saves::INFO_FILE), info)?;
    Ok(Point::of(&crate::saves::summary(&dir)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(year: u32, season: u8, day: u8, money: u64) -> Point {
        Point {
            year,
            season,
            day,
            money,
            total_earned: money,
            play_time_ms: 0,
            skills: 0,
            perfection: None,
        }
    }

    #[test]
    fn keeps_one_point_per_day_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("history");
        record(&root, "a_1", point(1, 1, 3, 300)).unwrap();
        record(&root, "a_1", point(1, 0, 9, 90)).unwrap();
        record(
            &root,
            "a_1",
            Point {
                perfection: Some(4.5),
                ..point(1, 1, 3, 300)
            },
        )
        .unwrap();
        record(&root, "a_1", point(1, 1, 3, 350)).unwrap();
        let h = history(&root, "a_1", &[], &dir.path().join("scratch")).unwrap();
        assert_eq!(h.iter().map(|p| p.money).collect::<Vec<_>>(), [90, 350]);
        assert_eq!(
            h[1].perfection,
            Some(4.5),
            "a score outlives a later look without one"
        );
    }

    #[test]
    fn reads_snapshots_once() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("history");
        let save = dir.path().join("a_1");
        std::fs::create_dir_all(&save).unwrap();
        std::fs::write(
            save.join(crate::saves::INFO_FILE),
            "<Farmer><name>A</name><money>500</money><yearForSaveGame>1</yearForSaveGame>\
             <seasonForSaveGame>2</seasonForSaveGame><dayOfMonthForSaveGame>7</dayOfMonthForSaveGame></Farmer>",
        )
        .unwrap();
        std::fs::write(save.join("a_1"), "<SaveGame/>").unwrap();
        let zip = dir.path().join("1@launch.zip");
        crate::archive::zip_dir(&save, &zip, "a_1").unwrap();

        let scratch = dir.path().join("scratch");
        let h = history(&root, "a_1", std::slice::from_ref(&zip), &scratch).unwrap();
        assert_eq!((h.len(), h[0].season, h[0].day, h[0].money), (1, 2, 7, 500));
        assert!(!scratch.exists());
        std::fs::remove_file(&zip).unwrap();
        assert_eq!(history(&root, "a_1", &[zip], &scratch).unwrap().len(), 1);
    }
}
