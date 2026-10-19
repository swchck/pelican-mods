//! Reading Stardew Valley save folders.
//!
//! A save is `Saves/<Farmer>_<id>/` holding `SaveGameInfo` (the farmer alone, a few KB,
//! enough for the list) and `<Farmer>_<id>` (the whole world, often 10–50 MB). The big
//! file is streamed once for the handful of progress counters; nothing builds a DOM.

use std::collections::HashMap;
use std::io::BufRead;
use std::path::{Path, PathBuf};

use quick_xml::events::Event;
use serde::Serialize;

use crate::error::{AppError, AppResult};

pub const INFO_FILE: &str = "SaveGameInfo";

/// The six Community Center rooms, by the mail flag the game sets when one is done.
pub const CC_ROOMS: [&str; 6] = [
    "ccPantry",
    "ccCraftsRoom",
    "ccFishTank",
    "ccBoilerRoom",
    "ccVault",
    "ccBulletin",
];

/// Joja's equivalents; buying all of them counts the same as restoring the rooms.
const JOJA_ROOMS: [&str; 5] = [
    "jojaPantry",
    "jojaCraftsRoom",
    "jojaFishTank",
    "jojaBoilerRoom",
    "jojaVault",
];

/// A save as the list shows it.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SaveSummary {
    /// Folder name, which is also the save's id.
    pub id: String,
    pub path: PathBuf,
    pub farmer: String,
    pub farm_name: String,
    pub money: u64,
    pub total_earned: u64,
    /// 0 spring, 1 summer, 2 fall, 3 winter.
    pub season: u8,
    pub day: u8,
    pub year: u32,
    pub play_time_ms: u64,
    pub game_version: Option<String>,
    pub skills: Skills,
    /// Unix seconds of the last save.
    pub modified: u64,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Skills {
    pub farming: u8,
    pub mining: u8,
    pub foraging: u8,
    pub fishing: u8,
    pub combat: u8,
}

/// Progress counters from the full save file.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    /// Community Center rooms restored, out of [`CC_ROOMS`].
    pub cc_rooms: Vec<String>,
    pub joja: bool,
    pub museum_items: u32,
    pub fish_species: u32,
    pub cooking_known: u32,
    pub cooking_cooked: u32,
    pub crafting_known: u32,
    pub friendships: Vec<Friend>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Friend {
    pub name: String,
    pub points: i32,
    /// Hearts as the social tab shows them: 250 points each.
    pub hearts: i32,
}

/// Lists the saves in `saves_dir`, newest first; unreadable folders are skipped.
pub fn list(saves_dir: &Path) -> Vec<SaveSummary> {
    let mut out: Vec<SaveSummary> = std::fs::read_dir(saves_dir)
        .into_iter()
        .flatten()
        .flatten()
        // dot folders are the launcher's own staging and swap leftovers, never saves
        .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .filter(|e| e.path().join(INFO_FILE).is_file())
        .filter_map(|e| summary(&e.path()).ok())
        .collect();
    out.sort_by_key(|s| std::cmp::Reverse(s.modified));
    out
}

/// Reads one save folder's `SaveGameInfo`.
pub fn summary(dir: &Path) -> AppResult<SaveSummary> {
    let id = dir
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .ok_or_else(|| AppError::NotFound(dir.display().to_string()))?;
    let file = std::fs::File::open(dir.join(INFO_FILE))?;
    let fields = root_fields(std::io::BufReader::new(file))?;
    let num = |k: &str| {
        fields
            .get(k)
            .and_then(|v| v.trim().parse::<u64>().ok())
            .unwrap_or(0)
    };
    let level = |k: &str| num(k).min(20) as u8;
    let modified = std::fs::metadata(dir.join(&id))
        .or_else(|_| std::fs::metadata(dir.join(INFO_FILE)))
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Ok(SaveSummary {
        farmer: fields.get("name").cloned().unwrap_or_default(),
        farm_name: fields.get("farmName").cloned().unwrap_or_default(),
        money: num("money"),
        total_earned: num("totalMoneyEarned"),
        season: season(fields.get("seasonForSaveGame").map(String::as_str)),
        day: num("dayOfMonthForSaveGame").min(28) as u8,
        year: num("yearForSaveGame").max(1) as u32,
        play_time_ms: num("millisecondsPlayed"),
        game_version: fields.get("gameVersion").cloned(),
        skills: Skills {
            farming: level("farmingLevel"),
            mining: level("miningLevel"),
            foraging: level("foragingLevel"),
            fishing: level("fishingLevel"),
            combat: level("combatLevel"),
        },
        modified,
        path: dir.to_path_buf(),
        id,
    })
}

// older saves store the season as a word, 1.6 as a number
fn season(v: Option<&str>) -> u8 {
    match v.map(|s| s.trim().to_ascii_lowercase()).as_deref() {
        Some("1" | "summer") => 1,
        Some("2" | "fall") => 2,
        Some("3" | "winter") => 3,
        _ => 0,
    }
}

