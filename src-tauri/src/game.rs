//! Locating a Stardew Valley install and reading what is in it.
//!
//! Every path here is the folder holding `Stardew Valley.dll`, which on macOS is
//! `Stardew Valley.app/Contents/MacOS`. Users may point at the `.app` bundle instead;
//! [`normalize`] maps that to the inner folder.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{AppError, AppResult};
use crate::rosetta;

const GAME_DLL: &str = "Stardew Valley.dll";
const GAME_DEPS: &str = "Stardew Valley.deps.json";
pub const VANILLA_EXE: &str = "Stardew Valley";
pub const SMAPI_EXE: &str = "StardewModdingAPI";
const SMAPI_DLL: &str = "StardewModdingAPI.dll";

/// Distribution channel the install came from, as far as files on disk tell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Store {
    Steam,
    Gog,
    Other,
}

/// A validated game folder and what is installed in it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameInstall {
    pub path: PathBuf,
    pub version: Option<String>,
    pub store: Store,
    pub smapi_version: Option<String>,
    pub mods_path: PathBuf,
    /// The game binary is Intel-only and Rosetta 2 is not installed.
    pub needs_rosetta: bool,
}

/// Returns the default install locations that exist on this machine.
pub fn detect(home: &Path) -> Vec<PathBuf> {
    let candidates = [
        PathBuf::from("/Applications/Stardew Valley.app"),
        home.join("Applications/Stardew Valley.app"),
        home.join(
            "Library/Application Support/Steam/steamapps/common/Stardew Valley/Contents/MacOS",
        ),
        home.join("Library/Application Support/Steam/steamapps/common/Stardew Valley"),
    ];
    let mut found: Vec<PathBuf> = candidates
        .iter()
        .filter_map(|c| normalize(c).ok())
        .collect();
    found.dedup();
    found
}

/// Resolves a user-chosen path (the `.app`, its `Contents`, or the game folder itself)
/// to the folder holding the game DLL.
pub fn normalize(path: &Path) -> AppResult<PathBuf> {
    let tries = [
        path.to_path_buf(),
        path.join("Contents/MacOS"),
        path.join("MacOS"),
        path.join("Stardew Valley.app/Contents/MacOS"),
    ];
    tries
        .into_iter()
        .find(|p| p.join(GAME_DLL).is_file())
        .ok_or_else(|| AppError::GameNotFound(path.display().to_string()))
}

/// Reads version, store and SMAPI state of the game folder at `path`.
pub fn inspect(path: &Path) -> AppResult<GameInstall> {
    let path = normalize(path)?;
    Ok(GameInstall {
        version: game_version(&path),
        store: store(&path),
        smapi_version: smapi_version(&path),
        mods_path: path.join("Mods"),
        needs_rosetta: rosetta::missing_for(&path.join(VANILLA_EXE)),
        path,
    })
}

/// Reports whether SMAPI's launcher is present in the game folder.
pub fn has_smapi(path: &Path) -> bool {
    path.join(SMAPI_EXE).is_file() && path.join(SMAPI_DLL).is_file()
}

fn game_version(path: &Path) -> Option<String> {
    let raw = std::fs::read_to_string(path.join(GAME_DEPS)).ok()?;
    version_from_deps(&raw)
}

// the game's own entry in deps.json is keyed "Stardew Valley/<full version>"
fn version_from_deps(raw: &str) -> Option<String> {
    let json: serde_json::Value = serde_json::from_str(raw).ok()?;
    json.get("libraries")?
        .as_object()?
        .keys()
        .find_map(|k| k.strip_prefix("Stardew Valley/"))
        .map(str::to_owned)
}

fn store(path: &Path) -> Store {
    let p = path.to_string_lossy();
    if p.contains("/steamapps/") {
        return Store::Steam;
    }
    let resources = path.with_file_name("Resources");
    let gog = std::fs::read_dir(resources)
        .map(|it| {
            it.flatten()
                .any(|e| e.file_name().to_string_lossy().starts_with("goggame-"))
        })
        .unwrap_or(false);
    if gog { Store::Gog } else { Store::Other }
}

fn smapi_version(path: &Path) -> Option<String> {
    if !has_smapi(path) {
        return None;
    }
    let bytes = std::fs::read(path.join(SMAPI_DLL)).ok()?;
    Some(pe_product_version(&bytes).unwrap_or_else(|| "?".into()))
}

/// Pulls `ProductVersion` out of a PE file's VS_VERSIONINFO resource, without the
/// `+<commit>` build metadata .NET appends.
fn pe_product_version(bytes: &[u8]) -> Option<String> {
    let key: Vec<u8> = "ProductVersion\0"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    let start = bytes.windows(key.len()).position(|w| w == key.as_slice())? + key.len();
    // String structs pad the key to a 32-bit boundary before the value; resource
    // sections are themselves aligned, so file offsets align the same way
    let start = start.next_multiple_of(4);
    let units: Vec<u16> = bytes
        .get(start..)?
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&c| u16::from_le_bytes(c))
        .take_while(|&u| u != 0)
        .take(64)
        .collect();
    let s = String::from_utf16(&units).ok()?;
    let v = s.split('+').next()?.trim();
    (!v.is_empty()).then(|| v.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_version_from_deps() {
        let raw =
            r#"{"libraries":{"Foo/1.0":{},"Stardew Valley/1.6.15.24356":{"type":"project"}}}"#;
        assert_eq!(version_from_deps(raw).as_deref(), Some("1.6.15.24356"));
        assert_eq!(version_from_deps("{}"), None);
    }

    #[test]
    fn reads_pe_product_version() {
        let mut bytes = vec![0u8; 6];
        bytes.extend("ProductVersion\0".encode_utf16().flat_map(u16::to_le_bytes));
        while !bytes.len().is_multiple_of(4) {
            bytes.push(0);
        }
        bytes.extend("4.5.2+821167e5".encode_utf16().flat_map(u16::to_le_bytes));
        bytes.extend([0, 0]);
        assert_eq!(pe_product_version(&bytes).as_deref(), Some("4.5.2"));
    }

    #[test]
    fn normalizes_app_bundle_and_folder() {
        let dir = tempfile::tempdir().unwrap();
        let macos = dir.path().join("Stardew Valley.app/Contents/MacOS");
        std::fs::create_dir_all(&macos).unwrap();
        std::fs::write(macos.join(GAME_DLL), b"").unwrap();
        let app = dir.path().join("Stardew Valley.app");
        assert_eq!(normalize(&app).unwrap(), macos);
        assert_eq!(normalize(&macos).unwrap(), macos);
        assert_eq!(normalize(dir.path()).unwrap(), macos);
        assert!(normalize(&dir.path().join("nope")).is_err());
    }

    #[test]
    fn inspects_gog_install_without_smapi() {
        let dir = tempfile::tempdir().unwrap();
        let macos = dir.path().join("Contents/MacOS");
        let res = dir.path().join("Contents/Resources");
        std::fs::create_dir_all(&macos).unwrap();
        std::fs::create_dir_all(&res).unwrap();
        std::fs::write(macos.join(GAME_DLL), b"").unwrap();
        std::fs::write(res.join("goggame-1.info"), b"").unwrap();
        let g = inspect(dir.path()).unwrap();
        assert_eq!(g.store, Store::Gog);
        assert_eq!(g.smapi_version, None);
        assert_eq!(g.mods_path, macos.join("Mods"));
    }
}
