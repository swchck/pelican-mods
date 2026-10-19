//! Reads SMAPI's log after a session into a report of what went wrong, by mod.
//!
//! SMAPI already says it all, but across thousands of lines: mods it skipped and why,
//! save data the game dropped because the mod behind it didn't load, and each mod's
//! errors and warnings. The report keeps those and folds repeats together.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;

use crate::error::AppResult;

/// Entries kept per mod; a mod failing every tick would otherwise fill the screen.
const MAX_ENTRIES: usize = 20;

/// What the last session's log says went wrong.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LogReport {
    /// Unix seconds when the log was last written; 0 without a log.
    pub written: u64,
    pub skipped: Vec<Skipped>,
    /// Save locations the game ignored because no loaded mod adds them; saving drops them.
    pub lost_locations: Vec<String>,
    /// NPCs that lived in those locations.
    pub lost_npcs: Vec<String>,
    /// Mods with errors or warnings, most errors first.
    pub mods: Vec<ModProblems>,
}

/// A mod SMAPI refused to load.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Skipped {
    pub name: String,
    pub version: String,
    /// "incompatible", "dependency", "smapi" or "other".
    pub kind: &'static str,
    /// The mod it needs, for "dependency".
    pub needs: Option<String>,
    /// SMAPI's own words, after "because".
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModProblems {
    /// The name SMAPI stamps the mod's lines with.
    pub source: String,
    pub errors: u32,
    pub warnings: u32,
    pub entries: Vec<Entry>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    /// "error" or "warn".
    pub level: &'static str,
    pub text: String,
    pub count: u32,
}

/// Reads the report from the SMAPI log at `file`; an empty one when there is none.
pub fn read(file: &Path) -> AppResult<LogReport> {
    let Ok(text) = std::fs::read_to_string(file) else {
        return Ok(LogReport::default());
    };
    let written = std::fs::metadata(file)?
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Ok(LogReport {
        written,
        ..parse(&text)
    })
}

/// Builds the report from the log's text.
pub fn parse(text: &str) -> LogReport {
    let mut report = LogReport::default();
    let mut by_mod: BTreeMap<String, ModProblems> = BTreeMap::new();
    for line in text.lines() {
        let Some((level, source, msg)) = stamp(line) else {
            continue;
        };
        if level != "ERROR" && level != "WARN" {
            continue;
        }
        if source == "SMAPI" {
            if let Some(s) = skipped(msg) {
                report.skipped.push(s);
                continue;
            }
            let m = msg.trim();
            // the frame around the skipped list, not problems of their own
            if m.is_empty()
                || m == "Skipped mods"
                || m.starts_with("---")
                || m.starts_with("These mods")
            {
                continue;
            }
        }
        if source == "game" {
            if let Some((location, npcs)) = ignored_location(msg) {
                report.lost_locations.push(location);
                report.lost_npcs.extend(npcs);
                continue;
            }
            // GOG Galaxy's login on a copy that isn't from GOG; nothing to do about it
            if msg.contains("Galaxy auth failure") {
                continue;
            }
        }
        let problems = by_mod
            .entry(source.to_owned())
            .or_insert_with(|| ModProblems {
                source: source.to_owned(),
                errors: 0,
                warnings: 0,
                entries: Vec::new(),
            });
        let level = if level == "ERROR" { "error" } else { "warn" };
        match level {
            "error" => problems.errors += 1,
            _ => problems.warnings += 1,
        }
        let text = msg.trim().to_owned();
        if let Some(e) = problems.entries.iter_mut().find(|e| e.text == text) {
            e.count += 1;
        } else if problems.entries.len() < MAX_ENTRIES {
            problems.entries.push(Entry {
                level,
                text,
                count: 1,
            });
        }
    }
    report.mods = by_mod.into_values().collect();
    report
        .mods
        .sort_by(|a, b| b.errors.cmp(&a.errors).then(b.warnings.cmp(&a.warnings)));
    report.lost_npcs.sort();
    report.lost_npcs.dedup();
    report
}

// "[12:21:28 ERROR Content Patcher] message" → ("ERROR", "Content Patcher", " message")
fn stamp(line: &str) -> Option<(&str, &str, &str)> {
    let rest = line.strip_prefix('[')?;
    let (head, msg) = rest.split_once(']')?;
    let mut parts = head.splitn(3, ' ');
    let time = parts.next()?;
    if !time.contains(':') {
        return None;
    }
    let level = parts.next()?;
    let source = parts.next()?.trim();
    Some((level, source, msg))
}

