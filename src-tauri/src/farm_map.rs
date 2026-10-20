//! A top-down map of one outdoor location of a save: buildings, crops with the days they
//! still need, trees, machines with something ready, chests, sprinklers and debris.
//!
//! The save lists a location's children before its `<name>`, so each location under
//! `<locations>` is held as a small tree until its name says whether it's the one asked
//! for. Building interiors and chest contents are skipped while reading; they'd be most
//! of the tree and none of the map.

use std::io::BufRead;

use quick_xml::events::Event;
use serde::Serialize;

use crate::error::{AppError, AppResult};

/// Children never worth holding in memory for a map.
const SKIPPED: &[&str] = &[
    "indoors",
    "items",
    "heldObject",
    "animals",
    "Animals",
    "characters",
    "furniture",
    "modData",
    "lastQuestionKey",
];

/// Something that takes up tiles.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Feature {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    /// "building", "crop", "deadCrop", "dirt", "tree", "fruitTree", "bush", "grass",
    /// "flooring", "clump", "debris", "chest", "sprinkler", "scarecrow", "machine",
    /// "fence" or "object".
    pub kind: &'static str,
    /// An item id for crops (the harvest) and objects, a building type for buildings.
    pub id: String,
    /// The internal English name, where the save gives one.
    pub name: String,
    /// Days until a crop can be picked; 0 when it's ready.
    pub days: Option<u32>,
    /// Watered tilled soil, with or without a crop.
    pub watered: bool,
    /// A machine with something to collect.
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FarmMap {
    pub location: String,
    /// Tiles across and down that anything occupies, from 0.
    pub width: i32,
    pub height: i32,
    pub features: Vec<Feature>,
}

#[derive(Debug, Default)]
struct Node {
    name: String,
    ty: Option<String>,
    text: String,
    children: Vec<Node>,
}

impl Node {
    fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name == name)
    }

    fn at(&self, path: &[&str]) -> Option<&Node> {
        path.iter().try_fold(self, |n, p| n.child(p))
    }

    fn text_at(&self, path: &[&str]) -> &str {
        self.at(path).map_or("", |n| n.text.trim())
    }

    fn num(&self, path: &[&str]) -> i32 {
        self.text_at(path).parse().unwrap_or(0)
    }

    fn flag(&self, path: &[&str]) -> bool {
        self.text_at(path) == "true"
    }

    fn vector(&self) -> (i32, i32) {
        (self.num(&["X"]), self.num(&["Y"]))
    }
}

/// Reads the map of `location` ("Farm", "Greenhouse", "IslandWest") from the save XML
/// `reader`.
pub fn read(reader: impl BufRead, location: &str) -> AppResult<FarmMap> {
    let node =
        find_location(reader, location)?.ok_or_else(|| AppError::NotFound(location.to_owned()))?;
    let mut features = Vec::new();
    if let Some(list) = node.child("buildings") {
        features.extend(list.children.iter().map(building));
    }
    for item in node
        .child("terrainFeatures")
        .map_or(&[][..], |n| &n.children)
    {
        let (Some(key), Some(value)) = (
            item.at(&["key", "Vector2"]),
            item.at(&["value", "TerrainFeature"]),
        ) else {
            continue;
        };
        if let Some(f) = terrain(key.vector(), value) {
            features.push(f);
        }
    }
    for item in node.child("objects").map_or(&[][..], |n| &n.children) {
        let (Some(key), Some(value)) =
            (item.at(&["key", "Vector2"]), item.at(&["value", "Object"]))
        else {
            continue;
        };
        features.push(object(key.vector(), value));
    }
    for clump in node
        .child("resourceClumps")
        .map_or(&[][..], |n| &n.children)
    {
        let (x, y) = clump.child("tile").map_or((0, 0), Node::vector);
        features.push(plain(
            x,
            y,
            clump.num(&["width"]),
            clump.num(&["height"]),
            "clump",
            clump.text_at(&["parentSheetIndex"]),
        ));
    }
    for large in node
        .child("largeTerrainFeatures")
        .map_or(&[][..], |n| &n.children)
    {
        if large.ty.as_deref() == Some("Bush") {
            let (x, y) = large.child("tilePosition").map_or((0, 0), Node::vector);
            // size 0 small, 1 medium, 2 large (3 wide), 3 the walnut bush, 4 tea
            let w = if large.num(&["size"]) == 2 { 3 } else { 1 };
            features.push(plain(x, y, w, 1, "bush", ""));
        }
    }
    let width = features.iter().map(|f| f.x + f.w).max().unwrap_or(0);
    let height = features.iter().map(|f| f.y + f.h).max().unwrap_or(0);
    Ok(FarmMap {
        location: location.to_owned(),
        width,
        height,
        features,
    })
}

