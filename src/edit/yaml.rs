// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 The pixel-perfect Authors

//! Insertion into YAML worksheets.
//!
//! Supports the layout worksheets use: one block-style document whose
//! top-level keys start at column 0. A collection's block runs from its key
//! to the next top-level key; new items go after the block's last content
//! line, so comments and blank lines before the next key stay with it.
//! Missing collections are appended at the end of the file. Anything else
//! (flow collections, several documents, anchors, aliases or tags on a
//! collection, tab indentation) is refused rather than guessed.

use super::{Addition, EditError, Insertion, Node, line_ending};

/// One line of the source, without its line ending.
struct Line<'a> {
    number: usize,
    content: &'a str,
    /// Byte offset just after the line ending (or the end of the text).
    next: usize,
    has_newline: bool,
}

impl Line<'_> {
    fn indent(&self) -> usize {
        self.content.len() - self.content.trim_start_matches(' ').len()
    }

    fn is_blank(&self) -> bool {
        self.content.trim().is_empty()
    }

    fn is_comment(&self) -> bool {
        self.content.trim_start().starts_with('#')
    }
}

fn lines(text: &str) -> Vec<Line<'_>> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut number = 1;
    while start < text.len() {
        let (end, next, has_newline) = match text[start..].find('\n') {
            Some(i) => (start + i, start + i + 1, true),
            None => (text.len(), text.len(), false),
        };
        let content = text[start..end]
            .strip_suffix('\r')
            .unwrap_or(&text[start..end]);
        out.push(Line {
            number,
            content,
            next,
            has_newline,
        });
        start = next;
        number += 1;
    }
    out
}

/// A top-level key and the rest of its line after the colon.
struct Key<'a> {
    name: String,
    line: usize,
    rest: &'a str,
}

fn refuse<T>(line: usize, what: &str) -> Result<T, EditError> {
    Err(EditError(format!(
        "line {line}: {what}; the importer cannot add to it without rewriting the file"
    )))
}

/// Parses a column-0 `key: rest` line.
fn parse_key<'a>(line: &Line<'a>) -> Option<(String, &'a str)> {
    let c = line.content;
    let (name, after) = if let Some(q) = c.strip_prefix('"') {
        let end = q.find('"')?;
        (q[..end].to_owned(), &q[end + 1..])
    } else if let Some(q) = c.strip_prefix('\'') {
        let end = q.find('\'')?;
        (q[..end].to_owned(), &q[end + 1..])
    } else {
        let colon = c
            .char_indices()
            .find(|&(i, ch)| ch == ':' && matches!(c[i + 1..].chars().next(), None | Some(' ')))?
            .0;
        (c[..colon].trim_end().to_owned(), &c[colon..])
    };
    let rest = after.trim_start().strip_prefix(':')?;
    Some((name, rest))
}

/// Strips a trailing comment from the value part of a line.
fn value_part(rest: &str) -> &str {
    let rest = rest.trim();
    if rest.starts_with('#') {
        return "";
    }
    match rest.find(" #") {
        Some(i) => rest[..i].trim_end(),
        None => rest,
    }
}

