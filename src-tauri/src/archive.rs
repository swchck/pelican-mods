//! Unpacking downloaded archives without letting them write outside the target folder.

use std::io::Read;
use std::path::Path;

use crate::error::{AppError, AppResult};

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
}
