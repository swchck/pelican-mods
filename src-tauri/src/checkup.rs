//! The detailed progress report, after Stardew Checkup: every achievement and milestone
//! a save shows progress on, down to the missing bundle items and museum pieces.
//!
//! Rules follow the 1.6 game code (`Stats.checkFor*Achievements`, `Utility`,
//! `AdventureGuild`, `CommunityCenter`), not Checkup's own tables. Names of things come
//! from the game through the companion's [`Catalog`]; fixed texts (milestones, rooms,
//! island upgrades) are labels the UI translates.

use std::collections::HashMap;

use serde::Serialize;

use crate::perfection::{Catalog, Facts};

/// Every section, in display order.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Checkup {
    pub sections: Vec<Section>,
    /// Whether the game's lists were there; without them name lists stay empty.
    pub catalog: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Section {
    pub id: &'static str,
    pub done: u32,
    pub total: u32,
    pub groups: Vec<Group>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    /// Also the translation key when `title` is empty.
    pub id: String,
    /// A name from the game (a bundle, a room), when the group has one.
    pub title: Option<String>,
    /// Translation key of a heading above the group, such as a Community Center room.
    pub category: Option<String>,
    pub done: u32,
    pub total: u32,
    pub items: Vec<Item>,
}

/// One thing to have or do.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub id: String,
    /// A name from the game; otherwise the UI translates `label` with `have`/`need`.
    pub name: Option<String>,
    pub label: Option<&'static str>,
    pub have: Option<u64>,
    pub need: Option<u64>,
    pub done: bool,
    /// Translation key of a short remark ("found, not donated", "gold quality"…).
    pub note: Option<&'static str>,
}

impl Item {
    fn named(id: impl Into<String>, name: impl Into<String>, done: bool) -> Self {
        Self {
            id: id.into(),
            name: Some(name.into()),
            label: None,
            have: None,
            need: None,
            done,
            note: None,
        }
    }

    fn label(label: &'static str, done: bool) -> Self {
        Self {
            id: label.to_owned(),
            name: None,
            label: Some(label),
            have: None,
            need: None,
            done,
            note: None,
        }
    }

    /// A count toward a target, done once it reaches it.
    fn goal(label: &'static str, have: u64, need: u64) -> Self {
        Self {
            id: format!("{label}-{need}"),
            have: Some(have),
            need: Some(need),
            ..Self::label(label, have >= need)
        }
    }

    fn counted(mut self, have: u64, need: u64) -> Self {
        self.have = Some(have);
        self.need = Some(need);
        self
    }

    fn with_note(mut self, note: &'static str) -> Self {
        self.note = Some(note);
        self
    }
}

fn group(id: impl Into<String>, items: Vec<Item>) -> Group {
    Group {
        id: id.into(),
        title: None,
        category: None,
        done: items.iter().filter(|i| i.done).count() as u32,
        total: items.len() as u32,
        items,
    }
}

fn section(id: &'static str, groups: Vec<Group>) -> Section {
    let groups: Vec<Group> = groups.into_iter().filter(|g| g.total > 0).collect();
    Section {
        id,
        done: groups.iter().map(|g| g.done).sum(),
        total: groups.iter().map(|g| g.total).sum(),
        groups,
    }
}

/// Builds the report for `facts`, with names and totals from `catalog`.
pub fn build(facts: &Facts, catalog: Option<&Catalog>) -> Checkup {
    let empty = Catalog::default();
    let c = catalog.unwrap_or(&empty);
    let names: HashMap<&str, &str> = c
        .items
        .iter()
        .map(|i| (i.id.as_str(), i.name.as_str()))
        .collect();
    Checkup {
        sections: vec![
            achievements(facts, c),
            skills(facts),
            community_center(facts, &names),
            museum(facts, c),
            friends(facts, c),
            collections(facts, c),
            quests(facts, c),
            monsters(facts, c),
            island(facts),
            secrets(facts, &names),
            grandpa(facts),
            money(facts),
        ],
        catalog: catalog.is_some(),
    }
}

fn money(f: &Facts) -> Section {
    let earned = f.money_earned;
    section(
        "money",
        vec![group(
            "earned",
            [15_000, 50_000, 250_000, 1_000_000, 10_000_000]
                .into_iter()
                .map(|need| Item::goal("earn", earned, need))
                .collect(),
        )],
    )
}

