//! Ginger Island's golden walnuts: which of the game's walnut sources a save has found.
//!
//! The sources are the ones Leo's parrot checks in `IslandHut.ShowNutHint` (1.6): bushes
//! and buried spots by tile, one-off rewards by flag, and drops capped per source. A save
//! keeps found flags in `collectedNutTracker` and capped drops in `limitedNutDrops`.

use std::collections::{HashMap, HashSet};
use std::io::BufRead;

use quick_xml::events::Event;
use serde::Serialize;

use crate::error::{AppError, AppResult};

/// The walnuts perfection counts.
pub const TOTAL: u32 = 130;

// flag sources: (key, where the game hands it out); every one gives its walnuts once
const FLAGS: &[(&str, &str)] = &[
    ("TreeNut", "IslandHut"),
    ("BananaShrine", "IslandEast"),
    ("IslandWestCavePuzzle", "IslandWestCave1"),
    ("SandDuggy", "IslandWest"),
    ("Mermaid", "IslandSouthEast"),
    ("TreeNutShot", "IslandNorth"),
    ("StardropPool", "IslandSouthEast"),
    ("IslandShrinePuzzle", "IslandShrine"),
    ("IslandGourmand1", "IslandWest"),
    ("IslandGourmand2", "IslandWest"),
    ("IslandGourmand3", "IslandWest"),
    ("IslandLeftPlantRestored", "IslandFieldOffice"),
    ("IslandRightPlantRestored", "IslandFieldOffice"),
    ("IslandBatRestored", "IslandFieldOffice"),
    ("IslandFrogRestored", "IslandFieldOffice"),
    ("IslandCenterSkeletonRestored", "IslandFieldOffice"),
    ("IslandSnakeRestored", "IslandFieldOffice"),
];

// capped drops: (key, area, how many the game hands out)
const LIMITED: &[(&str, &str, u32)] = &[
    ("TigerSlimeNut", "IslandWest", 1),
    ("MusselStone", "IslandWest", 5),
    ("IslandFarming", "IslandWest", 5),
    ("IslandFishing", "Island", 5),
    ("VolcanoNormalChest", "Volcano", 1),
    ("VolcanoRareChest", "Volcano", 1),
    ("VolcanoBarrel", "Volcano", 5),
    ("VolcanoMining", "Volcano", 5),
    ("VolcanoMonsterDrop", "Volcano", 5),
    ("Island_N_BuriedTreasureNut", "IslandNorth", 1),
    ("Island_W_BuriedTreasureNut", "IslandWest", 1),
    ("Island_W_BuriedTreasureNut2", "IslandWest", 1),
    ("Darts", "IslandSouthEastCave", 3),
    ("Birdie", "IslandWest", 5),
];

// bushes and buried spots, by "<kind>_<location>_<x>_<y>"
const SPOTS: &[&str] = &[
    "Bush_IslandNorth_13_33",
    "Bush_IslandNorth_5_30",
    "Buried_IslandNorth_19_39",
    "Bush_IslandNorth_4_42",
    "Bush_IslandNorth_45_38",
    "Bush_IslandNorth_47_40",
    "Buried_IslandNorth_19_13",
    "Buried_IslandNorth_57_79",
    "Buried_IslandNorth_54_21",
    "Buried_IslandNorth_42_77",
    "Buried_IslandNorth_62_54",
    "Buried_IslandNorth_26_81",
    "Bush_IslandNorth_20_26",
    "Bush_IslandNorth_9_84",
    "Bush_IslandNorth_56_27",
    "Bush_IslandSouth_31_5",
    "Buried_IslandWest_21_81",
    "Buried_IslandWest_62_76",
    "Buried_IslandWest_39_24",
    "Buried_IslandWest_88_14",
    "Buried_IslandWest_43_74",
    "Buried_IslandWest_30_75",
    "Bush_IslandWest_104_3",
    "Bush_IslandWest_31_24",
    "Bush_IslandWest_38_56",
    "Bush_IslandWest_75_29",
    "Bush_IslandWest_64_30",
    "Bush_IslandWest_54_18",
    "Bush_IslandWest_25_30",
    "Bush_IslandWest_15_3",
    "Buried_IslandSouthEastCave_36_26",
    "Buried_IslandSouthEast_25_17",
    "Bush_Caldera_28_36",
    "Bush_Caldera_9_34",
    "Bush_CaptainRoom_2_4",
    "Bush_IslandEast_17_37",
    "Bush_IslandShrine_23_34",
];

/// One place walnuts come from.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    /// The game's key ("Bush_IslandNorth_13_33", "Darts").
    pub key: String,
    /// "bush", "buried", "flag" or "limited".
    pub kind: &'static str,
    /// The location's internal name ("IslandNorth"), or "Island" for all of it.
    pub area: String,
    pub tile: Option<(i32, i32)>,
    pub have: u32,
    /// Walnuts it gives at most; 1 for a flag, though a flag may stand for a few.
    pub max: u32,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Walnuts {
    /// Walnuts the save counts as found.
    pub found: u32,
    pub total: u32,
    pub sources: Vec<Source>,
}

