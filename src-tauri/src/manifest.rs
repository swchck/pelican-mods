//! SMAPI `manifest.json` files.
//!
//! SMAPI reads manifests with Newtonsoft.Json, which is lax in ways mod authors rely on:
//! keys are case-insensitive, comments and trailing commas are fine, and some files
//! start with a UTF-8 BOM. Parsing mirrors that, so every manifest SMAPI accepts loads here.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

pub const FILE_NAME: &str = "manifest.json";

/// A mod's identity and requirements, as declared in its manifest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all(serialize = "camelCase", deserialize = "lowercase"))]
pub struct Manifest {
    pub name: String,
    #[serde(default)]
    pub author: String,
    #[serde(default, deserialize_with = "version_string")]
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(alias = "uniqueid")]
    pub unique_id: String,
    #[serde(default, alias = "entrydll")]
    pub entry_dll: Option<String>,
    #[serde(default, alias = "contentpackfor")]
    pub content_pack_for: Option<Dependency>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
    #[serde(default, alias = "updatekeys")]
    pub update_keys: Vec<String>,
    #[serde(
        default,
        alias = "minimumapiversion",
        deserialize_with = "opt_version_string"
    )]
    pub minimum_api_version: Option<String>,
    #[serde(
        default,
        alias = "minimumgameversion",
        deserialize_with = "opt_version_string"
    )]
    pub minimum_game_version: Option<String>,
}

/// Another mod this one needs or can use.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all(serialize = "camelCase", deserialize = "lowercase"))]
pub struct Dependency {
    #[serde(alias = "uniqueid")]
    pub unique_id: String,
    #[serde(
        default,
        alias = "minimumversion",
        deserialize_with = "opt_version_string"
    )]
    pub minimum_version: Option<String>,
    #[serde(default = "yes", alias = "isrequired")]
    pub is_required: bool,
}

fn yes() -> bool {
    true
}

/// Parses manifest text the way SMAPI does.
pub fn parse(text: &str) -> AppResult<Manifest> {
    let text = text.trim_start_matches('\u{feff}');
    let value: serde_json::Value =
        json5::from_str(text).map_err(|e| AppError::BadManifest(e.to_string()))?;
    let m: Manifest = serde_json::from_value(lowercase_keys(value))
        .map_err(|e| AppError::BadManifest(e.to_string()))?;
    if m.unique_id.trim().is_empty() {
        return Err(AppError::BadManifest("UniqueID is empty".into()));
    }
    Ok(m)
}

/// Reads and parses `<dir>/manifest.json`.
pub fn read(dir: &Path) -> AppResult<Manifest> {
    let text = std::fs::read_to_string(dir.join(FILE_NAME))?;
    parse(&text)
}

/// Returns the site prefix of the first update key ("Nexus", "GitHub", …), if any.
pub fn source(m: &Manifest) -> Option<String> {
    m.update_keys
        .iter()
        .find_map(|k| k.split_once(':').map(|(site, _)| site.trim().to_owned()))
}

fn lowercase_keys(v: serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match v {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(k, v)| (k.to_lowercase(), lowercase_keys(v)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.into_iter().map(lowercase_keys).collect()),
        other => other,
    }
}

// older manifests spell versions as {"MajorVersion": 1, "MinorVersion": 2, "PatchVersion": 0}
fn version_value(v: serde_json::Value) -> Option<String> {
    use serde_json::Value;
    match v {
        Value::String(s) => Some(s),
        Value::Number(n) => Some(n.to_string()),
        Value::Object(o) => {
            let part = |k: &str| o.get(k).and_then(Value::as_u64).unwrap_or(0);
            let mut s = format!(
                "{}.{}.{}",
                part("majorversion"),
                part("minorversion"),
                part("patchversion")
            );
            if let Some(tag) = o.get("build").and_then(Value::as_str) {
                s.push('-');
                s.push_str(tag);
            }
            Some(s)
        }
        _ => None,
    }
}

fn version_string<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Ok(version_value(serde_json::Value::deserialize(d)?).unwrap_or_default())
}

fn opt_version_string<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Ok(version_value(serde_json::Value::deserialize(d)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lax_manifest() {
        let text = "\u{feff}{
            // comment
            \"name\": \"Lookup Anything\",
            \"Author\": \"Pathoschild\",
            \"Version\": \"1.49.0\",
            \"UniqueID\": \"Pathoschild.LookupAnything\",
            \"EntryDll\": \"LookupAnything.dll\",
            \"MinimumApiVersion\": \"4.0.0\",
            \"UpdateKeys\": [ \"Nexus:541\", ],
            \"Dependencies\": [ { \"UniqueID\": \"spacechase0.GenericModConfigMenu\", \"IsRequired\": false } ],
        }";
        let m = parse(text).unwrap();
        assert_eq!(m.name, "Lookup Anything");
        assert_eq!(m.unique_id, "Pathoschild.LookupAnything");
        assert_eq!(m.minimum_api_version.as_deref(), Some("4.0.0"));
        assert_eq!(source(&m).as_deref(), Some("Nexus"));
        assert!(!m.dependencies[0].is_required);
    }

    #[test]
    fn parses_content_pack_and_object_version() {
        let text = r#"{
            "Name": "[CP] Pretty Paths",
            "Version": { "MajorVersion": 1, "MinorVersion": 2, "PatchVersion": 3 },
            "UniqueID": "someone.PrettyPaths",
            "ContentPackFor": { "UniqueID": "Pathoschild.ContentPatcher", "MinimumVersion": "2.0.0" }
        }"#;
        let m = parse(text).unwrap();
        assert_eq!(m.version, "1.2.3");
        let host = m.content_pack_for.unwrap();
        assert_eq!(host.unique_id, "Pathoschild.ContentPatcher");
        assert!(host.is_required);
    }

    #[test]
    fn rejects_manifest_without_id() {
        assert!(parse(r#"{"Name": "x", "UniqueID": " "}"#).is_err());
        assert!(parse("not json").is_err());
    }
}
