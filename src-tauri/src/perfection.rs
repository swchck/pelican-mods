//! The perfection tracker, computed offline from a save file.
//!
//! Scoring mirrors `Utility.percentGameComplete` in Stardew Valley 1.6. Most parts
//! compare the save against lists only the running game knows (which fish exist, which
//! items count as shipped, with mods), so the companion sends a [`Catalog`] whenever it
//! connects and the launcher keeps the last one. Without it those parts report what the
//! save has, but no totals.

use std::collections::{HashMap, HashSet};
use std::io::BufRead;
use std::path::Path;

use quick_xml::events::Event;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

/// Golden walnuts that count; more exist only as rewards past perfection.
const WALNUTS: u32 = 130;
const LEVEL_CAP: u32 = 25;
const OBELISKS: [&str; 4] = [
    "Water Obelisk",
    "Earth Obelisk",
    "Desert Obelisk",
    "Island Obelisk",
];
const GOLD_CLOCK: &str = "Gold Clock";

/// Mail flags for each stardrop, in the order the wiki lists them.
pub const STARDROPS: [&str; 7] = [
    "CF_Fair",
    "CF_Mines",
    "CF_Spouse",
    "CF_Sewer",
    "CF_Statue",
    "CF_Fish",
    "museumComplete",
];

/// What counts toward perfection in the player's game, mods included.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    /// Set on the copy built into the launcher; a game never sends it.
    #[serde(skip)]
    pub builtin: bool,
    pub game: String,
    /// The game's language when the lists were made; names are in it.
    pub language: String,
    /// Unqualified ids, as `basicShipped` keys them.
    pub shipping: Vec<CatalogItem>,
    /// Qualified ids, as `fishCaught` keys them.
    pub fish: Vec<CatalogItem>,
    pub cooking: Vec<CatalogRecipe>,
    pub crafting: Vec<CatalogRecipe>,
    pub villagers: Vec<CatalogVillager>,
    pub monsters: Vec<CatalogGoal>,
    // added after 0.2.0; default so a catalog cached by an older version still loads
    /// Every object, unqualified id → display name.
    #[serde(default)]
    pub items: Vec<CatalogItem>,
    #[serde(default)]
    pub museum: Vec<CatalogMuseumItem>,
    #[serde(default)]
    pub achievements: Vec<CatalogAchievement>,
    #[serde(default)]
    pub special_orders: Vec<CatalogOrder>,
    #[serde(default)]
    pub crops: Vec<CatalogCrop>,
    #[serde(default)]
    pub festivals: Vec<CatalogFestival>,
    #[serde(default)]
    pub fish_info: Vec<CatalogFish>,
    /// The game's own names for things the UI would otherwise translate itself, keyed
    /// "room.Pantry", "skill.farming", "building.Gold Clock", "location.Town".
    #[serde(default)]
    pub terms: std::collections::BTreeMap<String, String>,
}

impl Catalog {
    /// The game's name for `key`, when the catalog came with one.
    pub fn term(&self, key: &str) -> Option<&str> {
        self.terms.get(key).map(String::as_str)
    }

    /// The base game's lists in `language` (a game language code such as "ru"), or in
    /// English for one the game doesn't ship; for a launcher that hasn't heard from the game yet.
    pub fn builtin(language: &str) -> &'static Catalog {
        static PARSED: [std::sync::OnceLock<Catalog>; BUILTIN.len()] =
            [const { std::sync::OnceLock::new() }; BUILTIN.len()];
        let i = BUILTIN
            .iter()
            .position(|(code, _)| *code == language)
            .unwrap_or(0);
        PARSED[i].get_or_init(|| Catalog {
            builtin: true,
            ..serde_json::from_str(BUILTIN[i].1).expect("the built-in catalog parses")
        })
    }

    /// Which game, language and source the lists came from.
    pub fn info(&self) -> CatalogInfo {
        CatalogInfo {
            game: self.game.clone(),
            language: self.language.clone(),
            builtin: self.builtin,
        }
    }
}

// English first: it stands in for any language missing here
const BUILTIN: [(&str, &str); 12] = [
    ("en", include_str!("../resources/catalogs/en.json")),
    ("ja", include_str!("../resources/catalogs/ja.json")),
    ("ru", include_str!("../resources/catalogs/ru.json")),
    ("zh", include_str!("../resources/catalogs/zh.json")),
    ("pt", include_str!("../resources/catalogs/pt.json")),
    ("es", include_str!("../resources/catalogs/es.json")),
    ("de", include_str!("../resources/catalogs/de.json")),
    ("fr", include_str!("../resources/catalogs/fr.json")),
    ("ko", include_str!("../resources/catalogs/ko.json")),
    ("it", include_str!("../resources/catalogs/it.json")),
    ("tr", include_str!("../resources/catalogs/tr.json")),
    ("hu", include_str!("../resources/catalogs/hu.json")),
];

