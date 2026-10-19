//! Unpacking downloaded archives without letting them write outside the target folder.
//!
//! Mod pages ship zip, 7z and rar about equally, often with the wrong extension, so
//! [`extract`] picks the format from the file's magic bytes.

use std::io::Read;
use std::path::{Component, Path, PathBuf};

use crate::error::{AppError, AppResult};

/// Archive formats mods are distributed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Zip,
    SevenZip,
    Rar,
}

/// Detects the archive format from the first bytes of `path`.
pub fn sniff(path: &Path) -> AppResult<Format> {
    let mut head = [0u8; 8];
    let n = std::fs::File::open(path)?.read(&mut head)?;
    let head = &head[..n];
    if head.starts_with(b"PK\x03\x04") || head.starts_with(b"PK\x05\x06") {
        Ok(Format::Zip)
    } else if head.starts_with(b"7z\xbc\xaf\x27\x1c") {
        Ok(Format::SevenZip)
    } else if head.starts_with(b"Rar!\x1a\x07") {
        Ok(Format::Rar)
    } else {
        Err(AppError::BadArchive(format!(
            "{} is not a zip, 7z or rar archive",
            path.display()
        )))
    }
}

/// Extracts any supported archive at `archive` into `out`.
pub fn extract(archive: &Path, out: &Path) -> AppResult<()> {
    match sniff(archive)? {
        Format::Zip => extract_zip(archive, out),
        Format::SevenZip => extract_7z(archive, out),
        Format::Rar => extract_rar(archive, out),
    }
}

/// Packs the folder `src` into a new zip at `dest`, under a top-level folder `root`.
pub fn zip_dir(src: &Path, dest: &Path, root: &str) -> AppResult<()> {
    use std::io::Write;

    let tmp = dest.with_extension("zip.part");
    let mut w = zip::ZipWriter::new(std::fs::File::create(&tmp)?);
    let opts = zip::write::SimpleFileOptions::default().unix_permissions(0o644);
    let mut stack = vec![(src.to_path_buf(), PathBuf::from(root))];
    while let Some((dir, rel)) = stack.pop() {
        w.add_directory(rel.to_string_lossy(), opts)?;
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let ty = entry.file_type()?;
            let child = rel.join(entry.file_name());
            if ty.is_dir() {
                stack.push((entry.path(), child));
            } else if ty.is_file() {
                w.start_file(child.to_string_lossy(), opts)?;
                w.write_all(&std::fs::read(entry.path())?)?;
            }
        }
    }
    w.finish()?;
    // a half-written zip must never look like a finished backup
    std::fs::rename(tmp, dest)?;
    Ok(())
}

// the relative path an entry may be written to, or None when it would escape `out`
fn safe_relative(name: &Path) -> Option<PathBuf> {
    let mut rel = PathBuf::new();
    for c in name.components() {
        match c {
            Component::Normal(part) => rel.push(part),
            Component::CurDir => {}
            _ => return None,
        }
    }
    (!rel.as_os_str().is_empty()).then_some(rel)
}

fn skipped(rel: &Path) -> bool {
    rel.starts_with("__MACOSX")
}

fn extract_7z(archive: &Path, out: &Path) -> AppResult<()> {
    std::fs::create_dir_all(out)?;
    let mut unsafe_name = None;
    sevenz_rust2::decompress_file_with_extract_fn(archive, out, |entry, reader, _| {
        // 7z stores Windows separators; normalize before judging the path
        let name = entry.name().replace('\\', "/");
        let Some(rel) = safe_relative(Path::new(&name)) else {
            unsafe_name = Some(name);
            return Ok(false);
        };
        if skipped(&rel) {
            std::io::copy(reader, &mut std::io::sink())?;
            return Ok(true);
        }
        sevenz_rust2::default_entry_extract_fn(entry, reader, &out.join(rel))
    })
    .map_err(|e| AppError::BadArchive(e.to_string()))?;
    match unsafe_name {
        Some(name) => Err(AppError::BadArchive(format!("unsafe path {name}"))),
        None => Ok(()),
    }
}

fn extract_rar(archive: &Path, out: &Path) -> AppResult<()> {
    std::fs::create_dir_all(out)?;
    let bad = |e: unrar::error::UnrarError| AppError::BadArchive(e.to_string());
    let mut cursor = unrar::Archive::new(archive)
        .open_for_processing()
        .map_err(bad)?;
    while let Some(entry) = cursor.read_header().map_err(bad)? {
        let name = entry.entry().filename.clone();
        let Some(rel) = safe_relative(&name) else {
            return Err(AppError::BadArchive(format!(
                "unsafe path {}",
                name.display()
            )));
        };
        cursor = if skipped(&rel) || entry.entry().is_directory() {
            entry.skip().map_err(bad)?
        } else {
            entry.extract_with_base(out).map_err(bad)?
        };
    }
    Ok(())
}