fn plain(x: i32, y: i32, w: i32, h: i32, kind: &'static str, id: &str) -> Feature {
    Feature {
        x,
        y,
        w: w.max(1),
        h: h.max(1),
        kind,
        id: id.to_owned(),
        name: String::new(),
        days: None,
        watered: false,
        ready: false,
    }
}

fn building(b: &Node) -> Feature {
    let kind = b.text_at(&["buildingType"]);
    Feature {
        name: kind.to_owned(),
        ..plain(
            b.num(&["tileX"]),
            b.num(&["tileY"]),
            b.num(&["tilesWide"]),
            b.num(&["tilesHigh"]),
            "building",
            kind,
        )
    }
}

fn terrain((x, y): (i32, i32), t: &Node) -> Option<Feature> {
    let kind = match t.ty.as_deref()? {
        "HoeDirt" => "dirt",
        "Tree" => "tree",
        "FruitTree" => "fruitTree",
        "Grass" => "grass",
        "Flooring" => "flooring",
        _ => return None,
    };
    let mut f = plain(x, y, 1, 1, kind, "");
    if kind == "fruitTree" {
        f.id = t.text_at(&["treeId"]).to_owned();
    }
    if kind == "dirt" {
        // 1 watered, 2 the "invisible" state some mods use; only 1 counts
        f.watered = t.num(&["state"]) == 1;
        if let Some(crop) = t.child("crop") {
            f.id = crop.text_at(&["indexOfHarvest"]).to_owned();
            if crop.flag(&["dead"]) {
                f.kind = "deadCrop";
            } else {
                f.kind = "crop";
                f.days = Some(days_left(crop));
            }
        }
    }
    Some(f)
}

// the last phase is the game's 99999 "grown" sentinel; a regrowing crop stays in it and
// counts dayOfCurrentPhase down to its next harvest
fn days_left(crop: &Node) -> u32 {
    let phases: Vec<u32> = crop
        .child("phaseDays")
        .map_or(&[][..], |n| &n.children)
        .iter()
        .filter_map(|n| n.text.trim().parse().ok())
        .collect();
    let current = crop.num(&["currentPhase"]).max(0) as usize;
    let day = crop.num(&["dayOfCurrentPhase"]).max(0) as u32;
    let last = phases.len().saturating_sub(1);
    if current >= last {
        return if crop.flag(&["fullGrown"]) { day } else { 0 };
    }
    phases[current].saturating_sub(day) + phases[current + 1..last].iter().sum::<u32>()
}

fn object((x, y): (i32, i32), o: &Node) -> Feature {
    let name = o.text_at(&["name"]);
    let kind = match o.ty.as_deref() {
        Some("Chest") => "chest",
        Some("Fence") => "fence",
        _ if name.contains("Sprinkler") => "sprinkler",
        _ if name.contains("arecrow") => "scarecrow",
        _ if o.text_at(&["type"]) == "Litter" => "debris",
        _ if o.flag(&["bigCraftable"]) => "machine",
        _ => "object",
    };
    Feature {
        name: name.to_owned(),
        ready: kind == "machine" && o.flag(&["readyForHarvest"]),
        ..plain(x, y, 1, 1, kind, o.text_at(&["itemId"]))
    }
}