/// Returns the English wiki page title for base game object `id` ("24" or "(O)24"),
/// which both Stardew wikis name pages by; mod items have none.
pub fn wiki_title(id: &str) -> Option<String> {
    static TITLES: std::sync::OnceLock<HashMap<String, String>> = std::sync::OnceLock::new();
    let titles = TITLES.get_or_init(|| {
        let en = Catalog::builtin("en");
        let villagers = en.villagers.iter().map(|v| (v.key.clone(), v.name.clone()));
        let items = en.items.iter().map(|i| (i.id.clone(), i.name.clone()));
        items.chain(villagers).collect()
    });
    titles.get(id.strip_prefix("(O)").unwrap_or(id)).cloned()
}

/// The language code the game was last set to, from its startup preferences in `data_dir`.
pub fn game_language(data_dir: &Path) -> Option<String> {
    let prefs = std::fs::read_to_string(data_dir.join("startup_preferences")).ok()?;
    let start = prefs.find("<languageCode>")? + "<languageCode>".len();
    let len = prefs[start..].find('<')?;
    Some(prefs[start..start + len].to_owned())
}

/// Where a report's names and totals came from.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogInfo {
    pub game: String,
    pub language: String,
    /// The launcher's own base-game lists: no mod content.
    pub builtin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogMuseumItem {
    pub id: String,
    pub name: String,
    /// "artifact" or "mineral".
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogAchievement {
    pub id: u32,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogOrder {
    pub key: String,
    pub name: String,
    pub requester: String,
    /// Qi's walnut-room orders rather than the town board's.
    pub qi: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogCrop {
    /// Unqualified harvest item id.
    pub id: String,
    pub polyculture: bool,
    pub monoculture: bool,
    // added with the season plan; default so older catalogs still load
    /// Unqualified seed id.
    #[serde(default)]
    pub seed: Option<String>,
    /// "spring", "summer", "fall", "winter".
    #[serde(default)]
    pub seasons: Vec<String>,
}

/// A festival, or a passive one like the Night Market, over `day..=end_day`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogFestival {
    pub season: String,
    pub day: u32,
    pub end_day: u32,
    pub name: String,
}

/// When and where a fish bites.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogFish {
    /// Qualified, as `fishCaught` keys it.
    pub id: String,
    pub name: String,
    /// Empty for crab pot catches, which bite all year.
    pub seasons: Vec<String>,
    /// "sunny", "rainy", "both", or "any" for crab pots.
    pub weather: String,
    /// Clock windows as HHMM pairs, start inclusive, end exclusive.
    pub times: Vec<[u32; 2]>,
    /// Location display names where it can be caught.
    pub locations: Vec<String>,
    /// Caught in crab pots rather than with a rod.
    pub trap: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogItem {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogRecipe {
    pub key: String,
    pub name: String,
    /// Unqualified id of the item it makes; `recipesCooked` is keyed by it.
    pub output: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogVillager {
    pub key: String,
    pub name: String,
    /// Datable villagers max out at 8 hearts for perfection, others at 10.
    pub romance: bool,
    #[serde(default)]
    pub birth_season: Option<String>,
    /// 0 when the villager has no birthday.
    #[serde(default)]
    pub birth_day: u32,
    /// Unqualified ids of the items they love most.
    #[serde(default)]
    pub loves: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogGoal {
    pub key: String,
    pub name: String,
    pub count: u32,
    pub targets: Vec<String>,
}

/// What the main player's save says, as far as perfection cares.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Facts {
    pub shipped: HashSet<String>,
    pub cooking_known: HashSet<String>,
    pub cooked: HashSet<String>,
    /// Recipe name → times crafted; learned but never made is 0.
    pub crafted: HashMap<String, u32>,
    pub fish: HashSet<String>,
    pub mail: HashSet<String>,
    pub friends: HashMap<String, i32>,
    /// Sum of the six skill levels, luck included; the farmer level is half.
    pub skill_levels: u32,
    pub slayer_done: bool,
    pub kills: HashMap<String, u32>,
    pub mine_100_chest: bool,
    pub walnuts: u32,
    /// Finished buildings anywhere in the world, by type.
    pub buildings: HashSet<String>,
    /// Item id → number shipped.
    pub shipped_counts: HashMap<String, u32>,
    /// Qualified fish id → number caught.
    pub fish_counts: HashMap<String, u32>,
    pub money_earned: u64,
    /// Experience in the save's order: farming, fishing, foraging, mining, combat.
    pub xp: Vec<u32>,
    /// Levels: farming, fishing, foraging, mining, combat, luck.
    pub levels: [u32; 6],
    /// `stats/Values`, the 1.6 stat store (books read, mastery, quests…).
    pub stats: HashMap<String, u64>,
    pub achievements: HashSet<u32>,
    pub events: HashSet<String>,
    pub notes: HashSet<u32>,
    pub artifacts_found: HashSet<String>,
    pub minerals_found: HashSet<String>,
    /// Item ids in the museum, unqualified.
    pub donated: HashSet<String>,
    pub house_level: u32,
    pub spouse: Option<String>,
    pub children: u32,
    pub deepest_mine: u32,
    pub friend_status: HashMap<String, String>,
    pub special_orders: HashSet<String>,
    /// "Room/id" → bundle definition, as `bundleData` stores it.
    pub bundle_data: Vec<(String, String)>,
    /// Bundle id → which of its items are in.
    pub bundles: HashMap<u32, Vec<bool>>,
    pub raccoons_fed: u32,
    pub waivers: u32,
    /// Candles lit at Grandpa's shrine, 0 before the evaluation.
    pub grandpa_candles: u32,
}

/// One line of the perfection tracker.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Part {
    pub id: &'static str,
    /// Share of the 100% this part is worth.
    pub weight: f32,
    pub done: u32,
    /// `None` when it takes the catalog to know.
    pub total: Option<u32>,
    /// Points earned toward [`Self::weight`]; `None` alongside `total`.
    pub score: Option<f32>,
    pub missing: Vec<Missing>,
}

/// Something still to do, with progress where it has some.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Missing {
    pub key: String,
    pub name: String,
    /// For recipes: learned yet. For villagers and goals: how far along.
    pub have: Option<u32>,
    pub need: Option<u32>,
    pub known: Option<bool>,
    /// English page title on the Stardew wikis, for base game things.
    pub wiki: Option<String>,
}

impl Missing {
    fn named(key: &str, name: &str) -> Self {
        Self {
            key: key.to_owned(),
            name: name.to_owned(),
            have: None,
            need: None,
            known: None,
            wiki: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Perfection {
    /// 0–100, once every part has a total.
    pub percent: Option<f32>,
    /// The catalog used, if any.
    pub catalog: Option<CatalogInfo>,
    pub parts: Vec<Part>,
}

/// Scores `facts`, with totals from `catalog` where there is one.
pub fn evaluate(facts: &Facts, catalog: Option<&Catalog>) -> Perfection {
    let mut parts = Vec::new();

    parts.push(match catalog {
        Some(c) => listed(
            "shipping",
            15.0,
            &c.shipping,
            |i| facts.shipped.contains(&i.id),
            |i| Missing::named(&i.id, &i.name),
        ),
        None => unknown("shipping", 15.0, facts.shipped.len()),
    });

    let obelisks: Vec<Missing> = OBELISKS
        .iter()
        .filter(|o| !facts.buildings.contains(**o))
        .map(|o| Missing::named(o, building(catalog, o)))
        .collect();
    let built = (OBELISKS.len() - obelisks.len()) as u32;
    parts.push(Part {
        id: "obelisks",
        weight: 4.0,
        done: built,
        total: Some(OBELISKS.len() as u32),
        score: Some(built as f32),
        missing: obelisks,
    });

    let clock = facts.buildings.contains(GOLD_CLOCK);
    parts.push(flag(
        "clock",
        10.0,
        clock,
        GOLD_CLOCK,
        building(catalog, GOLD_CLOCK),
    ));

    parts.push(slayer(facts, catalog));

    parts.push(match catalog {
        Some(c) => {
            let need = |v: &CatalogVillager| if v.romance { 8 } else { 10 };
            let maxed = |v: &CatalogVillager| {
                facts.friends.get(&v.key).copied().unwrap_or(0) >= need(v) * 250
            };
            let mut part = listed("friends", 11.0, &c.villagers, maxed, |v| Missing {
                key: v.key.clone(),
                name: v.name.clone(),
                have: Some((facts.friends.get(&v.key).copied().unwrap_or(0).max(0) / 250) as u32),
                need: Some(need(v) as u32),
                known: None,
                wiki: None,
            });
            part.missing
                .sort_by_key(|m| std::cmp::Reverse(m.have.unwrap_or(0)));
            part
        }
        None => unknown("friends", 11.0, facts.friends.len()),
    });

    let level = (facts.skill_levels / 2).min(LEVEL_CAP);
    parts.push(Part {
        id: "level",
        weight: 5.0,
        done: level,
        total: Some(LEVEL_CAP),
        score: Some(level as f32 / LEVEL_CAP as f32 * 5.0),
        missing: Vec::new(),
    });

    parts.push(stardrops(facts));

    parts.push(match catalog {
        Some(c) => listed(
            "cooking",
            10.0,
            &c.cooking,
            // the game needs the recipe learned as well as its dish cooked
            |r| facts.cooking_known.contains(&r.key) && facts.cooked.contains(&r.output),
            |r| Missing {
                known: Some(facts.cooking_known.contains(&r.key)),
                ..Missing::named(&r.key, &r.name)
            },
        ),
        None => unknown("cooking", 10.0, facts.cooked.len()),
    });

    parts.push(match catalog {
        Some(c) => listed(
            "crafting",
            10.0,
            &c.crafting,
            |r| facts.crafted.get(&r.key).is_some_and(|n| *n > 0),
            |r| Missing {
                known: Some(facts.crafted.contains_key(&r.key)),
                ..Missing::named(&r.key, &r.name)
            },
        ),
        None => unknown(
            "crafting",
            10.0,
            facts.crafted.values().filter(|n| **n > 0).count(),
        ),
    });

    parts.push(match catalog {
        Some(c) => listed(
            "fish",
            10.0,
            &c.fish,
            |f| facts.fish.contains(&f.id),
            |f| Missing::named(&f.id, &f.name),
        ),
        None => unknown("fish", 10.0, facts.fish.len()),
    });

    let walnuts = facts.walnuts.min(WALNUTS);
    parts.push(Part {
        id: "walnuts",
        weight: 5.0,
        done: walnuts,
        total: Some(WALNUTS),
        score: Some(walnuts as f32 / WALNUTS as f32 * 5.0),
        missing: Vec::new(),
    });

    let percent = parts
        .iter()
        .map(|p| p.score)
        .sum::<Option<f32>>()
        .map(|s| (s * 10.0).round() / 10.0);
    for part in &mut parts {
        for m in &mut part.missing {
            m.wiki = match part.id {
                "shipping" | "fish" | "friends" => wiki_title(&m.key),
                // recipe keys, obelisks and the clock are the English names the pages use
                "cooking" | "crafting" | "obelisks" | "clock" => Some(m.key.clone()),
                "slayer" => Some("Adventurer's Guild".to_owned()),
                _ => None,
            };
        }
    }
    Perfection {
        percent,
        catalog: catalog.map(Catalog::info),
        parts,
    }
}

fn listed<T>(
    id: &'static str,
    weight: f32,
    all: &[T],
    done: impl Fn(&T) -> bool,
    missing: impl Fn(&T) -> Missing,
) -> Part {
    let mut have = 0u32;
    let mut rest = Vec::new();
    for item in all {
        if done(item) {
            have += 1;
        } else {
            rest.push(missing(item));
        }
    }
    let total = all.len() as u32;
    Part {
        id,
        weight,
        done: have,
        total: Some(total),
        score: Some(if total == 0 {
            weight
        } else {
            have as f32 / total as f32 * weight
        }),
        missing: rest,
    }
}

fn unknown(id: &'static str, weight: f32, done: usize) -> Part {
    Part {
        id,
        weight,
        done: done as u32,
        total: None,
        score: None,
        missing: Vec::new(),
    }
}

// a building's name in the game's language, or its data key for the UI to translate
fn building<'a>(catalog: Option<&'a Catalog>, key: &'a str) -> &'a str {
    catalog
        .and_then(|c| c.term(&format!("building.{key}")))
        .unwrap_or(key)
}

fn flag(id: &'static str, weight: f32, done: bool, what: &str, name: &str) -> Part {
    Part {
        id,
        weight,
        done: done as u32,
        total: Some(1),
        score: Some(if done { weight } else { 0.0 }),
        missing: if done {
            Vec::new()
        } else {
            vec![Missing::named(what, name)]
        },
    }
}

fn slayer(facts: &Facts, catalog: Option<&Catalog>) -> Part {
    let goals = catalog.map(|c| c.monsters.as_slice()).unwrap_or_default();
    let killed = |g: &CatalogGoal| -> u32 {
        g.targets
            .iter()
            .map(|t| facts.kills.get(t).copied().unwrap_or(0))
            .sum()
    };
    let missing: Vec<Missing> = goals
        .iter()
        .filter(|g| killed(g) < g.count)
        .map(|g| Missing {
            have: Some(killed(g)),
            need: Some(g.count),
            ..Missing::named(&g.key, &g.name)
        })
        .collect();
    // the save's own flag decides the score, as in the game; goals only explain it
    let total = goals.len().max(1) as u32;
    let done = match (facts.slayer_done, goals.is_empty()) {
        (true, _) => total,
        (false, true) => 0,
        (false, false) => total - missing.len() as u32,
    };
    Part {
        id: "slayer",
        weight: 10.0,
        done,
        total: Some(total),
        score: Some(if facts.slayer_done { 10.0 } else { 0.0 }),
        missing: if facts.slayer_done {
            Vec::new()
        } else {
            missing
        },
    }
}

fn stardrops(facts: &Facts) -> Part {
    let has =
        |flag: &str| facts.mail.contains(flag) || (flag == "CF_Mines" && facts.mine_100_chest);
    // the max-stamina flag covers players who ate all seven before 1.6 tracked them
    let all = facts.mail.contains("gotMaxStamina");
    let missing: Vec<Missing> = STARDROPS
        .iter()
        .filter(|f| !all && !has(f))
        .map(|f| Missing::named(f, f))
        .collect();
    let done = (STARDROPS.len() - missing.len()) as u32;
    Part {
        id: "stardrops",
        weight: 10.0,
        done,
        total: Some(STARDROPS.len() as u32),
        score: Some(if missing.is_empty() { 10.0 } else { 0.0 }),
        missing,
    }
}

/// Streams the main save file in `dir` for the perfection facts.
pub fn facts(dir: &Path) -> AppResult<Facts> {
    let id = dir
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .ok_or_else(|| AppError::NotFound(dir.display().to_string()))?;
    let file = std::fs::File::open(dir.join(&id))?;
    facts_from(std::io::BufReader::with_capacity(1 << 16, file))
}

// everything the checkup reads beyond perfection; `key` carries a dictionary key from
// its <key> to the matching <value>
fn collect_more(
    f: &mut Facts,
    path: &[String],
    t: &str,
    key: &mut Option<String>,
    location: Option<&str>,
) {
    let at = |i: usize, name: &str| path.get(i).is_some_and(|p| p == name);
    let n = path.len();
    let num = || t.parse::<u64>().unwrap_or(0);
    if at(1, "player") {
        match n {
            3 => match path[2].as_str() {
                "totalMoneyEarned" => f.money_earned = num(),
                "farmingLevel" => f.levels[0] = num() as u32,
                "fishingLevel" => f.levels[1] = num() as u32,
                "foragingLevel" => f.levels[2] = num() as u32,
                "miningLevel" => f.levels[3] = num() as u32,
                "combatLevel" => f.levels[4] = num() as u32,
                "luckLevel" => f.levels[5] = num() as u32,
                "houseUpgradeLevel" => f.house_level = num() as u32,
                "deepestMineLevel" => f.deepest_mine = num() as u32,
                "spouse" if !t.is_empty() => f.spouse = Some(t.to_owned()),
                _ => {}
            },
            4 => match path[2].as_str() {
                "experiencePoints" => f.xp.push(num() as u32),
                "achievements" => {
                    f.achievements.insert(num() as u32);
                }
                "eventsSeen" => {
                    f.events.insert(t.to_owned());
                }
                "secretNotesSeen" => {
                    f.notes.insert(num() as u32);
                }
                _ => {}
            },
            6 if at(3, "item") && at(4, "key") => match path[2].as_str() {
                "archaeologyFound" => {
                    f.artifacts_found.insert(t.to_owned());
                }
                "mineralsFound" => {
                    f.minerals_found.insert(t.to_owned());
                }
                _ => {}
            },
            6 if at(3, "item") && at(4, "value") && at(2, "basicShipped") => {
                if let Some(k) = key.as_ref() {
                    f.shipped_counts.insert(k.clone(), num() as u32);
                }
            }
            7 if at(2, "fishCaught") && at(4, "value") => {
                // [count, biggest size]; the first int is the count
                if let Some(k) = key.as_ref() {
                    f.fish_counts.entry(k.clone()).or_insert(num() as u32);
                }
            }
            7 if at(2, "friendshipData") && at(6, "Status") => {
                if let Some(k) = key.as_ref() {
                    f.friend_status.insert(k.clone(), t.to_owned());
                }
            }
            7 if at(2, "stats") && at(3, "Values") && at(5, "key") => *key = Some(t.to_owned()),
            7 if at(2, "stats") && at(3, "Values") && at(5, "value") => {
                if let Some(k) = key.as_ref() {
                    f.stats.insert(k.clone(), num());
                }
            }
            _ => {}
        }
        return;
    }
    match (n, path.get(1).map(String::as_str)) {
        (2, Some("timesFedRaccoons")) => f.raccoons_fed = num() as u32,
        (2, Some("perfectionWaivers")) => f.waivers = num() as u32,
        (3, Some("completedSpecialOrders")) => {
            f.special_orders.insert(t.to_owned());
        }
        (5, Some("bundleData")) if at(3, "key") => *key = Some(t.to_owned()),
        (5, Some("bundleData")) if at(3, "value") => {
            if let Some(k) = key.take() {
                f.bundle_data.push((k, t.to_owned()));
            }
        }
        _ => {}
    }
    if at(1, "locations") {
        match (location, n) {
            (Some("Farm"), 4) if at(3, "grandpaScore") => f.grandpa_candles = num() as u32,
            (Some("CommunityCenter"), 7) if at(3, "bundles") && at(5, "key") => {
                *key = Some(t.to_owned())
            }
            (Some("CommunityCenter"), 8) if at(3, "bundles") && at(6, "ArrayOfBoolean") => {
                if let Some(id) = key.as_ref().and_then(|k| k.parse().ok()) {
                    f.bundles.entry(id).or_default().push(t == "true");
                }
            }
            (Some("LibraryMuseum"), 7) if at(3, "museumPieces") && at(5, "value") => {
                f.donated.insert(t.trim_start_matches("(O)").to_owned());
            }
            _ => {}
        }
    }
}

#[derive(Default)]
struct Building {
    depth: usize,
    kind: Option<String>,
    days_left: u32,
}

fn facts_from(reader: impl BufRead) -> AppResult<Facts> {
    let mut xml = quick_xml::Reader::from_reader(reader);
    let mut buf = Vec::new();
    let mut path: Vec<String> = Vec::new();
    let mut text = String::new();
    let mut f = Facts::default();
    // dictionary entries are <item><key>…</key><value>…</value></item>; the key comes first
    let mut key: Option<String> = None;
    let mut buildings: Vec<Building> = Vec::new();
    // xsi:type of the <GameLocation> we're inside, if any
    let mut location: Option<String> = None;
    let type_of = |e: &quick_xml::events::BytesStart<'_>| {
        e.attributes()
            .flatten()
            .find(|a| a.key.local_name().as_ref() == "type")
            .map(|a| a.value.to_string())
    };

    let ends = |path: &[String], tail: &[&str]| {
        path.len() >= tail.len()
            && path[path.len() - tail.len()..]
                .iter()
                .zip(tail)
                .all(|(a, b)| a == b)
    };

    loop {
        match xml
            .read_event_into(&mut buf)
            .map_err(|e| AppError::BadSave(e.to_string()))?
        {
            Event::Start(e) => {
                let name = e.local_name().as_ref().to_owned();
                if name == "GameLocation" && path.len() == 2 {
                    location = type_of(&e);
                }
                if name == "NPC" && type_of(&e).as_deref() == Some("Child") {
                    f.children += 1;
                }
                if name == "Building" && path.last().is_some_and(|p| p == "buildings") {
                    buildings.push(Building {
                        depth: path.len() + 1,
                        ..Building::default()
                    });
                }
                path.push(name);
                text.clear();
            }
            Event::Text(t) => text.push_str(&t.xml10_content()),
            Event::GeneralRef(r) => text.push_str(&crate::saves::entity(&r)),
            Event::End(_) => {
                let t = text.trim();
                // only the main player counts; farmhands live under <farmhands>
                let player = path.len() >= 3 && path[1] == "player";
                collect_more(&mut f, &path, t, &mut key, location.as_deref());
                if path.len() == 3 && path[1] == "locations" {
                    location = None;
                }
                if player && path.len() == 6 && path[3] == "item" && path[4] == "key" {
                    key = Some(t.to_owned());
                    let set = match path[2].as_str() {
                        "basicShipped" => Some(&mut f.shipped),
                        "cookingRecipes" => Some(&mut f.cooking_known),
                        "recipesCooked" => Some(&mut f.cooked),
                        "fishCaught" => Some(&mut f.fish),
                        _ => None,
                    };
                    if let Some(set) = set {
                        set.insert(t.to_owned());
                    }
                    if path[2] == "craftingRecipes" {
                        f.crafted.insert(t.to_owned(), 0);
                    }
                }
                if player
                    && path.len() == 6
                    && path[3] == "item"
                    && path[4] == "value"
                    && let Some(k) = key.as_ref()
                {
                    match path[2].as_str() {
                        "craftingRecipes" => {
                            f.crafted.insert(k.clone(), t.parse().unwrap_or(0));
                        }
                        "chestConsumedMineLevels" if k == "100" && t == "true" => {
                            f.mine_100_chest = true;
                        }
                        _ => {}
                    }
                }
                if player
                    && path.len() == 4
                    && matches!(path[2].as_str(), "mailReceived" | "mailForTomorrow")
                {
                    // tomorrow's mail can carry a "%&NL&%" no-letter marker
                    f.mail.insert(t.trim_end_matches("%&NL&%").to_owned());
                }
                if player && path.len() == 3 {
                    match path[2].as_str() {
                        "farmingLevel" | "miningLevel" | "foragingLevel" | "fishingLevel"
                        | "combatLevel" | "luckLevel" => {
                            f.skill_levels += t.parse::<u32>().unwrap_or(0);
                        }
                        "hasCompletedAllMonsterSlayerQuests" => f.slayer_done = t == "true",
                        _ => {}
                    }
                }
                if player
                    && path.len() == 7
                    && ends(
                        &path,
                        &["friendshipData", "item", "value", "Friendship", "Points"],
                    )
                    && let (Some(k), Ok(points)) = (key.as_ref(), t.parse())
                {
                    f.friends.insert(k.clone(), points);
                }
                if player
                    && path.len() == 7
                    && ends(
                        &path,
                        &["stats", "specificMonstersKilled", "item", "key", "string"],
                    )
                {
                    key = Some(t.to_owned());
                }
                if player
                    && path.len() == 7
                    && ends(
                        &path,
                        &["stats", "specificMonstersKilled", "item", "value", "int"],
                    )
                    && let Some(k) = key.as_ref()
                {
                    f.kills.insert(k.clone(), t.parse().unwrap_or(0));
                }
                if path.len() == 2 && path[1] == "goldenWalnutsFound" {
                    f.walnuts = t.parse().unwrap_or(0);
                }
                if let Some(b) = buildings.last_mut() {
                    if path.len() == b.depth + 1 {
                        match path[b.depth].as_str() {
                            "buildingType" => b.kind = Some(t.to_owned()),
                            "daysOfConstructionLeft" => b.days_left = t.parse().unwrap_or(0),
                            _ => {}
                        }
                    }
                    if path.len() == b.depth {
                        let b = buildings.pop().expect("checked above");
                        if let (Some(kind), 0) = (b.kind, b.days_left) {
                            f.buildings.insert(kind);
                        }
                    }
                }
                path.pop();
                text.clear();
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(f)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const SAVE: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<SaveGame xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
  <player>
    <farmingLevel>10</farmingLevel><miningLevel>10</miningLevel><foragingLevel>10</foragingLevel>
    <fishingLevel>10</fishingLevel><combatLevel>8</combatLevel><luckLevel>0</luckLevel>
    <mailReceived><string>CF_Fair</string><string>CF_Fish</string></mailReceived>
    <mailForTomorrow><string>CF_Sewer%&amp;NL&amp;%</string></mailForTomorrow>
    <basicShipped><item><key><string>24</string></key><value><int>5</int></value></item></basicShipped>
    <fishCaught><item><key><string>(O)128</string></key><value><ArrayOfInt><int>1</int></ArrayOfInt></value></item></fishCaught>
    <cookingRecipes>
      <item><key><string>Fried Egg</string></key><value><int>0</int></value></item>
      <item><key><string>Omelet</string></key><value><int>0</int></value></item>
    </cookingRecipes>
    <recipesCooked><item><key><string>194</string></key><value><int>3</int></value></item></recipesCooked>
    <craftingRecipes>
      <item><key><string>Chest</string></key><value><int>2</int></value></item>
      <item><key><string>Torch</string></key><value><int>0</int></value></item>
    </craftingRecipes>
    <chestConsumedMineLevels><item><key><int>100</int></key><value><boolean>true</boolean></value></item></chestConsumedMineLevels>
    <friendshipData>
      <item><key><string>Abigail</string></key><value><Friendship><Points>2000</Points></Friendship></value></item>
      <item><key><string>Pierre</string></key><value><Friendship><Points>2400</Points></Friendship></value></item>
    </friendshipData>
    <stats><specificMonstersKilled>
      <item><key><string>Green Slime</string></key><value><int>700</int></value></item>
      <item><key><string>Bat</string></key><value><int>20</int></value></item>
    </specificMonstersKilled></stats>
    <hasCompletedAllMonsterSlayerQuests>false</hasCompletedAllMonsterSlayerQuests>
  </player>
  <farmhands><Farmer><basicShipped><item><key><string>999</string></key></item></basicShipped></Farmer></farmhands>
  <locations>
    <GameLocation xsi:type="Farm">
      <buildings>
        <Building><buildingType>Earth Obelisk</buildingType><daysOfConstructionLeft>0</daysOfConstructionLeft></Building>
        <Building><buildingType>Gold Clock</buildingType><daysOfConstructionLeft>2</daysOfConstructionLeft></Building>
        <Building><buildingType>Barn</buildingType><indoors><buildings/></indoors><daysOfConstructionLeft>0</daysOfConstructionLeft></Building>
      </buildings>
    </GameLocation>
  </locations>
  <goldenWalnutsFound>65</goldenWalnutsFound>
</SaveGame>"#;

    #[test]
    fn builtin_catalog_covers_the_base_game() {
        let c = Catalog::builtin("en");
        assert!(c.builtin);
        assert_eq!(Catalog::builtin("xx"), c);
        let ru = Catalog::builtin("ru");
        assert_eq!(ru.language, "ru");
        assert!(ru.items.iter().any(|i| i.name == "Пастернак"));
        assert_eq!(c.museum.len(), 95);
        assert!(c.achievements.len() >= 39 && c.items.len() > 800);
        assert!(c.items.iter().any(|i| i.id == "Book_Trash"));
    }

    fn catalog() -> Catalog {
        let item = |id: &str, name: &str| CatalogItem {
            id: id.into(),
            name: name.into(),
        };
        let recipe = |key: &str, output: &str| CatalogRecipe {
            key: key.into(),
            name: key.into(),
            output: output.into(),
        };
        Catalog {
            game: "1.6.15".into(),
            language: "en".into(),
            shipping: vec![item("24", "Parsnip"), item("188", "Green Bean")],
            fish: vec![item("(O)128", "Pufferfish"), item("(O)129", "Anchovy")],
            cooking: vec![
                recipe("Fried Egg", "194"),
                recipe("Omelet", "195"),
                recipe("Salad", "196"),
            ],
            crafting: vec![recipe("Chest", "130"), recipe("Torch", "93")],
            villagers: vec![
                CatalogVillager {
                    key: "Abigail".into(),
                    name: "Abigail".into(),
                    romance: true,
                    birth_season: None,
                    birth_day: 0,
                    loves: Vec::new(),
                },
                CatalogVillager {
                    key: "Pierre".into(),
                    name: "Pierre".into(),
                    romance: false,
                    birth_season: None,
                    birth_day: 0,
                    loves: Vec::new(),
                },
            ],
            monsters: vec![
                CatalogGoal {
                    key: "Slimes".into(),
                    name: "Slimes".into(),
                    count: 1000,
                    targets: vec!["Green Slime".into(), "Frost Jelly".into()],
                },
                CatalogGoal {
                    key: "Bats".into(),
                    name: "Bats".into(),
                    count: 200,
                    targets: vec!["Bat".into()],
                },
            ],
            ..Catalog::default()
        }
    }

    fn part<'a>(p: &'a Perfection, id: &str) -> &'a Part {
        p.parts.iter().find(|x| x.id == id).unwrap()
    }

    #[test]
    fn reads_facts_for_the_main_player() {
        let f = facts_from(SAVE.as_bytes()).unwrap();
        assert_eq!(f.shipped, HashSet::from(["24".to_owned()]));
        assert_eq!(f.crafted["Chest"], 2);
        assert_eq!(f.crafted["Torch"], 0);
        assert!(f.mail.contains("CF_Sewer"));
        assert!(f.mine_100_chest);
        assert_eq!(f.friends["Pierre"], 2400);
        assert_eq!(f.kills["Green Slime"], 700);
        assert_eq!(f.skill_levels, 48);
        assert_eq!(f.walnuts, 65);
        assert_eq!(
            f.buildings,
            HashSet::from(["Earth Obelisk".to_owned(), "Barn".to_owned()])
        );
    }

    #[test]
    fn scores_like_the_game() {
        let f = facts_from(SAVE.as_bytes()).unwrap();
        let p = evaluate(&f, Some(&catalog()));
        let shipping = part(&p, "shipping");
        assert_eq!((shipping.done, shipping.total), (1, Some(2)));
        assert_eq!(shipping.missing[0].name, "Green Bean");
        assert_eq!(part(&p, "obelisks").done, 1);
        assert_eq!(part(&p, "clock").score, Some(0.0));
        let friends = part(&p, "friends");
        assert_eq!(
            friends.done, 1,
            "Abigail maxes at 8 hearts, Pierre needs 10"
        );
        assert_eq!(friends.missing[0].have, Some(9));
        let cooking = part(&p, "cooking");
        assert_eq!(cooking.done, 1);
        assert_eq!(cooking.missing[0].known, Some(true));
        assert_eq!(cooking.missing[1].known, Some(false));
        assert_eq!(part(&p, "crafting").done, 1);
        assert_eq!(part(&p, "level").done, 24);
        let drops = part(&p, "stardrops");
        assert_eq!(
            drops.done, 4,
            "fair, fish, sewer tomorrow, mines by its chest"
        );
        assert_eq!(drops.score, Some(0.0));
        let slayer = part(&p, "slayer");
        assert_eq!(slayer.missing.len(), 2);
        assert_eq!(slayer.missing[0].have, Some(700));
        assert_eq!(part(&p, "walnuts").score, Some(2.5));
        // 7.5 + 1 + 0 + 0 + 5.5 + 4.8 + 0 + 3.333 + 5 + 5 + 2.5
        assert_eq!(p.percent, Some(34.6));
    }

    #[test]
    fn reports_counts_without_a_catalog() {
        let f = facts_from(SAVE.as_bytes()).unwrap();
        let p = evaluate(&f, None);
        assert_eq!(p.percent, None);
        assert_eq!(part(&p, "fish").total, None);
        assert_eq!(part(&p, "fish").done, 1);
        assert_eq!(part(&p, "walnuts").total, Some(130));
    }

    // a real save, read-only: PELICAN_TEST_SAVE=<save folder> cargo test -- --ignored real_save_facts --nocapture
    #[test]
    #[ignore]
    fn real_save_facts() {
        let dir = PathBuf::from(std::env::var("PELICAN_TEST_SAVE").unwrap());
        let f = facts(&dir).unwrap();
        eprintln!(
            "money {} xp {:?} levels {:?} house {} spouse {:?} children {} mine {}",
            f.money_earned, f.xp, f.levels, f.house_level, f.spouse, f.children, f.deepest_mine
        );
        eprintln!(
            "achievements {} events {} notes {} donated {} artifacts {} minerals {}",
            f.achievements.len(),
            f.events.len(),
            f.notes.len(),
            f.donated.len(),
            f.artifacts_found.len(),
            f.minerals_found.len()
        );
        eprintln!(
            "shipped {} fish {} friends {} statuses {} orders {} bundles {}/{} raccoons {} candles {}",
            f.shipped_counts.len(),
            f.fish_counts.len(),
            f.friends.len(),
            f.friend_status.len(),
            f.special_orders.len(),
            f.bundle_data.len(),
            f.bundles.len(),
            f.raccoons_fed,
            f.grandpa_candles
        );
        let mut stats: Vec<_> = f.stats.iter().collect();
        stats.sort();
        eprintln!("stats {stats:?}");
        assert_eq!(f.xp.len(), 6);
        assert!(!f.bundle_data.is_empty() && !f.bundles.is_empty());
    }
}
