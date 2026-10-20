//! The player's own translations of mods that don't ship their language.
//!
//! SMAPI reads `i18n/<language>.json` next to `i18n/default.json`. A translation made here
//! is written there and kept in the app's data as well, since an update replaces the mod's
//! folder; installs put the kept copy back when the new version still lacks the language.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{AppError, AppResult};
use crate::library::Package;

/// One line to translate.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Line {
    pub key: String,
    /// The mod's own text, from `default.json`.
    pub source: String,
    /// The translation so far; empty when there is none.
    pub text: String,
}

/// Lines of the mod in `mod_dir` with what's translated into `language` so far: the
/// player's kept copy wins over a file in the mod.
pub fn load(root: &Path, unique_id: &str, mod_dir: &Path, language: &str) -> AppResult<Vec<Line>> {
    let source = strings(&mod_dir.join("i18n/default.json"))?;
    let mut done = strings(&mod_dir.join(format!("i18n/{language}.json"))).unwrap_or_default();
    done.extend(strings(&kept(root, unique_id, language)).unwrap_or_default());
    Ok(source
        .into_iter()
        .map(|(key, source)| Line {
            text: done.get(&key).cloned().unwrap_or_default(),
            key,
            source,
        })
        .collect())
}

/// Writes the non-empty `texts` as the mod's `language` file and keeps a copy.
pub fn save(
    root: &Path,
    unique_id: &str,
    mod_dir: &Path,
    language: &str,
    texts: &BTreeMap<String, String>,
) -> AppResult<()> {
    check_language(language)?;
    let kept_texts: BTreeMap<&String, &String> =
        texts.iter().filter(|(_, v)| !v.trim().is_empty()).collect();
    let body = serde_json::to_vec_pretty(&kept_texts).expect("translation serialize");
    let copy = kept(root, unique_id, language);
    if let Some(dir) = copy.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&copy, &body)?;
    std::fs::write(mod_dir.join(format!("i18n/{language}.json")), &body)?;
    Ok(())
}

/// Puts kept translations back into `packages` whose mods lack them, as after an update;
/// returns how many files it wrote.
pub fn reapply(root: &Path, packages: &[Package]) -> u32 {
    let mut written = 0;
    for p in packages {
        for m in &p.mods {
            let i18n = p.path.join(&m.rel_path).join("i18n");
            let dir = root.join(folder(&m.manifest.unique_id));
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            if !i18n.is_dir() {
                continue;
            }
            for e in entries.flatten() {
                let target = i18n.join(e.file_name());
                if !target.exists() && std::fs::copy(e.path(), &target).is_ok() {
                    written += 1;
                }
            }
        }
    }
    written
}

fn kept(root: &Path, unique_id: &str, language: &str) -> PathBuf {
    root.join(folder(unique_id))
        .join(format!("{language}.json"))
}

// UniqueIDs are the mod's word; keep them to one plain folder name
fn folder(unique_id: &str) -> String {
    unique_id
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_start_matches('.')
        .to_lowercase()
}

fn check_language(language: &str) -> AppResult<()> {
    let plain = !language.is_empty()
        && language.len() <= 8
        && language
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-');
    if plain && language != "default" {
        Ok(())
    } else {
        Err(AppError::NotFound(language.to_owned()))
    }
}

// a translation file is a flat object of strings; anything else is skipped
fn strings(file: &Path) -> AppResult<BTreeMap<String, String>> {
    let value: serde_json::Value =
        crate::jsonc::parse(&crate::jsonc::read(file)?).map_err(AppError::BadConfig)?;
    Ok(value
        .as_object()
        .map(|o| {
            o.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned())))
                .collect()
        })
        .unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translates_keeps_a_copy_and_puts_it_back() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("mods");
        let m = store.join("Cozy");
        std::fs::create_dir_all(m.join("i18n")).unwrap();
        std::fs::write(
            m.join("manifest.json"),
            r#"{"Name":"Cozy","Author":"a","Version":"1.0.0","Description":"","UniqueID":"a.Cozy","EntryDll":"x.dll"}"#,
        )
        .unwrap();
        // comments and trailing commas, as mods ship them
        std::fs::write(
            m.join("i18n/default.json"),
            "{ \"hello\": \"Hello\", // greeting\n \"bye\": \"Bye\", }",
        )
        .unwrap();
        let root = dir.path().join("translations");

        let lines = load(&root, "a.Cozy", &m, "ru").unwrap();
        assert_eq!(lines.len(), 2);
        assert!(lines.iter().all(|l| l.text.is_empty()));

        let texts = BTreeMap::from([
            ("hello".to_owned(), "Привет".to_owned()),
            ("bye".to_owned(), " ".to_owned()),
        ]);
        save(&root, "a.Cozy", &m, "ru", &texts).unwrap();
        let written: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(m.join("i18n/ru.json")).unwrap())
                .unwrap();
        assert_eq!(
            written,
            serde_json::json!({ "hello": "Привет" }),
            "blank lines stay out"
        );
        assert_eq!(load(&root, "a.Cozy", &m, "ru").unwrap()[1].text, "Привет");

        // an update brings a fresh folder without it
        std::fs::remove_file(m.join("i18n/ru.json")).unwrap();
        assert_eq!(reapply(&root, &crate::library::scan(&store)), 1);
        assert!(m.join("i18n/ru.json").is_file());
        assert!(save(&root, "a.Cozy", &m, "../x", &texts).is_err());
    }
}