/// Extracts the zip at `archive` into `out`, keeping unix permission bits.
///
/// Entries that would escape `out` fail the whole extraction; Finder's `__MACOSX`
/// resource-fork folder is skipped.
pub fn extract_zip(archive: &Path, out: &Path) -> AppResult<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut zip = zip::ZipArchive::new(std::fs::File::open(archive)?)?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let Some(rel) = entry.enclosed_name() else {
            return Err(AppError::BadArchive(format!(
                "unsafe path {}",
                entry.name_raw().escape_ascii()
            )));
        };
        if rel.starts_with("__MACOSX") {
            continue;
        }
        let dest = out.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&dest)?;
            continue;
        }
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut buf = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut buf)?;
        std::fs::write(&dest, buf)?;
        if let Some(mode) = entry.unix_mode() {
            // some Windows-made zips carry 0o000; the owner must still be able to update or delete
            let mode = (mode & 0o777) | 0o600;
            std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(mode))?;
        }
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::Write;

    /// Writes a zip at `path` holding `files` as (name, content) pairs.
    pub fn make_zip(path: &Path, files: &[(&str, &str)]) {
        let mut w = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for (name, body) in files {
            w.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            w.write_all(body.as_bytes()).unwrap();
        }
        w.finish().unwrap();
    }

    #[test]
    fn rejects_path_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("a.zip");
        make_zip(&archive, &[("../evil", "x")]);
        assert!(extract_zip(&archive, &dir.path().join("out")).is_err());
    }

    #[test]
    fn skips_macos_resource_forks() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("a.zip");
        make_zip(
            &archive,
            &[
                ("Mod/manifest.json", "{}"),
                ("__MACOSX/Mod/._manifest.json", "x"),
            ],
        );
        let out = dir.path().join("out");
        extract_zip(&archive, &out).unwrap();
        assert!(out.join("Mod/manifest.json").is_file());
        assert!(!out.join("__MACOSX").exists());
    }

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    #[test]
    fn sniffs_formats_by_content() {
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join("mislabeled.7z");
        make_zip(&zip, &[("a", "b")]);
        assert_eq!(sniff(&zip).unwrap(), Format::Zip);
        assert_eq!(sniff(&fixture("mod.7z")).unwrap(), Format::SevenZip);
        assert_eq!(sniff(&fixture("solid.rar")).unwrap(), Format::Rar);
        let txt = dir.path().join("readme.zip");
        std::fs::write(&txt, "hello").unwrap();
        assert!(sniff(&txt).is_err());
    }

    #[test]
    fn extracts_7z_and_rejects_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out");
        extract(&fixture("mod.7z"), &out).unwrap();
        assert!(out.join("Mod7z/manifest.json").is_file());
        assert!(extract(&fixture("evil.7z"), &dir.path().join("evil")).is_err());
        assert!(!dir.path().join("x").exists());
    }

    #[test]
    fn extracts_rar() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out");
        extract(&fixture("solid.rar"), &out).unwrap();
        assert!(out.join(".gitignore").is_file());
    }

    #[test]
    fn judges_entry_paths() {
        assert_eq!(
            safe_relative(Path::new("./a/b")),
            Some(PathBuf::from("a/b"))
        );
        assert_eq!(safe_relative(Path::new("../a")), None);
        assert_eq!(safe_relative(Path::new("/etc/passwd")), None);
        assert_eq!(safe_relative(Path::new("")), None);
    }

    #[test]
    fn zips_folder_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("Farm_1");
        std::fs::create_dir_all(src.join("sub")).unwrap();
        std::fs::write(src.join("Farm_1"), "world").unwrap();
        std::fs::write(src.join("sub/x"), "y").unwrap();
        let zip = dir.path().join("b.zip");
        zip_dir(&src, &zip, "Farm_1").unwrap();
        let out = dir.path().join("out");
        extract(&zip, &out).unwrap();
        assert_eq!(
            std::fs::read_to_string(out.join("Farm_1/Farm_1")).unwrap(),
            "world"
        );
        assert_eq!(
            std::fs::read_to_string(out.join("Farm_1/sub/x")).unwrap(),
            "y"
        );
        assert!(!dir.path().join("b.zip.part").exists());
    }
}