// "      - Json Assets 1.11.8 because it needs the 'SpaceCore' mod, which couldn't be loaded."
fn skipped(msg: &str) -> Option<Skipped> {
    let item = msg.trim_start().strip_prefix("- ")?;
    let (head, reason) = item.split_once(" because ")?;
    let (name, version) = head.rsplit_once(' ')?;
    let reason = reason.trim().to_owned();
    let needs = reason
        .strip_prefix("it needs the '")
        .and_then(|r| r.split_once('\'').map(|(m, _)| m.to_owned()));
    let kind = if reason.starts_with("it's no longer compatible") {
        "incompatible"
    } else if needs.is_some() {
        "dependency"
    } else if reason.contains("SMAPI") {
        "smapi"
    } else {
        "other"
    };
    Some(Skipped {
        name: name.to_owned(),
        version: version.to_owned(),
        kind,
        needs,
        reason,
    })
}

// "Ignored unknown location 'Custom_X' in save data, including NPCs 'Olivia', 'Victor'."
fn ignored_location(msg: &str) -> Option<(String, Vec<String>)> {
    let rest = msg.trim().strip_prefix("Ignored unknown location '")?;
    let (location, tail) = rest.split_once('\'')?;
    let npcs = tail
        .split_once("including NPC")
        .map(|(_, list)| {
            list.split('\'')
                .skip(1)
                .step_by(2)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    Some((location.to_owned(), npcs))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG: &str = "\
[12:21:20 INFO  SMAPI] SMAPI 4.5.2 with Stardew Valley 1.6.15
[12:21:28 ERROR SMAPI]    Skipped mods
[12:21:28 ERROR SMAPI]    --------------------------------------------------
[12:21:28 ERROR SMAPI]       These mods could not be added to your game.

[12:21:28 ERROR SMAPI]       - SpaceCore 1.25.2 because it's no longer compatible. Please check for a new version at https://smapi.io/mods
[12:21:28 ERROR SMAPI]       - Json Assets 1.11.8 because it needs the 'SpaceCore' mod, which couldn't be loaded.
[12:21:30 ERROR game] Galaxy auth failure: FAILURE_REASON_GALAXY_SERVICE_NOT_AVAILABLE
[12:21:31 ERROR Content Patcher] Two content packs want to load the 'Animals/horse' asset.
[12:21:31 ERROR Content Patcher] Two content packs want to load the 'Animals/horse' asset.
   at StackTrace.Line()
[12:21:32 WARN  Automate] Couldn't read a chest.
[12:22:01 WARN  game] Ignored unknown location 'Custom_AndyHouse' in save data, including NPC 'Andy'.
[12:22:01 WARN  game] Ignored unknown location 'Custom_JenkinsHouse' in save data, including NPCs 'Olivia', 'Victor'.
[12:22:01 WARN  game] Ignored unknown location 'Custom_WillyRoom' in save data.
";

    #[test]
    fn reads_a_session() {
        let r = parse(LOG);
        assert_eq!(r.skipped.len(), 2);
        assert_eq!(r.skipped[0].name, "SpaceCore");
        assert_eq!(r.skipped[0].version, "1.25.2");
        assert_eq!(r.skipped[0].kind, "incompatible");
        assert_eq!(r.skipped[1].kind, "dependency");
        assert_eq!(r.skipped[1].needs.as_deref(), Some("SpaceCore"));
        assert_eq!(
            r.lost_locations,
            [
                "Custom_AndyHouse",
                "Custom_JenkinsHouse",
                "Custom_WillyRoom"
            ]
        );
        assert_eq!(r.lost_npcs, ["Andy", "Olivia", "Victor"]);
        assert_eq!(r.mods.len(), 2);
        assert_eq!(r.mods[0].source, "Content Patcher");
        assert_eq!(r.mods[0].errors, 2);
        assert_eq!(r.mods[0].entries.len(), 1);
        assert_eq!(r.mods[0].entries[0].count, 2);
        assert_eq!(r.mods[1].source, "Automate");
        assert_eq!(r.mods[1].entries[0].level, "warn");
    }

    // PELICAN_TEST_LOG=<SMAPI-latest.txt> cargo test -- --ignored real_log --nocapture
    #[test]
    #[ignore]
    fn real_log() {
        let r = read(Path::new(&std::env::var("PELICAN_TEST_LOG").unwrap())).unwrap();
        eprintln!("skipped {}:", r.skipped.len());
        for s in &r.skipped {
            eprintln!("  {} {} [{}] {:?}", s.name, s.version, s.kind, s.needs);
        }
        eprintln!(
            "lost {} locations, npcs {:?}",
            r.lost_locations.len(),
            r.lost_npcs
        );
        for m in &r.mods {
            eprintln!("{}: {} errors, {} warnings", m.source, m.errors, m.warnings);
            for e in m.entries.iter().take(3) {
                eprintln!(
                    "  x{} {}",
                    e.count,
                    e.text.chars().take(110).collect::<String>()
                );
            }
        }
    }
}
