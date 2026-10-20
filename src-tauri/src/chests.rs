//! Where a save keeps its things: every chest, fridge, auto-grabber and the inventory,
//! read straight from the save file, so "where are my iridium bars" works without the game.

use std::io::BufRead;

use quick_xml::events::{BytesStart, Event};
use serde::Serialize;

use crate::error::{AppError, AppResult};

/// One stack of something stored somewhere.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StoredItem {
    /// Qualified item id, such as "(O)337"; "(BC)" for big craftables.
    pub id: String,
    /// The item's internal English name, for when the catalog has no localized one.
    pub name: String,
    pub stack: u32,
    /// 0 normal, 1 silver, 2 gold, 4 iridium.
    pub quality: u8,
    pub place: Place,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Place {
    /// The location's internal name ("Farm", "FarmHouse", "Custom_X"), or the building's
    /// interior name; "inventory" for what the player carries.
    pub location: String,
    /// "chest", "fridge", "grabber" (an auto-grabber's hopper), "junimo" (the chests
    /// that share one inventory everywhere) or "inventory".
    pub kind: &'static str,
    /// The container's tile, when it stands somewhere.
    pub tile: Option<(i32, i32)>,
}

// a container whose <items> we're inside of, by the depth of its element
struct Container {
    depth: usize,
    kind: &'static str,
    tile: (Option<i32>, Option<i32>),
    key_tile: Option<(i32, i32)>,
    frame: Option<usize>,
}

#[derive(Default)]
struct ItemDraft {
    depth: usize,
    kind: String,
    big_craftable: bool,
    id: String,
    name: String,
    stack: u32,
    quality: u8,
}

// a location or a building's interior: its name comes as a child, often after its objects
struct Frame {
    depth: usize,
    name: Option<String>,
}

/// Lists every stored stack in save folder `dir`.
pub fn in_save(dir: &std::path::Path) -> AppResult<Vec<StoredItem>> {
    let id = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let file = std::fs::File::open(dir.join(id))?;
    read(std::io::BufReader::with_capacity(1 << 16, file))
}