/// Experience needed for each skill level, 1–10.
const SKILL_XP: [u32; 10] = [100, 380, 770, 1300, 2150, 3300, 4800, 6900, 10000, 15000];
/// Mastery experience for each of the five mastery levels, cumulative.
const MASTERY_XP: [u64; 5] = [10_000, 25_000, 45_000, 70_000, 100_000];
const SKILLS: [&str; 5] = ["farming", "fishing", "foraging", "mining", "combat"];

fn skills(f: &Facts) -> Section {
    let mut levels = Vec::new();
    for (i, skill) in SKILLS.iter().enumerate() {
        let xp = f.xp.get(i).copied().unwrap_or(0);
        let next = SKILL_XP
            .iter()
            .copied()
            .find(|t| *t > xp)
            .unwrap_or(SKILL_XP[9]);
        levels.push(Item {
            id: (*skill).to_owned(),
            ..Item::label(skill_label(skill), f.levels[i] >= 10).counted(xp as u64, next as u64)
        });
    }
    let farmer = (f.skill_levels / 2) as u64;
    let mastery_xp = f.stats.get("MasteryExp").copied().unwrap_or(0);
    let mastery_level = MASTERY_XP.iter().filter(|t| mastery_xp >= **t).count() as u64;
    let mut mastery = vec![Item::goal("masteryLevel", mastery_level, 5)];
    for (i, skill) in SKILLS.iter().enumerate() {
        let done = f.stats.get(&format!("mastery_{i}")).is_some_and(|v| *v > 0);
        mastery.push(Item {
            id: format!("mastery_{skill}"),
            ..Item::label(mastery_label(skill), done)
        });
    }
    section(
        "skills",
        vec![
            group("levels", levels),
            group(
                "farmer",
                vec![
                    Item::goal("farmerLevel", farmer, 25),
                    Item::goal(
                        "singularTalent",
                        f.levels[..5].iter().filter(|l| **l >= 10).count().min(1) as u64,
                        1,
                    ),
                    Item::goal(
                        "fiveWays",
                        f.levels[..5].iter().filter(|l| **l >= 10).count() as u64,
                        5,
                    ),
                ],
            ),
            group("mastery", mastery),
        ],
    )
}

fn skill_label(skill: &str) -> &'static str {
    match skill {
        "farming" => "skillFarming",
        "fishing" => "skillFishing",
        "foraging" => "skillForaging",
        "mining" => "skillMining",
        _ => "skillCombat",
    }
}

fn mastery_label(skill: &str) -> &'static str {
    match skill {
        "farming" => "masteryFarming",
        "fishing" => "masteryFishing",
        "foraging" => "masteryForaging",
        "mining" => "masteryMining",
        _ => "masteryCombat",
    }
}

/// Room prefixes in `bundleData` keys, with the mail flags that mark them done.
const ROOMS: [(&str, &str, &str); 7] = [
    ("Pantry", "ccPantry", "jojaPantry"),
    ("Crafts Room", "ccCraftsRoom", "jojaCraftsRoom"),
    ("Fish Tank", "ccFishTank", "jojaFishTank"),
    ("Boiler Room", "ccBoilerRoom", "jojaBoilerRoom"),
    ("Vault", "ccVault", "jojaVault"),
    ("Bulletin Board", "ccBulletin", "ccBulletin"),
    ("Abandoned Joja Mart", "ccMovieTheater", "ccMovieTheater"),
];

fn community_center(f: &Facts, names: &HashMap<&str, &str>) -> Section {
    let mut groups = Vec::new();
    for (room, cc_flag, joja_flag) in ROOMS {
        let room_done = f.mail.contains(cc_flag) || f.mail.contains(joja_flag);
        let mut bundles: Vec<(u32, &str)> = f
            .bundle_data
            .iter()
            .filter_map(|(key, value)| {
                let (r, id) = key.split_once('/')?;
                (r == room).then(|| Some((id.parse().ok()?, value.as_str())))?
            })
            .collect();
        bundles.sort_by_key(|(id, _)| *id);
        for (id, def) in bundles {
            let mut g = bundle(id, def, f.bundles.get(&id), room_done, names);
            g.category = Some(room.replace(' ', ""));
            groups.push(g);
        }
    }
    let mut s = section("cc", groups);
    // a bundle needs `total` of its items (fewer than all for "any N of these"), so the
    // section counts bundles, not items
    s.done = s.groups.iter().filter(|g| g.done >= g.total).count() as u32;
    s.total = s.groups.len() as u32;
    s
}

