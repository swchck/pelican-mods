//! Rosetta 2 detection and install.
//!
//! The macOS builds of Stardew Valley and SMAPI ship x86_64-only binaries, so on Apple
//! silicon nothing runs until Rosetta is present. macOS installs it on demand only when
//! an Intel app is opened from Finder; a child process just fails with EBADARCH.

use std::path::Path;

use crate::error::{AppError, AppResult};

/// errno for "Bad CPU type in executable".
pub const EBADARCH: i32 = 86;

const RUNTIME: &str = "/Library/Apple/usr/share/rosetta/rosetta";

const CPU_X86_64: u32 = 0x0100_0007;
const CPU_ARM64: u32 = 0x0100_000c;

/// Reports whether running `exe` on this machine needs Rosetta that is not installed.
pub fn missing_for(exe: &Path) -> bool {
    cfg!(target_arch = "aarch64") && !Path::new(RUNTIME).exists() && intel_only(exe)
}

/// Maps a spawn failure caused by a missing Rosetta to its own error.
pub fn classify(e: std::io::Error) -> AppError {
    if e.raw_os_error() == Some(EBADARCH) {
        AppError::RosettaMissing
    } else {
        e.into()
    }
}

/// Installs Rosetta through `softwareupdate`, behind the system administrator prompt.
pub async fn install() -> AppResult<()> {
    // the user's click in the app is the license acceptance; macOS asks for the password itself
    let script = r#"do shell script "/usr/sbin/softwareupdate --install-rosetta --agree-to-license" with administrator privileges"#;
    let out = tokio::process::Command::new("/usr/bin/osascript")
        .args(["-e", script])
        .output()
        .await?;
    if out.status.success() {
        return Ok(());
    }
    let err = String::from_utf8_lossy(&out.stderr);
    // osascript reports a dismissed password prompt as error -128
    if err.contains("-128") {
        return Err(AppError::Cancelled);
    }
    Err(AppError::InstallerFailed(err.trim().to_owned()))
}

fn intel_only(exe: &Path) -> bool {
    let Ok(bytes) = read_head(exe) else {
        return false;
    };
    let cpus = cpu_types(&bytes);
    cpus.contains(&CPU_X86_64) && !cpus.contains(&CPU_ARM64)
}

fn read_head(path: &Path) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    let mut buf = vec![0u8; 4096];
    let n = std::fs::File::open(path)?.read(&mut buf)?;
    buf.truncate(n);
    Ok(buf)
}

// Mach-O header layouts: <mach-o/loader.h> for thin files, <mach-o/fat.h> for universal ones
fn cpu_types(b: &[u8]) -> Vec<u32> {
    let be = |i: usize| {
        b.get(i..i + 4)
            .map(|s| u32::from_be_bytes(s.try_into().unwrap()))
    };
    let le = |i: usize| {
        b.get(i..i + 4)
            .map(|s| u32::from_le_bytes(s.try_into().unwrap()))
    };
    match be(0) {
        Some(0xcffa_edfe) => le(4).into_iter().collect(),
        Some(magic @ (0xcafe_babe | 0xcafe_babf)) => {
            let stride = if magic == 0xcafe_babe { 20 } else { 32 };
            let n = be(4).unwrap_or(0).min(16) as usize;
            (0..n).filter_map(|i| be(8 + i * stride)).collect()
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_thin_intel_binary() {
        let mut b = vec![0xcf, 0xfa, 0xed, 0xfe];
        b.extend(CPU_X86_64.to_le_bytes());
        assert_eq!(cpu_types(&b), [CPU_X86_64]);
    }

    #[test]
    fn reads_universal_binary() {
        let mut b = vec![0xca, 0xfe, 0xba, 0xbe, 0, 0, 0, 2];
        for cpu in [CPU_X86_64, CPU_ARM64] {
            b.extend(cpu.to_be_bytes());
            b.extend([0u8; 16]);
        }
        assert_eq!(cpu_types(&b), [CPU_X86_64, CPU_ARM64]);
    }

    #[test]
    fn ignores_non_macho() {
        assert!(cpu_types(b"#!/bin/sh\n").is_empty());
        assert!(cpu_types(&[]).is_empty());
    }

    #[test]
    fn classifies_bad_cpu_type() {
        let e = std::io::Error::from_raw_os_error(EBADARCH);
        assert!(matches!(classify(e), AppError::RosettaMissing));
        let e = std::io::Error::from_raw_os_error(2);
        assert!(matches!(classify(e), AppError::Io(_)));
    }
}
