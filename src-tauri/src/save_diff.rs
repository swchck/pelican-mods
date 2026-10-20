//! What changed on a farm between two states of a save: money, skills, friendships,
//! firsts (shipped, caught, cooked, donated, achievements) and what moved in and out
//! of storage. Either side is a snapshot or the save as it is now.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use serde::Serialize;

use crate::error::AppResult;
use crate::perfection::{self, Catalog, Facts};

/// Item count changes past this many are noise for a summary; the biggest ones stay.
const ITEMS_SHOWN: usize = 30;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Side {
    pub year: u32,
    /// 0 spring … 3 winter.
    pub season: u8,
    pub day: u8,
    pub money: u64,
    pub total_earned: u64,
    pub play_time_ms: u64,
    /// Perfection, 0–100, when the catalog covers every part.
    pub perfection: Option<f32>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    /// A skill key ("farming") or a villager's internal name.
    pub key: String,
    pub name: String,
    pub from: u32,
    pub to: u32,
}

/// Things done for the first time between the two states, by kind.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Firsts {
    /// "shipped", "fish", "cooked", "crafted", "donated" or "achievement".
    pub kind: &'static str,
    pub names: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SaveDiff {
    pub from: Side,
    pub to: Side,
    pub skills: Vec<Change>,
    /// Hearts, whole ones, biggest change first.
    pub friends: Vec<Change>,
    pub firsts: Vec<Firsts>,
    /// Stored item totals that changed, biggest change first.
    pub items: Vec<Change>,
}

const SKILLS: [&str; 5] = ["farming", "fishing", "foraging", "mining", "combat"];