/// Plans the insertions adding `additions` to the YAML worksheet `text`.
pub fn plan(text: &str, additions: &[Addition]) -> Result<Vec<Insertion>, EditError> {
    let lines = lines(text);
    let nl = line_ending(text);
    let mut keys: Vec<Key> = Vec::new();
    let mut seen_content = false;
    for (idx, line) in lines.iter().enumerate() {
        if line.is_blank() || line.is_comment() {
            continue;
        }
        let c = line.content;
        if c.starts_with('\t') {
            return refuse(line.number, "tab indentation");
        }
        if c.starts_with("---") && matches!(c[3..].chars().next(), None | Some(' ')) {
            if seen_content || !value_part(&c[3..]).is_empty() {
                return refuse(line.number, "several YAML documents or content after `---`");
            }
            continue;
        }
        if c.starts_with("...") {
            return refuse(line.number, "YAML document end marker `...`");
        }
        seen_content = true;
        if line.indent() > 0 {
            continue;
        }
        if c == "-" || c.starts_with("- ") {
            if keys.is_empty() {
                return refuse(line.number, "the document is not a mapping");
            }
            continue; // a non-indented sequence item of the current key
        }
        if c.starts_with(['{', '[']) {
            return refuse(line.number, "flow-style document");
        }
        if c.starts_with(['?', '&', '*', '!', '%']) {
            return refuse(line.number, "unsupported top-level construct");
        }
        match parse_key(line) {
            Some((name, rest)) => keys.push(Key {
                name,
                line: idx,
                rest,
            }),
            None => return refuse(line.number, "unsupported top-level line"),
        }
    }

    let mut out = Vec::new();
    let mut appended = String::new();
    for add in additions {
        let Some(k) = keys.iter().position(|k| k.name == add.collection) else {
            appended.push_str(&format!("{}:{nl}", add.collection));
            for item in &add.items {
                appended.push_str(&item_text(item, 2, nl));
            }
            continue;
        };
        let key = &keys[k];
        let key_line = &lines[key.line];
        let value = value_part(key.rest);
        if value.starts_with('[') {
            out.push(flow_append(text, key_line, &key.name, value, &add.items)?);
            continue;
        }
        match value.chars().next() {
            None => {}
            Some('{') => {
                return refuse(
                    key_line.number,
                    &format!("`{}` is a flow collection", key.name),
                );
            }
            Some('&') => return refuse(key_line.number, &format!("`{}` has an anchor", key.name)),
            Some('*') => return refuse(key_line.number, &format!("`{}` is an alias", key.name)),
            Some('!') => return refuse(key_line.number, &format!("`{}` has a tag", key.name)),
            Some(_) => {
                return refuse(
                    key_line.number,
                    &format!("`{}` is not a block sequence", key.name),
                );
            }
        }
        let block_end = keys.get(k + 1).map_or(lines.len(), |next| next.line);
        let block = &lines[key.line + 1..block_end];
        let Some(first) = block.iter().find(|l| !l.is_blank() && !l.is_comment()) else {
            return refuse(key_line.number, &format!("`{}` has no items", key.name));
        };
        let item_indent = first.indent();
        let first_item = first.content.trim_start();
        if !(first_item == "-" || first_item.starts_with("- ")) {
            return refuse(
                first.number,
                &format!("`{}` is not a block sequence", key.name),
            );
        }
        let last = block
            .iter()
            .rev()
            .find(|l| !l.is_blank() && (!l.is_comment() || l.indent() > item_indent))
            .expect("the block has a first item");
        let mut text = String::new();
        if !last.has_newline {
            text.push_str(nl);
        }
        for item in &add.items {
            text.push_str(&item_text(item, item_indent, nl));
        }
        out.push(Insertion {
            offset: last.next,
            text,
        });
    }
    if !appended.is_empty() {
        let mut text = String::new();
        if let Some(last) = lines.last() {
            if !last.has_newline {
                text.push_str(nl);
            }
            if !last.is_blank() {
                text.push_str(nl);
            }
        }
        text.push_str(&appended);
        out.push(Insertion {
            offset: text_len(&lines),
            text,
        });
    }
    Ok(out)
}

/// Appends scalar items to a flow sequence written on one line
/// (`exports: ["/a", "/b"]`), just after its last item. Flow sequences of
/// mappings or spanning several lines are refused.
fn flow_append(
    text: &str,
    line: &Line,
    name: &str,
    value: &str,
    items: &[Node],
) -> Result<Insertion, EditError> {
    let flow = || refuse(line.number, &format!("`{name}` is a flow collection"));
    let inner = match value.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
        Some(inner) => inner,
        None => return flow(),
    };
    let nested = inner.chars().any(|c| matches!(c, '[' | ']' | '{' | '}'));
    let scalars = items
        .iter()
        .all(|n| matches!(n, Node::Str(_) | Node::Quoted(_) | Node::Num(_)));
    if nested || !scalars {
        return flow();
    }
    let trimmed = inner.trim_end();
    // `value` borrows from `text`, so its position is a pointer difference.
    let offset = trimmed.as_ptr() as usize - text.as_ptr() as usize + trimmed.len();
    let lead = if trimmed.trim().is_empty() { "" } else { ", " };
    let added: Vec<String> = items.iter().map(inline).collect();
    Ok(Insertion {
        offset,
        text: format!("{lead}{}", added.join(", ")),
    })
}

fn text_len(lines: &[Line]) -> usize {
    lines.last().map_or(0, |l| l.next)
}