// "name/reward/items/colour/needed/sprite/display name", items as "id count quality" triples
fn bundle(
    id: u32,
    def: &str,
    state: Option<&Vec<bool>>,
    room_done: bool,
    names: &HashMap<&str, &str>,
) -> Group {
    let fields: Vec<&str> = def.split('/').collect();
    let title = fields
        .get(6)
        .filter(|s| !s.is_empty())
        .or(fields.first())
        .map(|s| (*s).to_owned());
    let triples: Vec<&str> = fields
        .get(2)
        .map(|s| s.split_whitespace().collect())
        .unwrap_or_default();
    let mut items = Vec::new();
    for (i, t) in triples.chunks(3).enumerate() {
        let [item, count, quality] = t else { continue };
        let count: u64 = count.parse().unwrap_or(1);
        let done = room_done || state.and_then(|s| s.get(i)).copied().unwrap_or(false);
        let mut it = if *item == "-1" {
            Item {
                id: format!("{id}-{i}"),
                need: Some(count),
                ..Item::label("gold", done)
            }
        } else {
            let name = names.get(item).copied().unwrap_or(item);
            Item {
                need: (count > 1).then_some(count),
                ..Item::named(format!("{id}-{i}"), name, done)
            }
        };
        it.note = match *quality {
            "1" => Some("silver"),
            "2" => Some("gold"),
            "4" => Some("iridium"),
            _ => None,
        };
        items.push(it);
    }
    let needed = fields
        .get(4)
        .and_then(|s| s.parse::<u64>().ok())
        .filter(|n| *n > 0 && (*n as usize) < items.len());
    let mut g = group(format!("bundle-{id}"), items);
    g.title = title;
    if let Some(n) = needed {
        // "any N of these": the UI shows it as a goal above the list
        g.items.insert(
            0,
            Item {
                id: format!("{id}-needs"),
                ..Item::goal("bundleNeeds", g.done as u64, n)
            },
        );
        g.done = g.done.min(n as u32);
        g.total = n as u32;
    }
    g
}

fn museum(f: &Facts, c: &Catalog) -> Section {
    let mut artifacts = Vec::new();
    let mut minerals = Vec::new();
    for m in &c.museum {
        let donated = f.donated.contains(&m.id);
        let found = f.artifacts_found.contains(&m.id) || f.minerals_found.contains(&m.id);
        let mut item = Item::named(m.id.clone(), m.name.clone(), donated);
        if !donated && found {
            item = item.with_note("foundNotDonated");
        }
        if m.kind == "artifact" {
            artifacts.push(item);
        } else {
            minerals.push(item);
        }
    }
    let donated = f.donated.len() as u64;
    let total = c.museum.len().max(1) as u64;
    section(
        "museum",
        vec![
            group(
                "museumGoals",
                vec![
                    Item::goal("treasureTrove", donated, 40),
                    Item::goal("rustyKey", donated, 60),
                    Item::goal("completeCollection", donated, total),
                ],
            ),
            group("artifacts", artifacts),
            group("minerals", minerals),
        ],
    )
}

fn friends(f: &Facts, c: &Catalog) -> Section {
    let mut villagers: Vec<Item> = c
        .villagers
        .iter()
        .map(|v| {
            let points = f.friends.get(&v.key).copied().unwrap_or(0).max(0) as u64;
            let status = f.friend_status.get(&v.key).map(String::as_str);
            // 14 for a spouse, 10 when dating opens the cap, 8 for datables otherwise
            let max = match status {
                Some("Married") => 14,
                Some("Dating") | Some("Engaged") => 10,
                _ if v.romance => 8,
                _ => 10,
            };
            let mut item = Item::named(v.key.clone(), v.name.clone(), points / 250 >= max)
                .counted(points / 250, max);
            item.note = match status {
                Some("Married") => Some("married"),
                Some("Dating") => Some("dating"),
                Some("Engaged") => Some("engaged"),
                _ if !f.friends.contains_key(&v.key) => Some("notMet"),
                _ => None,
            };
            item
        })
        .collect();
    villagers.sort_by_key(|i| (i.done, std::cmp::Reverse(i.have)));
    let five = f.friends.values().filter(|p| **p >= 1250).count() as u64;
    let ten = f.friends.values().filter(|p| **p >= 2500).count() as u64;
    let family = vec![
        Item::goal("houseUpgrade", f.house_level as u64, 1),
        Item::goal("houseUpgrade", f.house_level as u64, 2),
        Item::goal("houseUpgrade", f.house_level as u64, 3),
        Item::label("spouse", f.spouse.is_some()),
        Item::goal(
            "fullHouse",
            f.children as u64 + f.spouse.is_some() as u64,
            3,
        ),
    ];
    section(
        "friends",
        vec![
            group(
                "friendGoals",
                vec![
                    Item::goal("fiveHeartFriends", five, 1),
                    Item::goal("fiveHeartFriends", five, 4),
                    Item::goal("fiveHeartFriends", five, 10),
                    Item::goal("fiveHeartFriends", five, 20),
                    Item::goal("tenHeartFriends", ten, 1),
                    Item::goal("tenHeartFriends", ten, 8),
                ],
            ),
            group("villagers", villagers),
            group("family", family),
        ],
    )
}