// holds each location under <locations> as a tree and returns the one named `wanted`
fn find_location(reader: impl BufRead, wanted: &str) -> AppResult<Option<Node>> {
    let mut xml = quick_xml::Reader::from_reader(reader);
    let mut buf = Vec::new();
    let mut depth = 0usize;
    let mut in_locations: Option<usize> = None;
    let mut stack: Vec<Node> = Vec::new();
    let mut skip_until: Option<usize> = None;
    loop {
        let event = xml
            .read_event_into(&mut buf)
            .map_err(|e| AppError::BadSave(e.to_string()))?;
        match event {
            Event::Start(e) => {
                depth += 1;
                let name = e.local_name().as_ref().to_owned();
                if skip_until.is_some() {
                } else if in_locations.is_none() && depth == 2 && name == "locations" {
                    in_locations = Some(depth);
                } else if in_locations.is_some_and(|d| depth == d + 1) || !stack.is_empty() {
                    if !stack.is_empty() && SKIPPED.contains(&name.as_str()) {
                        skip_until = Some(depth);
                    } else {
                        let ty = e
                            .attributes()
                            .flatten()
                            .find(|a| a.key.local_name().as_ref() == "type")
                            .map(|a| a.value.to_string());
                        stack.push(Node {
                            name,
                            ty,
                            ..Node::default()
                        });
                    }
                }
            }
            Event::Empty(e) => {
                if skip_until.is_none() && !stack.is_empty() {
                    let name = e.local_name().as_ref().to_owned();
                    stack.last_mut().expect("open node").children.push(Node {
                        name,
                        ..Node::default()
                    });
                }
            }
            Event::Text(t) if skip_until.is_none() => {
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&t.xml10_content());
                }
            }
            Event::End(_) => {
                if skip_until == Some(depth) {
                    skip_until = None;
                } else if skip_until.is_none() {
                    if let Some(node) = stack.pop() {
                        match stack.last_mut() {
                            Some(parent) => parent.children.push(node),
                            None if node.text_at(&["name"]) == wanted => return Ok(Some(node)),
                            None => {}
                        }
                    } else if in_locations == Some(depth) {
                        return Ok(None);
                    }
                }
                depth -= 1;
            }
            Event::Eof => return Ok(None),
            _ => {}
        }
        buf.clear();
    }
}