/// A sequence item `- ...` at `indent`, ending with a line ending.
fn item_text(node: &Node, indent: usize, nl: &str) -> String {
    let pad = " ".repeat(indent);
    match node {
        Node::Map {
            entries,
            flow: false,
        } if !entries.is_empty() => {
            let mut out = String::new();
            for (i, (key, value)) in entries.iter().enumerate() {
                let lead = if i == 0 {
                    format!("{pad}- ")
                } else {
                    format!("{pad}  ")
                };
                out.push_str(&entry_text(&lead, key, value, indent + 2, nl));
            }
            out
        }
        Node::Seq { flow: false, .. } => {
            // Not needed by worksheets; written in flow style.
            format!("{pad}- {}{nl}", inline(node))
        }
        _ => format!("{pad}- {}{nl}", inline(node)),
    }
}

/// `lead key: value` for a mapping entry whose keys sit at `key_indent`.
fn entry_text(lead: &str, key: &str, value: &Node, key_indent: usize, nl: &str) -> String {
    let key = scalar(key, false);
    match value {
        Node::Seq { items, flow: false } if !items.is_empty() => {
            let mut out = format!("{lead}{key}:{nl}");
            for item in items {
                out.push_str(&item_text(item, key_indent + 2, nl));
            }
            out
        }
        Node::Map {
            entries,
            flow: false,
        } if !entries.is_empty() => {
            let mut out = format!("{lead}{key}:{nl}");
            let pad = " ".repeat(key_indent + 2);
            for (k, v) in entries {
                out.push_str(&entry_text(&pad, k, v, key_indent + 2, nl));
            }
            out
        }
        _ => format!("{lead}{key}: {}{nl}", inline(value)),
    }
}

/// A node on one line, in flow style.
fn inline(node: &Node) -> String {
    match node {
        Node::Str(s) => scalar(s, false),
        Node::Quoted(s) => scalar(s, true),
        Node::Num(n) => n.clone(),
        Node::Map { entries, .. } => {
            let inner: Vec<String> = entries
                .iter()
                .map(|(k, v)| format!("{}: {}", scalar(k, false), inline(v)))
                .collect();
            format!("{{ {} }}", inner.join(", "))
        }
        Node::Seq { items, .. } => {
            let inner: Vec<String> = items.iter().map(inline).collect();
            format!("[{}]", inner.join(", "))
        }
    }
}

/// Words YAML 1.1 reads as booleans or null when unquoted.
const RESERVED: &[&str] = &["y", "n", "yes", "no", "on", "off", "true", "false", "null"];

/// A string scalar: plain when it is unambiguously a string, double-quoted
/// (with escapes) otherwise or when `force_quotes`.
fn scalar(s: &str, force_quotes: bool) -> String {
    let plain = !force_quotes
        && s.chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        && !RESERVED.contains(&s.to_ascii_lowercase().as_str());
    if plain {
        return s.to_owned();
    }
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c if needs_escape(c) => {
                let cp = c as u32;
                if cp <= 0xFFFF {
                    out.push_str(&format!("\\u{cp:04X}"));
                } else {
                    out.push_str(&format!("\\U{cp:08X}"));
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Characters YAML does not allow literally (outside its printable set) or
/// that some parsers treat as line breaks or byte order marks.
fn needs_escape(c: char) -> bool {
    let cp = c as u32;
    cp < 0x20
        || (0x7F..=0x9F).contains(&cp)
        || matches!(cp, 0x2028 | 0x2029 | 0xFEFF | 0xFFFE | 0xFFFF)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalars() {
        assert_eq!(scalar("hero", false), "hero");
        assert_eq!(scalar("slime-ice", false), "slime-ice");
        assert_eq!(scalar("#", false), "\"#\"");
        assert_eq!(scalar("y", false), "\"y\"");
        assert_eq!(scalar("N", false), "\"N\"");
        assert_eq!(scalar("1", false), "\"1\"");
        assert_eq!(scalar("-", false), "\"-\"");
        assert_eq!(scalar("a\"b\\", false), "\"a\\\"b\\\\\"");
        assert_eq!(scalar("\u{FFFE}", false), "\"\\uFFFE\"");
        assert_eq!(scalar("aab", true), "\"aab\"");
    }
}