fn collections(f: &Facts, c: &Catalog) -> Section {
    let fish_types = f.fish.len() as u64;
    let fish_total: u64 = f.fish_counts.values().map(|n| *n as u64).sum();
    let cooked = c
        .cooking
        .iter()
        .filter(|r| f.cooking_known.contains(&r.key) && f.cooked.contains(&r.output))
        .count() as u64;
    let crafted = f.crafted.values().filter(|n| **n > 0).count() as u64;
    let shipped = |id: &str| f.shipped_counts.get(id).copied().unwrap_or(0) as u64;
    let mut poly: Vec<Item> = c
        .crops
        .iter()
        .filter(|cr| cr.polyculture)
        .map(|cr| {
            let name = c
                .items
                .iter()
                .find(|i| i.id == cr.id)
                .map_or(cr.id.as_str(), |i| i.name.as_str());
            let have = shipped(&cr.id);
            Item::named(cr.id.clone(), name, have >= 15).counted(have, 15)
        })
        .collect();
    poly.sort_by_key(|i| (i.done, std::cmp::Reverse(i.have)));
    let mono = c
        .crops
        .iter()
        .filter(|cr| cr.monoculture)
        .map(|cr| shipped(&cr.id))
        .max()
        .unwrap_or(0);
    let all_or = |n: usize, fallback: u64| if n > 0 { n as u64 } else { fallback };
    section(
        "collections",
        vec![
            group(
                "fishGoals",
                vec![
                    Item::goal("fishTypes", fish_types, 10),
                    Item::goal("fishTypes", fish_types, 24),
                    Item::goal("fishTypes", fish_types, all_or(c.fish.len(), 72)),
                    Item::goal("fishCaught", fish_total, 100),
                ],
            ),
            group(
                "cookingGoals",
                vec![
                    Item::goal("dishesCooked", cooked, 10),
                    Item::goal("dishesCooked", cooked, 25),
                    Item::goal("dishesCooked", cooked, all_or(c.cooking.len(), 81)),
                ],
            ),
            group(
                "craftingGoals",
                vec![
                    Item::goal("itemsCrafted", crafted, 15),
                    Item::goal("itemsCrafted", crafted, 30),
                    Item::goal("itemsCrafted", crafted, all_or(c.crafting.len(), 149)),
                ],
            ),
            group(
                "shippingGoals",
                vec![
                    Item::goal(
                        "itemsShipped",
                        c.shipping.iter().filter(|i| shipped(&i.id) > 0).count() as u64,
                        all_or(c.shipping.len(), 155),
                    ),
                    Item::goal("monoculture", mono, 300),
                ],
            ),
            group("polyculture", poly),
        ],
    )
}

fn quests(f: &Facts, c: &Catalog) -> Section {
    let done_quests = f.stats.get("questsCompleted").copied().unwrap_or(0);
    let orders = |qi: bool| -> Vec<Item> {
        c.special_orders
            .iter()
            .filter(|o| o.qi == qi)
            .map(|o| {
                Item::named(
                    o.key.clone(),
                    o.name.clone(),
                    f.special_orders.contains(&o.key),
                )
            })
            .collect()
    };
    section(
        "quests",
        vec![
            group(
                "helpWanted",
                vec![
                    Item::goal("questsDone", done_quests, 10),
                    Item::goal("questsDone", done_quests, 40),
                ],
            ),
            group("townOrders", orders(false)),
            group("qiOrders", orders(true)),
            group(
                "raccoons",
                // the tenth feeding finishes the line; 1.6.4 counts the ninth once a week passed
                vec![Item::goal("raccoonsFed", f.raccoons_fed as u64, 10)],
            ),
        ],
    )
}