/// Collects the text of the root element's direct children.
fn root_fields(reader: impl BufRead) -> AppResult<HashMap<String, String>> {
    let mut xml = quick_xml::Reader::from_reader(reader);
    let mut buf = Vec::new();
    let mut depth = 0usize;
    let mut current: Option<String> = None;
    let mut fields = HashMap::new();
    loop {
        match xml.read_event_into(&mut buf).map_err(bad)? {
            Event::Start(e) => {
                depth += 1;
                if depth == 2 {
                    current = Some(e.local_name().as_ref().to_owned());
                }
            }
            Event::End(_) => {
                if depth == 2 {
                    current = None;
                }
                depth = depth.saturating_sub(1);
            }
            Event::Text(t) if depth == 2 => {
                if let Some(k) = &current {
                    fields
                        .entry(k.clone())
                        .or_insert_with(String::new)
                        .push_str(&t.xml10_content());
                }
            }
            Event::GeneralRef(r) if depth == 2 => {
                if let Some(k) = &current {
                    fields
                        .entry(k.clone())
                        .or_insert_with(String::new)
                        .push_str(&entity(&r));
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(fields)
}

pub(crate) fn entity(r: &quick_xml::events::BytesRef<'_>) -> String {
    if let Ok(Some(c)) = r.resolve_char_ref() {
        return c.to_string();
    }
    match r.as_ref() {
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "quot" => "\"",
        "apos" => "'",
        _ => "",
    }
    .to_owned()
}

fn bad(e: quick_xml::Error) -> AppError {
    AppError::BadSave(e.to_string())
}

/// Streams the full save file of `dir` and counts progress.
pub fn progress(dir: &Path) -> AppResult<Progress> {
    let id = dir
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .ok_or_else(|| AppError::NotFound(dir.display().to_string()))?;
    let file = std::fs::File::open(dir.join(&id))?;
    progress_from(std::io::BufReader::with_capacity(1 << 16, file))
}

fn progress_from(reader: impl BufRead) -> AppResult<Progress> {
    let mut xml = quick_xml::Reader::from_reader(reader);
    let mut buf = Vec::new();
    // element names from the root down to the current one
    let mut path: Vec<String> = Vec::new();
    // depth of a <GameLocation xsi:type="LibraryMuseum">, while inside one
    let mut museum_at: Option<usize> = None;
    let mut text = String::new();
    let mut mail = Vec::new();
    let mut friend_name: Option<String> = None;
    let mut p = Progress::default();

    let is = |path: &[String], tail: &[&str]| {
        path.len() >= tail.len()
            && path[path.len() - tail.len()..]
                .iter()
                .zip(tail)
                .all(|(a, b)| a == b)
    };

    loop {
        match xml.read_event_into(&mut buf).map_err(bad)? {
            Event::Start(e) => {
                let name = e.local_name().as_ref().to_owned();
                if name == "GameLocation" && museum_at.is_none() {
                    let museum = e.attributes().flatten().any(|a| {
                        a.key.local_name().as_ref() == "type" && a.value.as_ref() == "LibraryMuseum"
                    });
                    if museum {
                        museum_at = Some(path.len());
                    }
                }
                path.push(name);
                text.clear();

                // only the main player counts; farmhands live under <farmhands>
                let on_player = path.len() >= 3 && path[1] == "player";
                if on_player && path.len() == 4 && path[3] == "item" {
                    match path[2].as_str() {
                        "fishCaught" => p.fish_species += 1,
                        "cookingRecipes" => p.cooking_known += 1,
                        "recipesCooked" => p.cooking_cooked += 1,
                        "craftingRecipes" => p.crafting_known += 1,
                        _ => {}
                    }
                }
                if let Some(m) = museum_at
                    && path.len() == m + 3
                    && path[m + 1] == "museumPieces"
                    && path[m + 2] == "item"
                {
                    p.museum_items += 1;
                }
            }
            Event::Text(t) => text.push_str(&t.xml10_content()),
            Event::GeneralRef(r) => text.push_str(&entity(&r)),
            Event::End(_) => {
                let t = text.trim();
                if path.len() == 4 && path[1] == "player" && path[2] == "mailReceived" {
                    mail.push(t.to_owned());
                }
                if path.len() >= 2 && path[1] == "player" {
                    if is(&path, &["friendshipData", "item", "key", "string"]) && path.len() == 6 {
                        friend_name = Some(t.to_owned());
                    }
                    if is(
                        &path,
                        &["friendshipData", "item", "value", "Friendship", "Points"],
                    ) && path.len() == 7
                        && let (Some(name), Ok(points)) = (friend_name.take(), t.parse::<i32>())
                    {
                        p.friendships.push(Friend {
                            name,
                            hearts: points.max(0) / 250,
                            points,
                        });
                    }
                }
                path.pop();
                if museum_at == Some(path.len()) {
                    museum_at = None;
                }
                text.clear();
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    p.cc_rooms = CC_ROOMS
        .iter()
        .filter(|r| mail.iter().any(|m| m == *r))
        .map(|r| (*r).to_owned())
        .collect();
    p.joja = mail.iter().any(|m| m == "JojaMember")
        && JOJA_ROOMS.iter().all(|r| mail.iter().any(|m| m == *r));
    p.friendships.sort_by_key(|f| std::cmp::Reverse(f.points));
    Ok(p)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub const INFO: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<Farmer xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:xsd="http://www.w3.org/2001/XMLSchema">
  <name>Аня</name>
  <farmName>Ромашка &amp; Co</farmName>
  <money>1284500</money>
  <totalMoneyEarned>3500000</totalMoneyEarned>
  <millisecondsPlayed>36000000</millisecondsPlayed>
  <dayOfMonthForSaveGame>14</dayOfMonthForSaveGame>
  <seasonForSaveGame>1</seasonForSaveGame>
  <yearForSaveGame>2</yearForSaveGame>
  <farmingLevel>10</farmingLevel>
  <miningLevel>8</miningLevel>
  <foragingLevel>7</foragingLevel>
  <fishingLevel>6</fishingLevel>
  <combatLevel>5</combatLevel>
  <gameVersion>1.6.15</gameVersion>
  <hat><name>Cowboy Hat</name></hat>
</Farmer>"#;

    pub const WORLD: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<SaveGame xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
  <player>
    <name>Аня</name>
    <mailReceived><string>ccPantry</string><string>ccVault</string><string>spring_2_1</string></mailReceived>
    <fishCaught>
      <item><key><string>(O)128</string></key><value><ArrayOfInt><int>1</int></ArrayOfInt></value></item>
      <item><key><string>(O)129</string></key><value><ArrayOfInt><int>2</int></ArrayOfInt></value></item>
    </fishCaught>
    <cookingRecipes><item><key><string>Fried Egg</string></key><value><int>0</int></value></item></cookingRecipes>
    <recipesCooked><item><key><string>194</string></key><value><int>3</int></value></item></recipesCooked>
    <craftingRecipes>
      <item><key><string>Chest</string></key><value><int>1</int></value></item>
      <item><key><string>Torch</string></key><value><int>0</int></value></item>
    </craftingRecipes>
    <friendshipData>
      <item><key><string>Abigail</string></key><value><Friendship><Points>1250</Points><Status>Friendly</Status></Friendship></value></item>
      <item><key><string>Sebastian</string></key><value><Friendship><Points>300</Points></Friendship></value></item>
    </friendshipData>
  </player>
  <farmhands>
    <Farmer><fishCaught><item><key><string>(O)1</string></key></item></fishCaught></Farmer>
  </farmhands>
  <locations>
    <GameLocation xsi:type="Farm"><museumPieces><item/></museumPieces></GameLocation>
    <GameLocation xsi:type="LibraryMuseum">
      <name>ArchaeologyHouse</name>
      <museumPieces>
        <item><key><Vector2><X>26</X><Y>5</Y></Vector2></key><value><string>(O)96</string></value></item>
        <item><key><Vector2><X>27</X><Y>5</Y></Vector2></key><value><string>(O)97</string></value></item>
        <item><key><Vector2><X>28</X><Y>5</Y></Vector2></key><value><string>(O)98</string></value></item>
      </museumPieces>
    </GameLocation>
  </locations>
</SaveGame>"#;

    /// Writes a save folder named `id` into `saves` and returns its path.
    pub fn write_save(saves: &Path, id: &str) -> PathBuf {
        let dir = saves.join(id);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(INFO_FILE), INFO).unwrap();
        std::fs::write(dir.join(id), WORLD).unwrap();
        dir
    }

    #[test]
    fn reads_summary() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = write_save(tmp.path(), "Anya_123456789");
        let s = summary(&dir).unwrap();
        assert_eq!(s.id, "Anya_123456789");
        assert_eq!(s.farmer, "Аня");
        assert_eq!(s.farm_name, "Ромашка & Co");
        assert_eq!((s.season, s.day, s.year), (1, 14, 2));
        assert_eq!(s.money, 1_284_500);
        assert_eq!(s.play_time_ms, 36_000_000);
        assert_eq!(s.skills.farming, 10);
        assert_eq!(s.skills.combat, 5);
        assert_eq!(s.game_version.as_deref(), Some("1.6.15"));
    }

    #[test]
    fn counts_progress_for_main_player_only() {
        let p = progress_from(WORLD.as_bytes()).unwrap();
        assert_eq!(p.cc_rooms, ["ccPantry", "ccVault"]);
        assert!(!p.joja);
        assert_eq!(p.fish_species, 2);
        assert_eq!(p.cooking_known, 1);
        assert_eq!(p.cooking_cooked, 1);
        assert_eq!(p.crafting_known, 2);
        assert_eq!(p.museum_items, 3);
        assert_eq!(p.friendships[0].name, "Abigail");
        assert_eq!(p.friendships[0].hearts, 5);
        assert_eq!(p.friendships.len(), 2);
    }

    #[test]
    fn lists_saves_and_skips_junk() {
        let tmp = tempfile::tempdir().unwrap();
        write_save(tmp.path(), "Anya_1");
        std::fs::create_dir_all(tmp.path().join("not a save")).unwrap();
        let all = list(tmp.path());
        assert_eq!(all.len(), 1);
        assert_eq!(season(Some("winter")), 3);
    }
}
