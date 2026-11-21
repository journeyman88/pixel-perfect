// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 The pixel-perfect Authors

//! The import alphabet: every character the schema accepts as a palette
//! `char`, readable single-column characters first.
//!
//! The ranges live in the generated [`table`] module; see
//! `tools/gen-alphabet/README.md`.

mod table;

pub use table::UNICODE_VERSION;

fn count(ranges: &[(u32, u32)]) -> usize {
    ranges.iter().map(|(a, b)| (b - a + 1) as usize).sum()
}

/// Number of characters in tier 1 (single-column, visible).
pub fn tier1_len() -> usize {
    count(table::TIER1)
}

/// Number of characters in the whole alphabet.
pub fn len() -> usize {
    tier1_len() + count(table::TIER2)
}

/// The `i`-th character of the alphabet, or `None` past its end.
pub fn nth(mut i: usize) -> Option<char> {
    for &(start, end) in table::TIER1.iter().chain(table::TIER2) {
        let size = (end - start + 1) as usize;
        if i < size {
            return char::from_u32(start + i as u32);
        }
        i -= size;
    }
    None
}

/// Iterates over the alphabet in order.
pub fn chars() -> impl Iterator<Item = char> {
    table::TIER1.iter().chain(table::TIER2).flat_map(|&(a, b)| {
        (a..=b).map(|cp| char::from_u32(cp).expect("table holds scalar values"))
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    const ASCII_PREFIX: &str = concat!(
        "abcdefghijklmnopqrstuvwxyz",
        "ABCDEFGHIJKLMNOPQRSTUVWXYZ",
        "0123456789",
        "!#$%&()*+,-/:;<>?@[]^_{|}~",
        "\"'\\`",
    );

    /// The `char` pattern of the embedded schema.
    fn schema_pattern() -> String {
        let schema: serde_json::Value =
            serde_json::from_str(crate::schema::WORKSHEET_SCHEMA).unwrap();
        schema["$defs"]["color"]["properties"]["char"]["pattern"]
            .as_str()
            .unwrap()
            .to_owned()
    }

    #[test]
    fn starts_with_the_ascii_prefix() {
        let start: String = chars().take(92).collect();
        assert_eq!(start, ASCII_PREFIX);
        assert_eq!(nth(0), Some('a'));
        assert_eq!(nth(91), Some('`'));
    }

    #[test]
    fn equals_the_set_accepted_by_the_schema_pattern() {
        // Same engine (`regex-syntax` tables) as `jsonschema` uses.
        let re = regex::Regex::new(&schema_pattern()).unwrap();
        let alphabet: Vec<char> = chars().collect();
        let set: HashSet<char> = alphabet.iter().copied().collect();
        assert_eq!(set.len(), alphabet.len(), "duplicates in the alphabet");
        assert_eq!(alphabet.len(), len());
        let mut buf = [0u8; 4];
        let mut accepted = 0;
        for c in (0..=0x10FFFF).filter_map(char::from_u32) {
            let ok = re.is_match(c.encode_utf8(&mut buf));
            assert_eq!(ok, set.contains(&c), "U+{:04X}", c as u32);
            accepted += usize::from(ok);
        }
        assert_eq!(accepted, len());
    }

    #[test]
    fn tiers_separate_readable_characters() {
        let tier1: HashSet<char> = chars().take(tier1_len()).collect();
        for c in ['é', 'Ж', '█', 'α', '─', '¡'] {
            assert!(tier1.contains(&c), "{c:?} should be in tier 1");
        }
        // Wide, right-to-left, private use, unassigned, noncharacter.
        for c in ['山', 'א', 'ب', '\u{E000}', '\u{378}', '\u{FFFE}', '😀'] {
            assert!(!tier1.contains(&c), "{c:?} should be in tier 2");
            assert!(chars().skip(tier1_len()).any(|x| x == c), "{c:?} missing");
        }
    }

    #[test]
    fn nth_agrees_with_iteration() {
        let mut n = 0;
        for (i, c) in chars().enumerate() {
            // Checking every index would be quadratic; sample densely.
            if i % 997 == 0 || i < 200 || i + 5 > len() || (i + 2).is_power_of_two() {
                assert_eq!(nth(i), Some(c), "index {i}");
            }
            n += 1;
        }
        assert_eq!(n, len());
        assert_eq!(nth(len()), None);
    }
}