fn monsters(f: &Facts, c: &Catalog) -> Section {
    let goals: Vec<Item> = c
        .monsters
        .iter()
        .map(|g| {
            let have: u64 = g
                .targets
                .iter()
                .map(|t| f.kills.get(t).copied().unwrap_or(0) as u64)
                .sum();
            Item::named(g.key.clone(), g.name.clone(), have >= g.count as u64)
                .counted(have, g.count as u64)
        })
        .collect();
    section(
        "monsters",
        vec![
            group(
                "mines",
                vec![
                    Item::goal("mineBottom", f.deepest_mine as u64, 120),
                    Item::goal(
                        "monstersTotal",
                        f.stats.get("monstersKilled").copied().unwrap_or(0),
                        1000,
                    ),
                ],
            ),
            group("slayerGoals", goals),
        ],
    )
}

/// Ginger Island upgrades by mail flag, with their walnut cost.
const ISLAND_UPGRADES: [(&str, u64); 11] = [
    ("Island_FirstParrot", 1),
    ("Island_Turtle", 10),
    ("Island_UpgradeHouse", 20),
    ("Island_UpgradeHouse_Mailbox", 5),
    ("Island_UpgradeTrader", 10),
    ("Island_UpgradeBridge", 10),
    ("Island_UpgradeParrotPlatform", 10),
    ("Island_Resort", 20),
    ("Island_W_Obelisk", 20),
    ("Island_VolcanoBridge", 5),
    ("Island_VolcanoShortcutOut", 5),
];

fn island(f: &Facts) -> Section {
    let upgrades = ISLAND_UPGRADES
        .iter()
        .map(|(flag, cost)| Item {
            id: (*flag).to_owned(),
            name: None,
            label: Some(flag),
            have: None,
            need: Some(*cost),
            done: f.mail.contains(*flag),
            note: None,
        })
        .collect();
    // journal scraps are secret notes 1001–1011
    let scraps = (1001..=1011)
        .map(|n| Item {
            id: format!("scrap-{n}"),
            have: Some(n as u64 - 1000),
            ..Item::label("journalScrap", f.notes.contains(&n))
        })
        .collect();
    section(
        "island",
        vec![
            group(
                "walnuts",
                vec![
                    Item::label("visitedIsland", f.mail.contains("Visited_Island")),
                    Item::goal("walnutsFound", f.walnuts as u64, 130),
                ],
            ),
            group("islandUpgrades", upgrades),
            group("journalScraps", scraps),
        ],
    )
}

const BOOKS: [&str; 19] = [
    "Book_Trash",
    "Book_Crabbing",
    "Book_Bombs",
    "Book_Roe",
    "Book_WildSeeds",
    "Book_Woodcutting",
    "Book_Defense",
    "Book_Friendship",
    "Book_Void",
    "Book_Speed",
    "Book_Marlon",
    "Book_PriceCatalogue",
    "Book_Diamonds",
    "Book_Mystery",
    "Book_AnimalCatalogue",
    "Book_Speed2",
    "Book_Artifact",
    "Book_Horse",
    "Book_Grass",
];

/// Special items and powers by mail flag, as the 1.6 Powers tab lists them.
const POWERS: [&str; 10] = [
    "HasRustyKey",
    "HasSkullKey",
    "HasClubCard",
    "HasSpecialCharm",
    "HasDarkTalisman",
    "HasMagicInk",
    "HasMagnifyingGlass",
    "HasDwarvishTranslationGuide",
    "HasTownKey",
    "canReadJunimoText",
];

/// Secret notes in 1.6; journal scraps start at 1001.
const SECRET_NOTES: u32 = 27;

