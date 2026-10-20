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
/// Where a notice's explanation starts: past the "]", deeper than its header's three spaces.
const NOTICE_DETAIL: &str = "       ";

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
    /// Seconds from the game starting to SMAPI's console being ready.
    pub startup_secs: Option<u32>,
    /// Mods the load paused on for [`SLOW_SECS`] or more, slowest first.
    pub slow: Vec<SlowMod>,
    /// Mods loading or editing the most game assets, most first.
    pub heavy: Vec<HeavyMod>,
}

/// SMAPI stamps lines to the second, so a shorter pause can't be told from noise.
pub const SLOW_SECS: u32 = 2;
/// Mods listed as asset-heavy.
const HEAVY_SHOWN: usize = 8;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SlowMod {
    pub name: String,
    pub secs: u32,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HeavyMod {
    pub name: String,
    /// Assets it loaded or edited during the session.
    pub assets: u32,
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
    let mut first: Option<u32> = None;
    // the mod SMAPI is loading and when it started; the next line ends its turn
    let mut loading: Option<(String, u32)> = None;
    let mut assets: BTreeMap<String, u32> = BTreeMap::new();
    // SMAPI's startup notices are a header and deeper-indented lines that explain it;
    // the explanation joins the header's entry instead of counting as warnings of its own
    let mut notice: Option<usize> = None;
    for line in text.lines() {
        let Some((level, source, msg)) = stamp(line) else {
            continue;
        };
        let framing =
            source == "SMAPI" && (msg.starts_with(NOTICE_DETAIL) || msg.trim().starts_with("---"));
        if !framing {
            notice = None;
        }
        let now = clock(line);
        if first.is_none() {
            first = now;
        }
        if source == "SMAPI" {
            let m = msg.trim();
            if let Some(now) = now {
                let next = m
                    .split_once(" (from Mods/")
                    .map(|(name, _)| name.to_owned());
                if next.is_some() || m.starts_with("Launching mods") {
                    if let Some((name, at)) = loading.take() {
                        let secs = now.saturating_sub(at);
                        if secs >= SLOW_SECS {
                            report.slow.push(SlowMod { name, secs });
                        }
                    }
                    loading = next.map(|n| (n, now));
                }
                if m.starts_with("Type 'help' for help") && report.startup_secs.is_none() {
                    report.startup_secs = first.map(|f| now.saturating_sub(f));
                }
            }
            if let Some((name, _)) = m
                .split_once(" loaded asset '")
                .or_else(|| m.split_once(" edited asset '"))
            {
                *assets.entry(name.to_owned()).or_default() += 1;
            }
        }
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
            if m.is_empty() || m == "Skipped mods" || m.starts_with("---") {
                continue;
            }
            if msg.starts_with(NOTICE_DETAIL) {
                let entry =
                    notice.and_then(|i| by_mod.get_mut("SMAPI").and_then(|p| p.entries.get_mut(i)));
                if let Some(e) = entry {
                    e.text.push(if e.text.ends_with(':') { ' ' } else { '\n' });
                    e.text.push_str(m);
                }
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
            if source == "SMAPI" {
                notice = Some(problems.entries.len());
            }
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
    report.slow.sort_by_key(|m| std::cmp::Reverse(m.secs));
    let mut heavy: Vec<HeavyMod> = assets
        .into_iter()
        .map(|(name, assets)| HeavyMod { name, assets })
        .collect();
    heavy.sort_by_key(|m| std::cmp::Reverse(m.assets));
    heavy.truncate(HEAVY_SHOWN);
    report.heavy = heavy;
    report
}

// "[12:21:28 …" → seconds since midnight
fn clock(line: &str) -> Option<u32> {
    let t = line.get(1..9)?;
    let mut parts = t.split(':').map(|p| p.parse::<u32>().ok());
    Some(parts.next()?? * 3600 + parts.next()?? * 60 + parts.next()??)
}

/// The error a session ended on, and the mod its stack points at.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Crash {
    /// Who logged it: a mod's name, "SMAPI" or "game".
    pub source: String,
    pub message: String,
    /// The lines after it, usually the stack trace.
    pub details: Vec<String>,
    /// The mod whose code is highest in the stack, when that differs from `source`.
    pub suspect: Option<String>,
}

/// Lines of a stack trace kept; the frames past these are the game's own plumbing.
const CRASH_DETAILS: usize = 14;

/// Finds the last error in `text` that isn't SMAPI's skipped-mods list, and names the
/// mod in its stack from `assemblies` (assembly or root namespace → mod name).
pub fn last_error(text: &str, assemblies: &[(String, String)]) -> Option<Crash> {
    let mut last: Option<Crash> = None;
    let mut open = false;
    for line in text.lines() {
        match stamp(line) {
            Some((level, source, msg)) => {
                open = false;
                if level != "ERROR" {
                    continue;
                }
                let m = msg.trim();
                let frame = source == "SMAPI"
                    && (skipped(msg).is_some()
                        || m.is_empty()
                        || m == "Skipped mods"
                        || m.starts_with("---")
                        || m.starts_with("These mods"));
                if frame || m.contains("Galaxy auth failure") {
                    continue;
                }
                last = Some(Crash {
                    source: source.to_owned(),
                    message: m.to_owned(),
                    details: Vec::new(),
                    suspect: None,
                });
                open = true;
            }
            None if open => {
                if let Some(c) = last
                    .as_mut()
                    .filter(|c| c.details.len() < CRASH_DETAILS && !line.trim().is_empty())
                {
                    c.details.push(line.trim().to_owned());
                }
            }
            None => {}
        }
    }
    let mut crash = last?;
    // "at SpaceCore.Patches.Foo.Bar()" → the first frame whose namespace is a mod's
    crash.suspect = crash
        .details
        .iter()
        .filter_map(|l| l.strip_prefix("at "))
        .find_map(|frame| {
            assemblies
                .iter()
                .find(|(asm, _)| frame.starts_with(&format!("{asm}.")))
                .map(|(_, name)| name.clone())
        })
        .filter(|name| *name != crash.source);
    Some(crash)
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
[12:21:28 WARN  SMAPI]    Changed save serializer
[12:21:28 WARN  SMAPI]    --------------------------------------------------
[12:21:28 WARN  SMAPI]       These mods change the save serializer. They may corrupt your save files, or make them unusable if
[12:21:28 WARN  SMAPI]       you uninstall these mods.
[12:21:28 WARN  SMAPI]       - SpaceCore
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
    fn finds_the_crash_and_its_mod() {
        let log = "\
[10:00:00 ERROR SMAPI]       - Json Assets 1.11.8 because it needs the 'SpaceCore' mod.
[10:05:00 ERROR game] An error occurred in the base update loop: System.NullReferenceException
   at SpaceCore.Patches.FarmerPatcher.After_Update(Farmer __instance)
   at StardewValley.Farmer.Update(GameTime time)
[10:05:00 TRACE SMAPI] Shutting down.
";
        let assemblies = [("SpaceCore".to_owned(), "SpaceCore".to_owned())];
        let c = last_error(log, &assemblies).unwrap();
        assert_eq!(c.source, "game");
        assert!(
            c.message
                .starts_with("An error occurred in the base update loop")
        );
        assert_eq!(c.details.len(), 2);
        assert_eq!(c.suspect.as_deref(), Some("SpaceCore"));
        assert!(last_error("[10:00:00 INFO  SMAPI] hi\n", &assemblies).is_none());
    }

    #[test]
    fn times_the_startup() {
        let log = "\
[10:00:00 INFO  SMAPI] SMAPI 4.5.2
[10:00:02 DEBUG SMAPI] Loading mods...
[10:00:02 TRACE SMAPI]    Fast (from Mods/Fast/Fast.dll, ID: a.Fast, assembly version: 1.0.0)...
[10:00:03 TRACE SMAPI]    Slow Expansion (from Mods/SVE/content.json, ID: a.Sve)...
[10:00:09 TRACE SMAPI]    Last (from Mods/Last/Last.dll, ID: a.Last)...
[10:00:12 DEBUG SMAPI] Launching mods...
[10:00:13 TRACE SMAPI] Content Patcher edited asset 'Data/Objects'.
[10:00:13 TRACE SMAPI] Content Patcher edited asset 'Maps/Town'.
[10:00:13 TRACE SMAPI] Lookup Anything loaded asset 'Mods/x'.
[10:00:20 INFO  SMAPI] Type 'help' for help, or 'help <cmd>' for a command's usage
";
        let r = parse(log);
        assert_eq!(r.startup_secs, Some(20));
        assert_eq!(
            r.slow,
            [
                SlowMod {
                    name: "Slow Expansion".into(),
                    secs: 6
                },
                SlowMod {
                    name: "Last".into(),
                    secs: 3
                },
            ]
        );
        assert_eq!(
            r.heavy[0],
            HeavyMod {
                name: "Content Patcher".into(),
                assets: 2
            }
        );
    }

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
        assert_eq!(r.mods.len(), 3);
        let smapi = r.mods.iter().find(|m| m.source == "SMAPI").unwrap();
        assert_eq!(smapi.warnings, 1);
        assert_eq!(
            smapi.entries[0].text,
            "Changed save serializer\nThese mods change the save serializer. They may corrupt your save \
             files, or make them unusable if\nyou uninstall these mods.\n- SpaceCore"
        );
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