/// Reads the map of `location` from save folder `dir`.
pub fn in_save(dir: &std::path::Path, location: &str) -> AppResult<FarmMap> {
    let id = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    read(
        std::io::BufReader::with_capacity(1 << 16, std::fs::File::open(dir.join(id))?),
        location,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAVE: &str = r#"<SaveGame xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><player><name>A</name></player>
      <locations>
        <GameLocation><name>Town</name></GameLocation>
        <GameLocation xsi:type="Farm">
          <buildings><Building xsi:type="Barn"><indoors><objects><item/></objects></indoors>
            <tileX>10</tileX><tileY>4</tileY><tilesWide>7</tilesWide><tilesHigh>4</tilesHigh><buildingType>Deluxe Barn</buildingType></Building></buildings>
          <objects>
            <item><key><Vector2><X>3</X><Y>3</Y></Vector2></key><value><Object xsi:type="Chest"><name>Chest</name><items><Item><name>Wood</name></Item></items><bigCraftable>true</bigCraftable><itemId>130</itemId></Object></value></item>
            <item><key><Vector2><X>5</X><Y>3</Y></Vector2></key><value><Object><name>Keg</name><bigCraftable>true</bigCraftable><readyForHarvest>true</readyForHarvest><itemId>12</itemId></Object></value></item>
            <item><key><Vector2><X>6</X><Y>3</Y></Vector2></key><value><Object><name>Weeds</name><type>Litter</type><itemId>674</itemId></Object></value></item>
          </objects>
          <resourceClumps><ResourceClump><width>2</width><height>2</height><parentSheetIndex>600</parentSheetIndex><tile><X>30</X><Y>40</Y></tile></ResourceClump></resourceClumps>
          <terrainFeatures>
            <item><key><Vector2><X>7</X><Y>24</Y></Vector2></key><value><TerrainFeature xsi:type="HoeDirt"><state>1</state><crop>
              <phaseDays><int>1</int><int>1</int><int>1</int><int>2</int><int>1</int><int>99999</int></phaseDays>
              <currentPhase>3</currentPhase><indexOfHarvest>192</indexOfHarvest><dayOfCurrentPhase>1</dayOfCurrentPhase><fullGrown>false</fullGrown><dead>false</dead></crop></TerrainFeature></value></item>
            <item><key><Vector2><X>8</X><Y>24</Y></Vector2></key><value><TerrainFeature xsi:type="HoeDirt"><state>0</state><crop>
              <phaseDays><int>1</int><int>99999</int></phaseDays><currentPhase>1</currentPhase><indexOfHarvest>254</indexOfHarvest>
              <dayOfCurrentPhase>3</dayOfCurrentPhase><fullGrown>true</fullGrown><dead>false</dead></crop></TerrainFeature></value></item>
            <item><key><Vector2><X>9</X><Y>24</Y></Vector2></key><value><TerrainFeature xsi:type="Tree"><treeType>1</treeType></TerrainFeature></value></item>
          </terrainFeatures>
          <name>Farm</name>
        </GameLocation>
      </locations></SaveGame>"#;

    #[test]
    fn maps_a_location_and_its_crops() {
        let map = read(SAVE.as_bytes(), "Farm").unwrap();
        let kind = |k: &str| {
            map.features
                .iter()
                .filter(|f| f.kind == k)
                .collect::<Vec<_>>()
        };
        assert_eq!((map.width, map.height), (32, 42));
        let barn = kind("building")[0];
        assert_eq!(
            (barn.x, barn.y, barn.w, barn.h, barn.name.as_str()),
            (10, 4, 7, 4, "Deluxe Barn")
        );
        assert_eq!(kind("chest").len(), 1);
        assert!(kind("machine")[0].ready);
        assert_eq!(kind("debris").len(), 1);
        assert_eq!(kind("clump")[0].w, 2);
        let crops = kind("crop");
        // phase 3 of 2 days, one day in, then one more day of phase 4
        assert_eq!(
            (crops[0].days, crops[0].watered, crops[0].id.as_str()),
            (Some(2), true, "192")
        );
        assert_eq!(
            crops[1].days,
            Some(3),
            "a regrowing crop counts down to its next harvest"
        );
        assert_eq!(kind("tree").len(), 1);
        assert!(matches!(
            read(SAVE.as_bytes(), "Greenhouse"),
            Err(AppError::NotFound(_))
        ));
    }

    // PELICAN_TEST_SAVE_FILE=<save file> cargo test -- --ignored real_farm_map --nocapture
    #[test]
    #[ignore]
    fn real_farm_map() {
        let file = std::env::var("PELICAN_TEST_SAVE_FILE").unwrap();
        for loc in ["Farm", "Greenhouse", "IslandWest"] {
            let started = std::time::Instant::now();
            let map = read(
                std::io::BufReader::new(std::fs::File::open(&file).unwrap()),
                loc,
            );
            let Ok(map) = map else {
                eprintln!("{loc}: none");
                continue;
            };
            let mut kinds = std::collections::BTreeMap::new();
            for f in &map.features {
                *kinds.entry(f.kind).or_insert(0) += 1;
            }
            eprintln!(
                "{loc} {}x{} in {:?}: {kinds:?}",
                map.width,
                map.height,
                started.elapsed()
            );
        }
    }
}
