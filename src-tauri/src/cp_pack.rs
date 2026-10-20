//! Retexture packs from the player's own pictures: each PNG replaces one game asset
//! ("Portraits/Abigail") through Content Patcher, so no file of the game is touched.
//!
//! The pack is an ordinary Content Patcher mod (manifest, content.json, assets/) that the
//! library installs like any other and can update, disable or remove.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::error::{AppError, AppResult};

const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";
/// The game's largest sheets are a few MB; a picture far past this is a mistake.
const MAX_PNG: u64 = 32 << 20;
const CONTENT_PATCHER: &str = "Pathoschild.ContentPatcher";

/// One picture and the asset it replaces.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Replacement {
    pub file: PathBuf,
    /// The asset name as the game loads it, without extension: "Portraits/Abigail".
    pub target: String,
}

// asset names are folder/file words; anything else is a typo or a path escaping assets/
fn check_target(target: &str) -> AppResult<()> {
    let ok = !target.is_empty()
        && target.len() <= 200
        && target.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && part
                    .chars()
                    .all(|c| c.is_alphanumeric() || " _-.'".contains(c))
        });
    if ok {
        Ok(())
    } else {
        Err(AppError::NotFound(target.to_owned()))
    }
}

fn check_png(file: &Path) -> AppResult<()> {
    let meta = std::fs::metadata(file)?;
    let mut head = [0u8; 8];
    let read =
        std::fs::File::open(file).and_then(|mut f| std::io::Read::read_exact(&mut f, &mut head));
    if !meta.is_file() || meta.len() > MAX_PNG || read.is_err() || head != PNG_MAGIC {
        return Err(AppError::BadArchive(format!(
            "{}: not a PNG",
            file.display()
        )));
    }
    Ok(())
}

fn slug(name: &str) -> String {
    let s: String = name.chars().filter(char::is_ascii_alphanumeric).collect();
    if s.is_empty() {
        format!("Pack{}", crate::history::now_ms())
    } else {
        s
    }
}

/// Writes pack `name` by `author` replacing `items` into a new folder under `out`;
/// returns the folder.
pub fn build(out: &Path, name: &str, author: &str, items: &[Replacement]) -> AppResult<PathBuf> {
    let name = name.trim();
    if name.is_empty() || items.is_empty() {
        return Err(AppError::NoMods);
    }
    for r in items {
        check_target(&r.target)?;
        check_png(&r.file)?;
    }
    let author = if author.trim().is_empty() {
        "Pelican Mods"
    } else {
        author.trim()
    };
    let id = slug(name);
    let dir = out.join(format!("[CP] {id}"));
    let assets = dir.join("assets");
    std::fs::create_dir_all(&assets)?;
    let mut changes = Vec::new();
    for (i, r) in items.iter().enumerate() {
        // numbered, so two pictures both called portrait.png can't overwrite each other
        let file = format!("{:02}-{}.png", i + 1, slug(&r.target));
        std::fs::copy(&r.file, assets.join(&file))?;
        changes.push(json!({
            "Action": "Load",
            "Target": r.target,
            "FromFile": format!("assets/{file}"),
        }));
    }
    let manifest = json!({
        "Name": name,
        "Author": author,
        "Version": "1.0.0",
        "Description": format!("{} pictures replaced with Content Patcher.", items.len()),
        "UniqueID": format!("{}.{id}", slug(author)),
        "UpdateKeys": [],
        "ContentPackFor": { "UniqueID": CONTENT_PATCHER },
    });
    let content = json!({ "Format": "2.0.0", "Changes": changes });
    std::fs::write(
        dir.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).expect("manifest"),
    )?;
    std::fs::write(
        dir.join("content.json"),
        serde_json::to_vec_pretty(&content).expect("content"),
    )?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_a_content_patcher_pack() {
        let dir = tempfile::tempdir().unwrap();
        let png = dir.path().join("portrait.png");
        std::fs::write(&png, [PNG_MAGIC, b"rest"].concat()).unwrap();
        let items = vec![
            Replacement {
                file: png.clone(),
                target: "Portraits/Abigail".into(),
            },
            Replacement {
                file: png.clone(),
                target: "Characters/Abigail".into(),
            },
        ];
        let pack = build(dir.path(), "Моя Абигейл 2", "Аня", &items).unwrap();
        let packages = crate::library::scan(dir.path());
        let m = &packages.iter().find(|p| p.path == pack).unwrap().mods[0].manifest;
        assert_eq!(m.name, "Моя Абигейл 2");
        // non-ASCII names leave a time-based id rather than an empty one
        assert!(
            m.unique_id.starts_with("Pack") && m.unique_id.contains(".2"),
            "{}",
            m.unique_id
        );
        let content: serde_json::Value =
            serde_json::from_slice(&std::fs::read(pack.join("content.json")).unwrap()).unwrap();
        assert_eq!(content["Changes"][1]["Target"], "Characters/Abigail");
        assert_eq!(
            content["Changes"][1]["FromFile"],
            "assets/02-CharactersAbigail.png"
        );
        assert!(pack.join("assets/01-PortraitsAbigail.png").is_file());

        let bad = |target: &str, file: &Path| {
            build(
                dir.path(),
                "x",
                "a",
                &[Replacement {
                    file: file.into(),
                    target: target.into(),
                }],
            )
        };
        assert!(bad("../../etc", &png).is_err());
        assert!(bad("Portraits//Abigail", &png).is_err());
        let txt = dir.path().join("a.png");
        std::fs::write(&txt, "not a picture").unwrap();
        assert!(bad("Portraits/Abigail", &txt).is_err());
    }
}