/// Compares save folders `from` and `to`, naming things from `catalog`.
pub fn compare(from: &Path, to: &Path, catalog: &Catalog) -> AppResult<SaveDiff> {
    let (fa, fb) = (perfection::facts(from)?, perfection::facts(to)?);
    let side = |dir: &Path, f: &Facts| -> AppResult<Side> {
        let s = crate::saves::summary(dir)?;
        Ok(Side {
            year: s.year,
            season: s.season,
            day: s.day,
            money: s.money,
            total_earned: s.total_earned,
            play_time_ms: s.play_time_ms,
            perfection: perfection::evaluate(f, Some(catalog)).percent,
        })
    };
    let item_name: HashMap<&str, &str> = catalog
        .items
        .iter()
        .map(|i| (i.id.as_str(), i.name.as_str()))
        .collect();
    let name_of = |id: &str| {
        let bare = id.strip_prefix("(O)").unwrap_or(id);
        item_name
            .get(bare)
            .map_or_else(|| id.to_owned(), |n| (*n).to_owned())
    };

    let skills = SKILLS
        .iter()
        .enumerate()
        .filter(|(i, _)| fa.levels[*i] != fb.levels[*i])
        .map(|(i, s)| Change {
            key: (*s).to_owned(),
            name: catalog.term(&format!("skill.{s}")).unwrap_or(s).to_owned(),
            from: fa.levels[i],
            to: fb.levels[i],
        })
        .collect();

    let villager: HashMap<&str, &str> = catalog
        .villagers
        .iter()
        .map(|v| (v.key.as_str(), v.name.as_str()))
        .collect();
    let hearts = |f: &Facts, k: &str| (f.friends.get(k).copied().unwrap_or(0).max(0) / 250) as u32;
    let keys: HashSet<&String> = fa.friends.keys().chain(fb.friends.keys()).collect();
    let mut friends: Vec<Change> = keys
        .into_iter()
        .filter(|k| hearts(&fa, k) != hearts(&fb, k))
        .map(|k| Change {
            key: k.clone(),
            name: villager
                .get(k.as_str())
                .map_or_else(|| k.clone(), |n| (*n).to_owned()),
            from: hearts(&fa, k),
            to: hearts(&fb, k),
        })
        .collect();
    friends.sort_by_key(|c| std::cmp::Reverse(c.to.abs_diff(c.from)));

    let new = |a: &HashSet<String>, b: &HashSet<String>, name: &dyn Fn(&str) -> String| {
        let mut names: Vec<String> = b.difference(a).map(|id| name(id)).collect();
        names.sort();
        names
    };
    let recipe: HashMap<&str, &str> = catalog
        .cooking
        .iter()
        .map(|r| (r.output.as_str(), r.name.as_str()))
        .collect();
    let crafted_a: HashSet<String> = fa.crafted.keys().cloned().collect();
    let crafted_b: HashSet<String> = fb.crafted.keys().cloned().collect();
    let crafting: HashMap<&str, &str> = catalog
        .crafting
        .iter()
        .map(|r| (r.key.as_str(), r.name.as_str()))
        .collect();
    let achievement: HashMap<String, &str> = catalog
        .achievements
        .iter()
        .map(|a| (a.id.to_string(), a.name.as_str()))
        .collect();
    let ach = |f: &Facts| {
        f.achievements
            .iter()
            .map(u32::to_string)
            .collect::<HashSet<_>>()
    };
    let mut firsts = vec![
        Firsts {
            kind: "shipped",
            names: new(&fa.shipped, &fb.shipped, &|id| name_of(id)),
        },
        Firsts {
            kind: "fish",
            names: new(&fa.fish, &fb.fish, &|id| name_of(id)),
        },
        Firsts {
            kind: "cooked",
            names: new(&fa.cooked, &fb.cooked, &|id| {
                recipe
                    .get(id)
                    .map_or_else(|| name_of(id), |n| (*n).to_owned())
            }),
        },
        Firsts {
            kind: "crafted",
            names: new(&crafted_a, &crafted_b, &|k| {
                crafting
                    .get(k)
                    .map_or_else(|| k.to_owned(), |n| (*n).to_owned())
            }),
        },
        Firsts {
            kind: "donated",
            names: new(&fa.donated, &fb.donated, &|id| name_of(id)),
        },
        Firsts {
            kind: "achievement",
            names: new(&ach(&fa), &ach(&fb), &|id| {
                achievement
                    .get(id)
                    .map_or_else(|| id.to_owned(), |n| (*n).to_owned())
            }),
        },
    ];
    firsts.retain(|f| !f.names.is_empty());

    let totals = |dir: &Path| -> AppResult<BTreeMap<String, (String, u32)>> {
        let mut out: BTreeMap<String, (String, u32)> = BTreeMap::new();
        for i in crate::chests::in_save(dir)? {
            let e = out
                .entry(i.id.clone())
                .or_insert_with(|| (i.name.clone(), 0));
            e.1 += i.stack;
        }
        Ok(out)
    };
    let (ta, tb) = (totals(from)?, totals(to)?);
    let ids: HashSet<&String> = ta.keys().chain(tb.keys()).collect();
    let mut items: Vec<Change> = ids
        .into_iter()
        .filter_map(|id| {
            let a = ta.get(id).map_or(0, |e| e.1);
            let b = tb.get(id).map_or(0, |e| e.1);
            let raw = ta
                .get(id)
                .or(tb.get(id))
                .map(|e| e.0.clone())
                .unwrap_or_default();
            let name = if id.starts_with("(O)") {
                name_of(id)
            } else {
                raw
            };
            (a != b).then(|| Change {
                key: id.clone(),
                name,
                from: a,
                to: b,
            })
        })
        .collect();
    items.sort_by_key(|c| std::cmp::Reverse(c.to.abs_diff(c.from)));
    items.truncate(ITEMS_SHOWN);

    Ok(SaveDiff {
        from: side(from, &fa)?,
        to: side(to, &fb)?,
        skills,
        friends,
        firsts,
        items,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // PELICAN_TEST_SAVE=<save folder> PELICAN_TEST_SAVE_OLD=<older copy of it> \
    //   cargo test -- --ignored real_save_diff --nocapture
    #[test]
    #[ignore]
    fn real_save_diff() {
        let var = |k: &str| std::path::PathBuf::from(std::env::var(k).unwrap());
        let started = std::time::Instant::now();
        let d = compare(
            &var("PELICAN_TEST_SAVE_OLD"),
            &var("PELICAN_TEST_SAVE"),
            Catalog::builtin("ru"),
        )
        .unwrap();
        eprintln!("{:?}", started.elapsed());
        eprintln!("{d:#?}");
    }
}
