//! The mod store: every installed mod package, kept outside the game folder.
//!
//! A package is one top-level folder under the store root, as it came out of a download.
//! It holds one mod or several (big mods ship `[CP]`, `[FTM]`… parts together). Profiles
//! enable packages, not single mods, because parts of one download are versioned together.
//!
//! Folder discovery follows SMAPI's rules: a folder with `manifest.json` is a mod and is
//! not searched further, other folders are searched, names starting with `.` are skipped.
//! The store uses that last rule for its own staging folders.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::archive;
use crate::backups;
use crate::error::{AppError, AppResult};
use crate::manifest::{self, Manifest};

/// SMAPI stops looking for manifests this deep; so do we.
const MAX_DEPTH: usize = 6;

/// A mod found inside a package.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FoundMod {
    /// Folder of the mod relative to the package root, `""` when it is the root.
    pub rel_path: String,
    pub manifest: Manifest,
}

/// A folder that looked like a mod but whose manifest did not parse.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrokenMod {
    pub rel_path: String,
    pub error: String,
}

/// One installed download.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Package {
    pub id: String,
    pub path: PathBuf,
    pub mods: Vec<FoundMod>,
    pub broken: Vec<BrokenMod>,
}

impl Package {
    /// Unique IDs of the mods in this package.
    pub fn unique_ids(&self) -> impl Iterator<Item = &str> {
        self.mods.iter().map(|m| m.manifest.unique_id.as_str())
    }
}

/// Result of installing one package.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Installed {
    pub id: String,
    /// Packages this one replaced because they shared a mod.
    pub replaced: Vec<String>,
}

/// Reads the package at `path`; `None` when it holds no mod folders at all.
pub fn read_package(id: String, path: PathBuf) -> Option<Package> {
    let mut mods = Vec::new();
    let mut broken = Vec::new();
    walk(&path, &path, 0, &mut mods, &mut broken);
    if mods.is_empty() && broken.is_empty() {
        return None;
    }
    mods.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    Some(Package {
        id,
        path,
        mods,
        broken,
    })
}

/// Lists the packages in `root`, sorted by folder name.
pub fn scan(root: &Path) -> Vec<Package> {
    let mut out: Vec<Package> = child_dirs(root)
        .filter_map(|(name, path)| read_package(name, path))
        .collect();
    out.sort_by_key(|p| p.id.to_lowercase());
    out
}

/// Installs every package in the archive at `archive` into `root`; packages it replaces
/// move to `backups`.
pub fn install_archive(root: &Path, backups: &Path, archive: &Path) -> AppResult<Vec<Installed>> {
    let staging = Staging::new(root)?;
    archive::extract(archive, &staging.path)?;
    let fallback = archive
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Mod".into());
    install_staged(root, backups, &staging.path, &fallback)
}

/// Copies the mod folder `src` into `root` as a package; packages it replaces move to
/// `backups`.
pub fn install_dir(root: &Path, backups: &Path, src: &Path) -> AppResult<Vec<Installed>> {
    let staging = Staging::new(root)?;
    let name = src
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Mod".into());
    copy_dir(src, &staging.path.join(&name))?;
    install_staged(root, backups, &staging.path, &name)
}

/// Deletes the package `id` from `root`.
pub fn remove(root: &Path, id: &str) -> AppResult<()> {
    let path = package_path(root, id)?;
    std::fs::remove_dir_all(path)?;
    Ok(())
}

/// Resolves a package id to its folder, refusing ids that are not plain folder names.
pub fn package_path(root: &Path, id: &str) -> AppResult<PathBuf> {
    let plain = !id.is_empty() && !id.starts_with('.') && !id.contains(['/', '\\', ':']);
    if !plain {
        return Err(AppError::NotFound(id.to_owned()));
    }
    let path = root.join(id);
    if !path.is_dir() {
        return Err(AppError::NotFound(id.to_owned()));
    }
    Ok(path)
}

fn install_staged(
    root: &Path,
    backups: &Path,
    staging: &Path,
    fallback: &str,
) -> AppResult<Vec<Installed>> {
    let candidates = package_roots(staging, fallback);
    if candidates.is_empty() {
        return Err(AppError::NoMods);
    }
    let mut existing = scan(root);
    let mut out = Vec::new();
    for (path, name) in candidates {
        let Some(pkg) = read_package(name.clone(), path.clone()) else {
            continue;
        };
        let ids: HashSet<&str> = pkg.unique_ids().collect();
        let (replaced, kept): (Vec<Package>, Vec<Package>) = existing
            .into_iter()
            .partition(|old| old.unique_ids().any(|id| ids.contains(id)));
        existing = kept;

        for old in &replaced {
            carry_configs(old, &pkg)?;
        }
        for old in &replaced {
            backups::stash(backups, old)?;
        }
        let target = free_name(root, &sanitize(&name));
        std::fs::rename(&path, root.join(&target))?;
        out.push(Installed {
            id: target,
            replaced: replaced.into_iter().map(|p| p.id).collect(),
        });
    }
    Ok(out)
}

