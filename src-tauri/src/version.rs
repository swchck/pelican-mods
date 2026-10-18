//! Version ordering as SMAPI applies it to mods: dotted numbers, then an optional
//! prerelease tag that sorts before the release.

use std::cmp::Ordering;

/// Compares two version strings; malformed parts count as zero.
pub fn compare(a: &str, b: &str) -> Ordering {
    let (core_a, pre_a) = split(a);
    let (core_b, pre_b) = split(b);
    let len = core_a.len().max(core_b.len());
    for i in 0..len {
        let ord = core_a.get(i).unwrap_or(&0).cmp(core_b.get(i).unwrap_or(&0));
        if ord != Ordering::Equal {
            return ord;
        }
    }
    match (pre_a.is_empty(), pre_b.is_empty()) {
        (true, true) => Ordering::Equal,
        (true, false) => Ordering::Greater,
        (false, true) => Ordering::Less,
        (false, false) => pre_a.cmp(pre_b),
    }
}

/// Reports whether `have` satisfies a `need` minimum.
pub fn at_least(have: &str, need: &str) -> bool {
    compare(have, need) != Ordering::Less
}

fn split(v: &str) -> (Vec<u64>, &str) {
    let v = v.trim();
    let v = v.strip_prefix(['v', 'V']).unwrap_or(v);
    let v = v.split('+').next().unwrap_or(v);
    let (core, pre) = v.split_once('-').unwrap_or((v, ""));
    let nums = core
        .split('.')
        .map(|n| n.trim().parse().unwrap_or(0))
        .collect();
    (nums, pre)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orders_versions() {
        assert_eq!(compare("4.10.0", "4.9.1"), Ordering::Greater);
        assert_eq!(compare("1.6", "1.6.0"), Ordering::Equal);
        assert_eq!(compare("v2.0.0+abc", "2.0.0"), Ordering::Equal);
        assert_eq!(compare("1.0.0-beta.2", "1.0.0"), Ordering::Less);
        assert!(at_least("1.6.15.24356", "1.6.0"));
        assert!(!at_least("4.3.2", "4.5.0"));
    }
}
