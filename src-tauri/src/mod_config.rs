//! Reading and writing a mod's `config.json`.
//!
//! SMAPI mods create `config.json` themselves on first launch from defaults compiled
//! into their DLL, so before that there is nothing to edit. Content Patcher packs are
//! different: their `content.json` declares a `ConfigSchema` with allowed values and
//! defaults, which is enough to build the file and a proper form without the game.

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{Map, Value};

use crate::error::{AppError, AppResult};

const CONFIG: &str = "config.json";
const CONTENT: &str = "content.json";

/// One option from a Content Patcher `ConfigSchema`.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SchemaField {
    pub key: String,
    /// Empty means any value is allowed.
    pub allow_values: Vec<String>,
    pub allow_blank: bool,
    pub allow_multiple: bool,
    pub default: Vec<String>,
    pub description: Option<String>,
    pub section: Option<String>,
    /// `AllowValues` is exactly "true, false".
    pub boolean: bool,
}

/// Everything the editor needs for one mod.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModConfig {
    pub path: PathBuf,
    /// Current `config.json`, or `None` before it exists.
    pub values: Option<Value>,
    /// Content Patcher schema, when the mod is a content pack that declares one.
    pub schema: Option<Vec<SchemaField>>,
}

/// Loads `config.json` and, for content packs, the schema in `content.json`.
pub fn load(mod_dir: &Path) -> AppResult<ModConfig> {
    let path = mod_dir.join(CONFIG);
    let values = match std::fs::read_to_string(&path) {
        Ok(text) => Some(parse_lax(&text)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.into()),
    };
    let schema = match std::fs::read_to_string(mod_dir.join(CONTENT)) {
        Ok(text) => cp_schema(&parse_lax(&text)?),
        Err(_) => None,
    };
    Ok(ModConfig {
        path,
        values,
        schema,
    })
}

/// Reports whether the content pack in `mod_dir` declares a `ConfigSchema`, without
/// parsing the whole `content.json`, which can run to megabytes.
pub fn declares_schema(mod_dir: &Path) -> bool {
    std::fs::read_to_string(mod_dir.join(CONTENT))
        .map(|t| t.to_ascii_lowercase().contains("\"configschema\""))
        .unwrap_or(false)
}

/// Writes `values` as `config.json`, pretty-printed in the key order given.
pub fn save(mod_dir: &Path, values: &Value) -> AppResult<()> {
    if !values.is_object() {
        return Err(AppError::BadConfig("config must be a JSON object".into()));
    }
    let text = serde_json::to_string_pretty(values).expect("json value serializes");
    let tmp = mod_dir.join("config.json.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(tmp, mod_dir.join(CONFIG))?;
    Ok(())
}

/// Builds `config.json` contents from a content pack's schema defaults.
pub fn defaults(schema: &[SchemaField]) -> Value {
    let mut map = Map::new();
    for f in schema {
        let value = if f.boolean {
            Value::Bool(
                f.default
                    .first()
                    .is_none_or(|d| d.eq_ignore_ascii_case("true")),
            )
        } else {
            Value::String(f.default.join(", "))
        };
        map.insert(f.key.clone(), value);
    }
    Value::Object(map)
}

// mods ship config and content files with comments and trailing commas, as SMAPI allows
fn parse_lax(text: &str) -> AppResult<Value> {
    json5::from_str(text.trim_start_matches('\u{feff}'))
        .map_err(|e| AppError::BadConfig(e.to_string()))
}

fn get_ci<'a>(obj: &'a Map<String, Value>, key: &str) -> Option<&'a Value> {
    obj.iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .map(|(_, v)| v)
}

fn csv(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::String(s)) => s
            .split(',')
            .map(|p| p.trim().to_owned())
            .filter(|p| !p.is_empty())
            .collect(),
        Some(Value::Bool(b)) => vec![b.to_string()],
        Some(Value::Number(n)) => vec![n.to_string()],
        _ => Vec::new(),
    }
}

fn cp_schema(content: &Value) -> Option<Vec<SchemaField>> {
    let schema = get_ci(content.as_object()?, "ConfigSchema")?.as_object()?;
    let fields = schema
        .iter()
        .filter_map(|(key, def)| {
            let def = def.as_object()?;
            let allow_values = csv(get_ci(def, "AllowValues"));
            let flag = |k: &str| get_ci(def, k).and_then(Value::as_bool).unwrap_or(false);
            let text = |k: &str| get_ci(def, k).and_then(Value::as_str).map(str::to_owned);
            let boolean = allow_values.len() == 2
                && allow_values.iter().any(|v| v.eq_ignore_ascii_case("true"))
                && allow_values.iter().any(|v| v.eq_ignore_ascii_case("false"));
            Some(SchemaField {
                key: key.clone(),
                allow_blank: flag("AllowBlank"),
                allow_multiple: flag("AllowMultiple"),
                default: csv(get_ci(def, "Default")),
                description: text("Description"),
                section: text("Section"),
                boolean,
                allow_values,
            })
        })
        .collect();
    Some(fields)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONTENT_JSON: &str = r#"{
        "Format": "2.0.0",
        // comment
        "configSchema": {
            "EnableJohn": { "AllowValues": "true, false", "Default": true },
            "Material": { "AllowValues": "Wood, Metal", "Default": "Wood", "Description": "Billboard look", "Section": "Looks" },
            "Seasons": { "AllowValues": "spring, summer, fall, winter", "AllowMultiple": true, "Default": "spring, fall" },
            "Name": { "AllowBlank": true },
        },
        "Changes": []
    }"#;

    #[test]
    fn reads_content_patcher_schema() {
        let schema = cp_schema(&parse_lax(CONTENT_JSON).unwrap()).unwrap();
        assert_eq!(schema.len(), 4);
        assert!(schema[0].boolean);
        assert_eq!(schema[1].allow_values, ["Wood", "Metal"]);
        assert_eq!(schema[1].section.as_deref(), Some("Looks"));
        assert!(schema[2].allow_multiple);
        assert!(schema[3].allow_values.is_empty() && schema[3].allow_blank);
        let d = defaults(&schema);
        assert_eq!(
            d,
            serde_json::json!({ "EnableJohn": true, "Material": "Wood", "Seasons": "spring, fall", "Name": "" })
        );
    }

    #[test]
    fn loads_saves_and_keeps_key_order() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load(dir.path()).unwrap().values.is_none());
        std::fs::write(
            dir.path().join(CONFIG),
            "{ \"Zeta\": 1, \"Alpha\": { \"Key\": \"F1\" }, }",
        )
        .unwrap();
        let mut cfg = load(dir.path()).unwrap().values.unwrap();
        cfg["Zeta"] = Value::from(2);
        save(dir.path(), &cfg).unwrap();
        let text = std::fs::read_to_string(dir.path().join(CONFIG)).unwrap();
        assert!(text.find("Zeta").unwrap() < text.find("Alpha").unwrap());
        assert_eq!(load(dir.path()).unwrap().values.unwrap()["Zeta"], 2);
        assert!(save(dir.path(), &Value::from(3)).is_err());
    }
}
