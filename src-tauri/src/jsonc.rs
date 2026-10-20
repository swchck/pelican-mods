//! JSON5 from mods and the internet, read without trusting its shape.
//!
//! The `json5` crate recurses once per nesting level with no limit, so a file of a few
//! hundred kilobytes of `[[[[…` overflows the stack and aborts the whole app; no
//! `catch_unwind` stops that. Everything JSON5 a mod or a download hands us goes through
//! [`parse`], which refuses such files before the parser sees them.

use serde::de::DeserializeOwned;

/// Larger than any real manifest, config or content file by two orders of magnitude.
pub const MAX_LEN: usize = 16 << 20;
/// serde_json's own default; real content packs stay under 20.
const MAX_DEPTH: usize = 128;

/// Parses JSON5 `text` (a leading BOM is fine), refusing text over [`MAX_LEN`] bytes or
/// nested deeper than serde_json would allow.
pub fn parse<T: DeserializeOwned>(text: &str) -> Result<T, String> {
    let text = text.trim_start_matches('\u{feff}');
    if text.len() > MAX_LEN {
        return Err(format!("larger than {} MB", MAX_LEN >> 20));
    }
    if depth(text) > MAX_DEPTH {
        return Err(format!("nested deeper than {MAX_DEPTH} levels"));
    }
    json5::from_str(text).map_err(|e| e.to_string())
}

/// Reads the regular file at `path` for [`parse`], refusing anything else (a device, a
/// FIFO) and anything over [`MAX_LEN`] before reading it into memory.
pub fn read(path: &std::path::Path) -> std::io::Result<String> {
    let meta = std::fs::metadata(path)?;
    if !meta.is_file() || meta.len() > MAX_LEN as u64 {
        return Err(std::io::Error::other("not a regular file of sensible size"));
    }
    std::fs::read_to_string(path)
}

// deepest bracket nesting outside strings and comments; malformed text just reads as
// shallower or deeper, and the parser then reports what's actually wrong
fn depth(text: &str) -> usize {
    let b = text.as_bytes();
    let (mut i, mut level, mut deepest) = (0, 0usize, 0);
    while i < b.len() {
        match b[i] {
            q @ (b'"' | b'\'') => {
                i += 1;
                while i < b.len() && b[i] != q {
                    i += if b[i] == b'\\' { 2 } else { 1 };
                }
            }
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                    i += 1;
                }
                i += 1;
            }
            b'[' | b'{' => {
                level += 1;
                deepest = deepest.max(level);
            }
            b']' | b'}' => level = level.saturating_sub(1),
            _ => {}
        }
        i += 1;
    }
    deepest
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn reads_json5_and_refuses_deep_nesting() {
        let v: Value = parse("\u{feff}{ a: [1, 2,], // c\n b: '[[[', /* {{{ */ }").unwrap();
        assert_eq!(v["b"], "[[[");
        assert_eq!(depth(r#"{"a": "]}}\"[[", "b": [[1]]}"#), 3);
        // would abort the process inside json5 without the check
        let bomb = "[".repeat(200_000) + &"]".repeat(200_000);
        assert!(parse::<Value>(&bomb).unwrap_err().contains("nested"));
        let ok = "[".repeat(MAX_DEPTH) + &"]".repeat(MAX_DEPTH);
        assert!(parse::<Value>(&ok).is_ok());
    }
}