// config.json is written by the mod at runtime and never ships in the download, so an
// update that replaces the folder would silently reset the player's settings
fn carry_configs(old: &Package, new: &Package) -> AppResult<()> {
    for m in &old.mods {
        let from = old.path.join(&m.rel_path).join("config.json");
        if !from.is_file() {
            continue;
        }
        let Some(target) = new.mods.iter().find(|n| {
            n.manifest
                .unique_id
                .eq_ignore_ascii_case(&m.manifest.unique_id)
        }) else {
            continue;
        };
        let to = new.path.join(&target.rel_path).join("config.json");
        if !to.exists() {
            std::fs::copy(from, to)?;
        }
    }
    Ok(())
}

// a manifest right at the top means the archive has no wrapper folder: the whole
// staging dir is one package, named after the mod
fn package_roots(staging: &Path, fallback: &str) -> Vec<(PathBuf, String)> {
    if staging.join(manifest::FILE_NAME).is_file() {
        let name = manifest::read(staging)
            .map(|m| m.name)
            .unwrap_or_else(|_| fallback.to_owned());
        return vec![(staging.to_path_buf(), name)];
    }
    let children: Vec<(String, PathBuf)> = child_dirs(staging).collect();
    // a lone wrapper folder without a manifest is just packaging: "Mod-123-1-0/Mod/…"
    if let [(_, only)] = children.as_slice()
        && !only.join(manifest::FILE_NAME).is_file()
    {
        let inner: Vec<(String, PathBuf)> = child_dirs(only).collect();
        if inner.len() == 1 {
            return package_roots(only, fallback);
        }
    }
    children
        .into_iter()
        .filter(|(_, path)| read_package(String::new(), path.clone()).is_some())
        .map(|(name, path)| (path, name))
        .collect()
}

fn walk(
    root: &Path,
    dir: &Path,
    depth: usize,
    mods: &mut Vec<FoundMod>,
    broken: &mut Vec<BrokenMod>,
) {
    if dir.join(manifest::FILE_NAME).is_file() {
        let rel_path = dir
            .strip_prefix(root)
            .unwrap_or(dir)
            .to_string_lossy()
            .into_owned();
        match manifest::read(dir) {
            Ok(manifest) => mods.push(FoundMod { rel_path, manifest }),
            Err(e) => broken.push(BrokenMod {
                rel_path,
                error: e.to_string(),
            }),
        }
        return;
    }
    if depth >= MAX_DEPTH {
        return;
    }
    for (_, child) in child_dirs(dir) {
        walk(root, &child, depth + 1, mods, broken);
    }
}

fn child_dirs(dir: &Path) -> impl Iterator<Item = (String, PathBuf)> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let path = e.path();
            (!name.starts_with('.') && path.is_dir()).then_some((name, path))
        })
}

fn sanitize(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':') || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim().trim_start_matches('.').trim();
    if cleaned.is_empty() {
        "Mod".into()
    } else {
        cleaned.to_owned()
    }
}

fn free_name(root: &Path, name: &str) -> String {
    if !root.join(name).exists() {
        return name.to_owned();
    }
    (2..)
        .map(|i| format!("{name} ({i})"))
        .find(|n| !root.join(n).exists())
        .expect("some suffix is free")
}

fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let to = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir(&entry.path(), &to)?;
        } else if ty.is_file() {
            std::fs::copy(entry.path(), to)?;
        }
    }
    Ok(())
}

/// A scratch folder inside the store, so finished packages move in with a rename.
struct Staging {
    path: PathBuf,
}