/// Lists every stored stack in the save XML `reader`.
pub fn read(reader: impl BufRead) -> AppResult<Vec<StoredItem>> {
    let mut xml = quick_xml::Reader::from_reader(reader);
    let mut buf = Vec::new();
    let mut path: Vec<String> = Vec::new();
    let mut text = String::new();
    let mut frames: Vec<Frame> = Vec::new();
    let mut containers: Vec<Container> = Vec::new();
    let mut item: Option<ItemDraft> = None;
    // items waiting for their location's name, by frame depth
    let mut pending: Vec<(usize, StoredItem)> = Vec::new();
    // the tile of the object dictionary entry being read: <item><key><Vector2><X/><Y/>…
    let mut key_tile: (Option<i32>, Option<i32>) = (None, None);
    let mut out = Vec::new();

    loop {
        match xml
            .read_event_into(&mut buf)
            .map_err(|e| AppError::BadSave(e.to_string()))?
        {
            Event::Start(e) => {
                let name = e.local_name().as_ref().to_owned();
                let depth = path.len() + 1;
                let kind = type_of(&e);
                let parent = path.last().map(String::as_str);
                if (name == "GameLocation" && depth == 3) || name == "indoors" {
                    frames.push(Frame { depth, name: None });
                }
                // the farmhouse fridge is a Chest without a type attribute
                if (kind.as_deref() == Some("Chest") || name == "fridge") && item.is_none() {
                    containers.push(Container {
                        depth,
                        kind: match name.as_str() {
                            "fridge" => "fridge",
                            "heldObject" => "grabber",
                            _ => "chest",
                        },
                        tile: (None, None),
                        key_tile: key_tile.0.zip(key_tile.1),
                        frame: frames.last().map(|f| f.depth),
                    });
                }
                let shared = path.iter().any(|p| p == "globalInventories");
                let in_items = item.is_none()
                    && ((parent == Some("items")
                        && (containers.last().is_some_and(|c| depth == c.depth + 2)
                            || path.len() == 3 && path[1] == "player"))
                        || (shared && parent == Some("ArrayOfItem")));
                if in_items && name == "Item" {
                    item = Some(ItemDraft {
                        depth,
                        kind: kind.unwrap_or_default(),
                        stack: 1,
                        ..ItemDraft::default()
                    });
                }
                path.push(name);
                text.clear();
            }
            Event::Text(t) => text.push_str(&t.xml10_content()),
            Event::GeneralRef(r) => text.push_str(&crate::saves::entity(&r)),
            Event::End(_) => {
                let depth = path.len();
                let t = text.trim();
                let field = path.last().map(String::as_str).unwrap_or("");
                if let Some(d) = item.as_mut().filter(|d| depth == d.depth + 1) {
                    match field {
                        "itemId" => d.id = t.to_owned(),
                        "name" => d.name = t.to_owned(),
                        "stack" => d.stack = t.parse().unwrap_or(1),
                        "quality" => d.quality = t.parse().unwrap_or(0),
                        "bigCraftable" => d.big_craftable = t == "true",
                        _ => {}
                    }
                }
                if let Some(c) = containers.last_mut() {
                    // <tileLocation><X/><Y/></tileLocation> right under the container
                    if depth == c.depth + 2 && path[depth - 2] == "tileLocation" {
                        match field {
                            "X" => c.tile.0 = t.parse().ok(),
                            "Y" => c.tile.1 = t.parse().ok(),
                            _ => {}
                        }
                    }
                }
                if depth >= 3 && path[depth - 2] == "Vector2" && path[depth - 3] == "key" {
                    match field {
                        "X" => key_tile.0 = t.parse().ok(),
                        "Y" => key_tile.1 = t.parse().ok(),
                        _ => {}
                    }
                }
                if let Some(f) = frames.last_mut().filter(|f| depth == f.depth + 1)
                    && field == "name"
                {
                    f.name = Some(t.to_owned());
                }

                if item.as_ref().is_some_and(|d| d.depth == depth) {
                    let d = item.take().unwrap();
                    let shared = path.iter().any(|p| p == "globalInventories");
                    let place = match containers.last() {
                        _ if shared => Place {
                            location: "Junimo".to_owned(),
                            kind: "junimo",
                            tile: None,
                        },
                        // an auto-grabber's hopper has no tile of its own; its machine's
                        // dictionary key has it
                        Some(c) if path.len() > c.depth => Place {
                            location: String::new(),
                            kind: c.kind,
                            tile: c.tile.0.zip(c.tile.1).or(c.key_tile),
                        },
                        _ => Place {
                            location: "inventory".to_owned(),
                            kind: "inventory",
                            tile: None,
                        },
                    };
                    let stored = StoredItem {
                        id: qualified(&d),
                        name: d.name,
                        stack: d.stack,
                        quality: d.quality,
                        place,
                    };
                    match containers.last().and_then(|c| c.frame) {
                        Some(frame) if !matches!(stored.place.kind, "inventory" | "junimo") => {
                            pending.push((frame, stored))
                        }
                        _ => out.push(stored),
                    }
                }
                if containers.last().is_some_and(|c| c.depth == depth) {
                    containers.pop();
                }
                if let Some(f) = frames.pop_if(|f| f.depth == depth) {
                    let name = f.name.unwrap_or_default();
                    let (mine, rest): (Vec<_>, Vec<_>) =
                        pending.into_iter().partition(|(frame, _)| *frame == depth);
                    pending = rest;
                    out.extend(mine.into_iter().map(|(_, mut s)| {
                        s.place.location = name.clone();
                        s
                    }));
                }
                path.pop();
                text.clear();
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(out)
}

fn type_of(e: &BytesStart<'_>) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.local_name().as_ref() == "type")
        .map(|a| a.value.to_string())
}

// the xsi:type and bigCraftable flag say which item registry an itemId belongs to
fn qualified(d: &ItemDraft) -> String {
    let prefix = match d.kind.as_str() {
        "Object" | "ColoredObject" | "Ring" | "CombinedRing" if d.big_craftable => "(BC)",
        "Object" | "ColoredObject" | "Ring" | "CombinedRing" => "(O)",
        "Furniture" | "BedFurniture" | "FishTankFurniture" | "StorageFurniture" | "TV" => "(F)",
        "Hat" => "(H)",
        "Boots" => "(B)",
        "MeleeWeapon" | "Slingshot" => "(W)",
        "Clothing" => "",
        _ => "",
    };
    if prefix.is_empty() || d.id.starts_with('(') {
        d.id.clone()
    } else {
        format!("{prefix}{}", d.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAVE: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<SaveGame xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
  <player>
    <items>
      <Item xsi:type="Object"><name>Iridium Bar</name><itemId>337</itemId><quality>0</quality><stack>5</stack></Item>
      <Item xsi:nil="true" />
    </items>
  </player>
  <locations>
    <GameLocation xsi:type="Farm">
      <objects>
        <item><key><Vector2><X>21</X><Y>8</Y></Vector2></key>
          <value><Object xsi:type="Chest">
            <name>Chest</name><itemId>130</itemId><tileLocation><X>21</X><Y>8</Y></tileLocation>
            <items>
              <Item xsi:type="Object"><name>Duck Egg</name><itemId>442</itemId><quality>1</quality><stack>2</stack>
                <tileLocation><X>0</X><Y>0</Y></tileLocation></Item>
              <Item xsi:type="Object"><name>Keg</name><itemId>12</itemId><bigCraftable>true</bigCraftable><stack>1</stack></Item>
            </items>
          </Object></value></item>
        <item><key><Vector2><X>3</X><Y>4</Y></Vector2></key>
          <value><Object xsi:type="Object"><name>Auto-Grabber</name><tileLocation><X>3</X><Y>4</Y></tileLocation>
            <heldObject xsi:type="Chest"><items>
              <Item xsi:type="Object"><name>Milk</name><itemId>184</itemId><stack>3</stack></Item>
            </items></heldObject>
          </Object></value></item>
      </objects>
      <buildings>
        <Building><indoors xsi:type="FarmHouse"><name>Cabin</name>
          <fridge><items><Item xsi:type="Object"><name>Salad</name><itemId>196</itemId><stack>1</stack></Item></items></fridge>
        </indoors></Building>
      </buildings>
      <name>Farm</name>
    </GameLocation>
  </locations>
  <globalInventories>
    <item><key><string>JunimoChests</string></key>
      <value><ArrayOfItem><Item xsi:type="Object"><name>Ruby</name><itemId>64</itemId><stack>6</stack></Item></ArrayOfItem></value></item>
  </globalInventories>
</SaveGame>"#;

    #[test]
    fn finds_every_stack_and_its_place() {
        let items = read(SAVE.as_bytes()).unwrap();
        let find = |id: &str| items.iter().find(|i| i.id == id).unwrap();
        assert_eq!(items.len(), 6, "{items:#?}");
        let ruby = find("(O)64");
        assert_eq!((ruby.place.kind, ruby.stack), ("junimo", 6));
        assert_eq!(find("(O)337").place.kind, "inventory");
        assert_eq!(find("(O)337").stack, 5);
        let egg = find("(O)442");
        assert_eq!((egg.stack, egg.quality), (2, 1));
        assert_eq!(
            egg.place.location, "Farm",
            "the name comes after the objects"
        );
        assert_eq!(egg.place.tile, Some((21, 8)));
        assert_eq!(find("(BC)12").place.kind, "chest");
        let milk = find("(O)184");
        assert_eq!(
            (milk.place.kind, milk.place.tile),
            ("grabber", Some((3, 4)))
        );
        let salad = find("(O)196");
        assert_eq!(
            (salad.place.kind, salad.place.location.as_str()),
            ("fridge", "Cabin")
        );
    }

    // PELICAN_TEST_SAVE_FILE=<save file> cargo test -- --ignored real_save_items --nocapture
    #[test]
    #[ignore]
    fn real_save_items() {
        let file = std::env::var("PELICAN_TEST_SAVE_FILE").unwrap();
        let started = std::time::Instant::now();
        let items = read(std::io::BufReader::new(std::fs::File::open(file).unwrap())).unwrap();
        eprintln!("{} stacks in {:?}", items.len(), started.elapsed());
        let mut places: std::collections::BTreeMap<String, usize> = Default::default();
        for i in &items {
            *places
                .entry(format!("{} {}", i.place.kind, i.place.location))
                .or_default() += 1;
        }
        eprintln!("{places:#?}");
        eprintln!("{:?}", items.iter().find(|i| i.id == "(O)337"));
    }
}