fn secrets(f: &Facts, names: &HashMap<&str, &str>) -> Section {
    let books = BOOKS
        .iter()
        .map(|b| {
            let read = f.stats.get(*b).is_some_and(|n| *n > 0);
            Item::named(*b, names.get(b).copied().unwrap_or(b), read)
        })
        .collect();
    let mut powers: Vec<Item> = POWERS
        .iter()
        .map(|p| Item {
            id: (*p).to_owned(),
            ..Item::label(p, f.mail.contains(*p))
        })
        .collect();
    powers.push(Item::label("bearsKnowledge", f.events.contains("2120303")));
    powers.push(Item::label(
        "springOnionMastery",
        f.events.contains("3910979"),
    ));
    let notes = (1..=SECRET_NOTES)
        .map(|n| Item {
            id: format!("note-{n}"),
            have: Some(n as u64),
            ..Item::label("secretNote", f.notes.contains(&n))
        })
        .collect();
    section(
        "secrets",
        vec![
            group("books", books),
            group("powers", powers),
            group("secretNotes", notes),
        ],
    )
}

// Grandpa's evaluation, as `Utility.getGrandpaScore` adds it up (21 points)
fn grandpa(f: &Facts) -> Section {
    let earned = f.money_earned;
    let money_points = [
        (50_000, 1),
        (100_000, 2),
        (200_000, 3),
        (300_000, 4),
        (500_000, 5),
        (1_000_000, 7),
    ]
    .iter()
    .filter(|(at, _)| earned >= *at)
    .map(|(_, p)| *p)
    .max()
    .unwrap_or(0);
    let near_max = f.friends.values().filter(|p| **p >= 1975).count();
    let level = f.skill_levels / 2;
    let cc_done = f.events.contains("191393");
    let cc_rooms = [
        "ccPantry",
        "ccCraftsRoom",
        "ccFishTank",
        "ccBoilerRoom",
        "ccVault",
        "ccBulletin",
    ]
    .iter()
    .all(|r| f.mail.contains(*r));
    let points = |label: &'static str, have: u64, max: u64| Item {
        done: have >= max,
        ..Item::label(label, have >= max).counted(have, max)
    };
    let items = vec![
        points("grandpaMoney", money_points, 7),
        points(
            "grandpaAchievements",
            [5, 26, 34]
                .iter()
                .filter(|a| f.achievements.contains(a))
                .count() as u64,
            3,
        ),
        points(
            "grandpaCommunityCenter",
            if cc_done { 3 } else { u64::from(cc_rooms) },
            3,
        ),
        points("grandpaRustyKey", f.mail.contains("HasRustyKey") as u64, 1),
        points("grandpaSkullKey", f.mail.contains("HasSkullKey") as u64, 1),
        points(
            "grandpaFamily",
            (f.spouse.is_some() && f.house_level >= 2) as u64,
            1,
        ),
        points(
            "grandpaFriends",
            match near_max {
                10.. => 2,
                5.. => 1,
                _ => 0,
            },
            2,
        ),
        points(
            "grandpaSkills",
            match level {
                25.. => 2,
                15.. => 1,
                _ => 0,
            },
            2,
        ),
        points("grandpaPet", f.mail.contains("petLoveMessage") as u64, 1),
    ];
    let score: u64 = items.iter().filter_map(|i| i.have).sum();
    let candles = match score {
        12.. => 4,
        8..=11 => 3,
        4..=7 => 2,
        _ => 1,
    };
    section(
        "grandpa",
        vec![
            group("grandpaScore", items),
            group(
                "grandpaCandles",
                vec![
                    Item::goal("candlesExpected", candles, 4),
                    Item::goal("candlesLit", f.grandpa_candles as u64, 4),
                ],
            ),
        ],
    )
}