impl Staging {
    fn new(root: &Path) -> AppResult<Self> {
        std::fs::create_dir_all(root)?;
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = root.join(format!(".staging-{nanos}"));
        std::fs::create_dir_all(&path)?;
        Ok(Self { path })
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::tests::make_zip;

    fn manifest(name: &str, id: &str, version: &str) -> String {
        format!(r#"{{"Name":"{name}","UniqueID":"{id}","Version":"{version}"}}"#)
    }

    #[test]
    fn installs_zip_with_wrapper_folder() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("store");
        let zip = dir.path().join("LookupAnything 1.49.zip");
        make_zip(
            &zip,
            &[(
                "LookupAnything/manifest.json",
                &manifest("Lookup Anything", "P.Lookup", "1.49.0"),
            )],
        );
        let res = install_archive(&root, &dir.path().join("backups"), &zip).unwrap();
        assert_eq!(res[0].id, "LookupAnything");
        let pkgs = scan(&root);
        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs[0].mods[0].rel_path, "");
        assert!(!std::fs::read_dir(&root).unwrap().any(|e| {
            e.unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".staging")
        }));
    }

    #[test]
    fn installs_bare_zip_named_after_mod() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("store");
        let zip = dir.path().join("x.zip");
        make_zip(
            &zip,
            &[(
                "manifest.json",
                &manifest("Pretty: Paths", "s.Paths", "1.0"),
            )],
        );
        assert_eq!(
            install_archive(&root, &dir.path().join("backups"), &zip).unwrap()[0].id,
            "Pretty_ Paths"
        );
    }

    #[test]
    fn groups_multi_part_mod_into_one_package() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("store");
        let zip = dir.path().join("sve.zip");
        make_zip(
            &zip,
            &[
                (
                    "Stardew Valley Expanded/[CP] SVE/manifest.json",
                    &manifest("SVE", "F.SVE", "1.15"),
                ),
                (
                    "Stardew Valley Expanded/[FTM] SVE/manifest.json",
                    &manifest("SVE FTM", "F.SVE.FTM", "1.15"),
                ),
            ],
        );
        let res = install_archive(&root, &dir.path().join("backups"), &zip).unwrap();
        assert_eq!(res.len(), 1);
        let pkg = &scan(&root)[0];
        assert_eq!(pkg.id, "Stardew Valley Expanded");
        assert_eq!(pkg.mods.len(), 2);
    }

    #[test]
    fn update_replaces_package_and_keeps_config() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("store");
        let v1 = dir.path().join("v1.zip");
        make_zip(
            &v1,
            &[(
                "Lookup/manifest.json",
                &manifest("Lookup", "P.Lookup", "1.0.0"),
            )],
        );
        install_archive(&root, &dir.path().join("backups"), &v1).unwrap();
        std::fs::write(root.join("Lookup/config.json"), r#"{"Key":"F2"}"#).unwrap();

        let v2 = dir.path().join("v2.zip");
        make_zip(
            &v2,
            &[(
                "Lookup Anything/manifest.json",
                &manifest("Lookup", "P.Lookup", "1.1.0"),
            )],
        );
        let res = install_archive(&root, &dir.path().join("backups"), &v2).unwrap();
        assert_eq!(res[0].id, "Lookup Anything");
        assert_eq!(res[0].replaced, ["Lookup"]);
        let pkgs = scan(&root);
        assert_eq!(pkgs.len(), 1);
        assert_eq!(pkgs[0].mods[0].manifest.version, "1.1.0");
        let saved = backups::list(&dir.path().join("backups"), "P.Lookup");
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].version, "1.0.0");
        let cfg = std::fs::read_to_string(root.join("Lookup Anything/config.json")).unwrap();
        assert!(cfg.contains("F2"));
    }

    #[test]
    fn reports_broken_manifest_and_rejects_empty_zip() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("store");
        let zip = dir.path().join("bad.zip");
        make_zip(&zip, &[("Bad/manifest.json", "{ nope")]);
        install_archive(&root, &dir.path().join("backups"), &zip).unwrap();
        assert_eq!(scan(&root)[0].broken.len(), 1);

        let empty = dir.path().join("empty.zip");
        make_zip(&empty, &[("readme.txt", "hi")]);
        assert!(matches!(
            install_archive(&root, &dir.path().join("backups"), &empty),
            Err(AppError::NoMods)
        ));
    }

    #[test]
    fn name_collision_gets_suffix_and_ids_are_checked() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("store");
        for (i, id) in ["a.One", "a.Two"].iter().enumerate() {
            let zip = dir.path().join(format!("{i}.zip"));
            make_zip(&zip, &[("Mod/manifest.json", &manifest("Mod", id, "1.0"))]);
            install_archive(&root, &dir.path().join("backups"), &zip).unwrap();
        }
        let ids: Vec<String> = scan(&root).into_iter().map(|p| p.id).collect();
        assert_eq!(ids, ["Mod", "Mod (2)"]);
        assert!(package_path(&root, "../etc").is_err());
        assert!(package_path(&root, ".staging-1").is_err());
        remove(&root, "Mod (2)").unwrap();
        assert_eq!(scan(&root).len(), 1);
    }

    #[test]
    fn imports_folder() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("game/Mods/Automate");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(
            src.join("manifest.json"),
            manifest("Automate", "P.Automate", "2.3.1"),
        )
        .unwrap();
        let root = dir.path().join("store");
        assert_eq!(
            install_dir(&root, &dir.path().join("backups"), &src).unwrap()[0].id,
            "Automate"
        );
        assert!(src.join("manifest.json").is_file());
    }
}
