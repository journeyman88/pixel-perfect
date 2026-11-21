// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 The pixel-perfect Authors

//! Format-preserving additions to worksheet text.
//!
//! The editors never re-serialize a worksheet: they find where new items go
//! and insert text there, so comments, key order and formatting survive.
//! JSON and YAML use small scanners ([`json`], [`yaml`]); TOML uses
//! `toml_edit` ([`toml`]). Callers must verify the result (see
//! `crate::import`), because the scanners only understand what they need.

pub mod json;
pub mod toml;
pub mod yaml;

use serde_json::Value;

use crate::input::Format;

/// A value to insert, with enough style to write it naturally.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    /// A mapping; `flow` writes it on one line (`{ a: 1 }`).
    Map {
        entries: Vec<(String, Node)>,
        flow: bool,
    },
    /// A sequence; `flow` writes it on one line (`[1, 2]`).
    Seq { items: Vec<Node>, flow: bool },
    /// A string, quoted only where the format requires it.
    Str(String),
    /// A string that is always quoted (sprite lines, pointers).
    Quoted(String),
    /// A number, as the literal to write.
    Num(String),
}

impl Node {
    pub fn map(entries: Vec<(String, Node)>) -> Node {
        Node::Map {
            entries,
            flow: false,
        }
    }

    pub fn flow_map(entries: Vec<(String, Node)>) -> Node {
        Node::Map {
            entries,
            flow: true,
        }
    }

    pub fn seq(items: Vec<Node>) -> Node {
        Node::Seq { items, flow: false }
    }

    pub fn flow_seq(items: Vec<Node>) -> Node {
        Node::Seq { items, flow: true }
    }

    /// The value in the JSON data model.
    pub fn to_json(&self) -> Value {
        match self {
            Node::Map { entries, .. } => Value::Object(
                entries
                    .iter()
                    .map(|(k, v)| (k.clone(), v.to_json()))
                    .collect(),
            ),
            Node::Seq { items, .. } => Value::Array(items.iter().map(Node::to_json).collect()),
            Node::Str(s) | Node::Quoted(s) => Value::String(s.clone()),
            Node::Num(n) => serde_json::from_str(n).expect("numeric literal"),
        }
    }
}

/// Items to append to one top-level collection (`palettes`, `sprites`,
/// `exports`), which is created at the end of the worksheet if missing.
#[derive(Debug, Clone, PartialEq)]
pub struct Addition {
    pub collection: String,
    pub items: Vec<Node>,
}

/// Text to insert at a byte offset of the original.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Insertion {
    pub offset: usize,
    pub text: String,
}

/// Why the worksheet cannot be extended by insertion alone.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct EditError(pub String);

/// The line ending of `text`: CRLF if it has any, LF otherwise.
pub fn line_ending(text: &str) -> &'static str {
    if text.contains("\r\n") { "\r\n" } else { "\n" }
}

/// Applies insertions to `text`. Insertions at the same offset keep their
/// order.
pub fn apply(text: &str, mut insertions: Vec<Insertion>) -> String {
    insertions.sort_by_key(|i| i.offset);
    let mut out =
        String::with_capacity(text.len() + insertions.iter().map(|i| i.text.len()).sum::<usize>());
    let mut at = 0;
    for ins in insertions {
        out.push_str(&text[at..ins.offset]);
        out.push_str(&ins.text);
        at = ins.offset;
    }
    out.push_str(&text[at..]);
    out
}

/// Returns `text` with `additions` appended, in the worksheet's format.
pub fn add(text: &str, format: Format, additions: &[Addition]) -> Result<String, EditError> {
    match format {
        Format::Json => Ok(apply(text, json::plan(text, additions)?)),
        Format::Yaml => Ok(apply(text, yaml::plan(text, additions)?)),
        Format::Toml => toml::add(text, additions),
    }
}

/// Whether `old` appears in `new` in order: every byte of `old` is kept and
/// only insertions happened.
pub fn is_subsequence(old: &str, new: &str) -> bool {
    let mut rest = new.bytes();
    old.bytes().all(|b| rest.any(|n| n == b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_keeps_order_at_equal_offsets() {
        let ins = vec![
            Insertion {
                offset: 3,
                text: "Y".into(),
            },
            Insertion {
                offset: 0,
                text: "<".into(),
            },
            Insertion {
                offset: 3,
                text: "Z".into(),
            },
        ];
        assert_eq!(apply("abc", ins), "<abcYZ");
    }

    #[test]
    fn subsequence() {
        assert!(is_subsequence("abc", "xaybzc"));
        assert!(!is_subsequence("abc", "acb"));
    }
}