fn achievements(f: &Facts, c: &Catalog) -> Section {
    let items = c
        .achievements
        .iter()
        .map(|a| {
            Item::named(
                a.id.to_string(),
                a.name.clone(),
                f.achievements.contains(&a.id),
            )
        })
        .collect();
    section("achievements", vec![group("achievementList", items)])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::perfection::{CatalogItem, CatalogMuseumItem};

    #[test]
    fn reads_bundles() {
        let names = HashMap::from([("24", "Parsnip"), ("188", "Green Bean")]);
        let g = bundle(
            0,
            "Spring Crops/O 465 20/24 1 0 188 5 2/0///Весенние культуры",
            Some(&vec![true, false]),
            false,
            &names,
        );
        assert_eq!(g.title.as_deref(), Some("Весенние культуры"));
        assert_eq!((g.done, g.total), (1, 2));
        assert_eq!(g.items[1].name.as_deref(), Some("Green Bean"));
        assert_eq!(g.items[1].need, Some(5));
        assert_eq!(g.items[1].note, Some("gold"));

        // "any 1 of 2" bundles become a goal with that count
        let any = bundle(
            1,
            "Pick/O 1 1/24 1 0 188 1 0/0/1//Pick",
            Some(&vec![false, true]),
            false,
            &names,
        );
        assert_eq!(any.items[0].label, Some("bundleNeeds"));
        assert_eq!((any.done, any.total), (1, 1));

        let gold = bundle(2, "2,500g/O 1 1/-1 2500 2500/4///", None, true, &names);
        assert_eq!(gold.items[0].label, Some("gold"));
        assert!(gold.items[0].done, "a finished room marks its bundles done");
    }

    #[test]
    fn builds_sections() {
        let mut f = Facts {
            money_earned: 60_000,
            xp: vec![15000, 100, 0, 0, 0, 0],
            levels: [10, 1, 0, 0, 0, 0],
            skill_levels: 11,
            ..Facts::default()
        };
        f.donated.insert("96".into());
        f.artifacts_found.insert("97".into());
        f.friends.insert("Abigail".into(), 2000);
        f.friend_status.insert("Abigail".into(), "Friendly".into());
        let catalog = Catalog {
            items: vec![CatalogItem {
                id: "Book_Trash".into(),
                name: "The Alleyway Buffet".into(),
            }],
            museum: vec![
                CatalogMuseumItem {
                    id: "96".into(),
                    name: "Dwarf Scroll I".into(),
                    kind: "artifact".into(),
                },
                CatalogMuseumItem {
                    id: "97".into(),
                    name: "Dwarf Scroll II".into(),
                    kind: "artifact".into(),
                },
            ],
            villagers: vec![crate::perfection::CatalogVillager {
                key: "Abigail".into(),
                name: "Abigail".into(),
                romance: true,
            }],
            ..Catalog::default()
        };
        let c = build(&f, Some(&catalog));
        let get = |id: &str| c.sections.iter().find(|s| s.id == id).unwrap();

        let money = get("money");
        assert_eq!((money.done, money.total), (2, 5));
        let skills = get("skills");
        let farming = &skills.groups[0].items[0];
        assert!(farming.done);
        assert_eq!(
            skills.groups[0].items[1].need,
            Some(380),
            "fishing's next level"
        );

        let museum = get("museum");
        let artifacts = museum.groups.iter().find(|g| g.id == "artifacts").unwrap();
        assert_eq!(artifacts.done, 1);
        assert_eq!(artifacts.items[1].note, Some("foundNotDonated"));

        let friends = get("friends");
        let abigail = &friends
            .groups
            .iter()
            .find(|g| g.id == "villagers")
            .unwrap()
            .items[0];
        assert!(abigail.done, "8 hearts max out a datable villager");
        let secrets = get("secrets");
        assert_eq!(
            secrets.groups[0].items[0].name.as_deref(),
            Some("The Alleyway Buffet")
        );
    }

    // PELICAN_TEST_SAVE=<save folder> PELICAN_TEST_CATALOG=<catalog.json> \
    //   cargo test -- --ignored real_save_checkup --nocapture
    #[test]
    #[ignore]
    fn real_save_checkup() {
        let dir = std::path::PathBuf::from(std::env::var("PELICAN_TEST_SAVE").unwrap());
        let catalog: Catalog = serde_json::from_slice(
            &std::fs::read(std::env::var("PELICAN_TEST_CATALOG").unwrap()).unwrap(),
        )
        .unwrap();
        let facts = crate::perfection::facts(&dir).unwrap();
        let c = build(&facts, Some(&catalog));
        for s in &c.sections {
            eprintln!("{:<12} {:>3}/{:<3}", s.id, s.done, s.total);
            for g in &s.groups {
                let missing: Vec<String> = g
                    .items
                    .iter()
                    .filter(|i| !i.done)
                    .take(4)
                    .map(|i| {
                        let what = i
                            .name
                            .clone()
                            .unwrap_or_else(|| i.label.unwrap_or("?").to_owned());
                        match (i.have, i.need) {
                            (Some(h), Some(n)) => format!("{what} {h}/{n}"),
                            _ => what,
                        }
                    })
                    .collect();
                eprintln!(
                    "    {:<22} {:>3}/{:<3} {} {:?}",
                    g.title.clone().unwrap_or_else(|| g.id.clone()),
                    g.done,
                    g.total,
                    g.category.clone().unwrap_or_default(),
                    missing
                );
            }
        }
    }
}