/// Reads the walnut state from the save XML `reader`.
pub fn read(reader: impl BufRead) -> AppResult<Walnuts> {
    let mut xml = quick_xml::Reader::from_reader(reader);
    let mut buf = Vec::new();
    let mut path: Vec<String> = Vec::new();
    let mut text = String::new();
    let mut collected: HashSet<String> = HashSet::new();
    let mut drops: HashMap<String, u32> = HashMap::new();
    let mut key: Option<String> = None;
    let mut found = 0;
    loop {
        match xml
            .read_event_into(&mut buf)
            .map_err(|e| AppError::BadSave(e.to_string()))?
        {
            Event::Start(e) => {
                path.push(e.local_name().as_ref().to_owned());
                text.clear();
            }
            Event::Text(t) => text.push_str(&t.xml10_content()),
            Event::End(_) => {
                let t = text.trim();
                let n = path.len();
                let parent = |back: usize| n.checked_sub(back).map(|i| path[i].as_str());
                match path.last().map(String::as_str) {
                    Some("string") if parent(2) == Some("collectedNutTracker") => {
                        collected.insert(t.to_owned());
                    }
                    Some("string")
                        if parent(3) == Some("item")
                            && parent(2) == Some("key")
                            && path.iter().any(|p| p == "limitedNutDrops") =>
                    {
                        key = Some(t.to_owned());
                    }
                    Some("int")
                        if parent(2) == Some("value")
                            && path.iter().any(|p| p == "limitedNutDrops") =>
                    {
                        if let Some(k) = key.take() {
                            drops.insert(k, t.parse().unwrap_or(0));
                        }
                    }
                    Some("goldenWalnutsFound") => found = t.parse().unwrap_or(0),
                    _ => {}
                }
                path.pop();
                text.clear();
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    let mut sources: Vec<Source> = SPOTS
        .iter()
        .map(|key| {
            let mut parts = key.split('_');
            let kind = if parts.next() == Some("Buried") {
                "buried"
            } else {
                "bush"
            };
            let area = parts.next().unwrap_or_default().to_owned();
            let x = parts.next().and_then(|v| v.parse().ok());
            let y = parts.next().and_then(|v| v.parse().ok());
            Source {
                key: (*key).to_owned(),
                kind,
                area,
                tile: x.zip(y),
                have: collected.contains(*key) as u32,
                max: 1,
            }
        })
        .collect();
    sources.extend(FLAGS.iter().map(|(key, area)| Source {
        key: (*key).to_owned(),
        kind: "flag",
        area: (*area).to_owned(),
        tile: None,
        have: collected.contains(*key) as u32,
        max: 1,
    }));
    sources.extend(LIMITED.iter().map(|(key, area, max)| Source {
        key: (*key).to_owned(),
        kind: "limited",
        area: (*area).to_owned(),
        tile: None,
        have: drops.get(*key).copied().unwrap_or(0).min(*max),
        max: *max,
    }));
    Ok(Walnuts {
        found,
        total: TOTAL,
        sources,
    })
}

/// Reads the walnut state of save folder `dir`.
pub fn in_save(dir: &std::path::Path) -> AppResult<Walnuts> {
    let id = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    read(std::io::BufReader::with_capacity(
        1 << 16,
        std::fs::File::open(dir.join(id))?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_found_and_capped_walnuts() {
        let save = r#"<SaveGame><player><team><limitedNutDrops>
            <item><key><string>Darts</string></key><value><int>3</int></value></item>
            <item><key><string>VolcanoMining</string></key><value><int>2</int></value></item>
          </limitedNutDrops></team></player>
          <goldenWalnutsFound>7</goldenWalnutsFound>
          <collectedNutTracker><string>Bush_IslandNorth_13_33</string><string>Mermaid</string></collectedNutTracker>
        </SaveGame>"#;
        let w = read(save.as_bytes()).unwrap();
        let get = |k: &str| w.sources.iter().find(|s| s.key == k).unwrap();
        assert_eq!(w.found, 7);
        assert_eq!(get("Bush_IslandNorth_13_33").have, 1);
        assert_eq!(get("Bush_IslandNorth_13_33").tile, Some((13, 33)));
        assert_eq!(get("Buried_IslandWest_21_81").kind, "buried");
        assert_eq!(get("Mermaid").have, 1);
        assert_eq!((get("Darts").have, get("Darts").max), (3, 3));
        assert_eq!(get("VolcanoMining").have, 2);
        assert_eq!(get("IslandFishing").have, 0);
    }

    // PELICAN_TEST_SAVE_FILE=<save file> cargo test -- --ignored real_walnuts --nocapture
    #[test]
    #[ignore]
    fn real_walnuts() {
        let file = std::env::var("PELICAN_TEST_SAVE_FILE").unwrap();
        let w = read(std::io::BufReader::new(std::fs::File::open(file).unwrap())).unwrap();
        let missing: Vec<&str> = w
            .sources
            .iter()
            .filter(|s| s.have < s.max)
            .map(|s| s.key.as_str())
            .collect();
        eprintln!(
            "found {} of {}; missing {}: {missing:?}",
            w.found,
            w.total,
            missing.len()
        );
    }
}
