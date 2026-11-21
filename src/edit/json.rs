// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 The pixel-perfect Authors

//! Insertion into JSON worksheets.
//!
//! A scanner walks the root object just enough to find each collection's
//! array: the end of its last element (new items go after it) or, for an
//! empty array, its `[`. Missing collections go after the root's last member.
//! New items copy the indentation of the existing ones; a file written on a
//! single line gets compact items.

use super::{Addition, EditError, Insertion, Node, line_ending};

struct Scanner<'a> {
    text: &'a str,
    b: &'a [u8],
    i: usize,
}

/// What the scanner found about one top-level array.
struct ArrayInfo {
    key_start: usize,
    open: usize,
    first_elem: Option<usize>,
    last_elem_end: Option<usize>,
}

impl<'a> Scanner<'a> {
    fn err<T>(&self, what: &str) -> Result<T, EditError> {
        Err(EditError(format!(
            "unexpected JSON at byte {}: {what}",
            self.i
        )))
    }

    fn ws(&mut self) {
        while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }

    fn expect(&mut self, c: u8) -> Result<(), EditError> {
        if self.b.get(self.i) == Some(&c) {
            self.i += 1;
            Ok(())
        } else {
            self.err(&format!("expected {:?}", c as char))
        }
    }

    /// Skips a string, returning its span including the quotes.
    fn string(&mut self) -> Result<(usize, usize), EditError> {
        let start = self.i;
        self.expect(b'"')?;
        while self.i < self.b.len() {
            match self.b[self.i] {
                b'\\' => self.i += 2,
                b'"' => {
                    self.i += 1;
                    return Ok((start, self.i));
                }
                _ => self.i += 1,
            }
        }
        self.err("unterminated string")
    }

    /// Skips any value.
    fn value(&mut self) -> Result<(), EditError> {
        match self.b.get(self.i) {
            Some(b'"') => self.string().map(|_| ()),
            Some(b'{' | b'[') => {
                let mut depth = 0usize;
                while self.i < self.b.len() {
                    match self.b[self.i] {
                        b'"' => {
                            self.string()?;
                            continue;
                        }
                        b'{' | b'[' => depth += 1,
                        b'}' | b']' => {
                            depth -= 1;
                            if depth == 0 {
                                self.i += 1;
                                return Ok(());
                            }
                        }
                        _ => {}
                    }
                    self.i += 1;
                }
                self.err("unterminated value")
            }
            Some(_) => {
                while self.i < self.b.len()
                    && !matches!(
                        self.b[self.i],
                        b',' | b'}' | b']' | b' ' | b'\t' | b'\n' | b'\r'
                    )
                {
                    self.i += 1;
                }
                Ok(())
            }
            None => self.err("unexpected end"),
        }
    }

    /// Scans an array at the cursor, recording its elements.
    fn array(&mut self, key_start: usize) -> Result<ArrayInfo, EditError> {
        let open = self.i;
        self.expect(b'[')?;
        let mut info = ArrayInfo {
            key_start,
            open,
            first_elem: None,
            last_elem_end: None,
        };
        self.ws();
        if self.b.get(self.i) == Some(&b']') {
            self.i += 1;
            return Ok(info);
        }
        loop {
            self.ws();
            info.first_elem.get_or_insert(self.i);
            self.value()?;
            info.last_elem_end = Some(self.i);
            self.ws();
            match self.b.get(self.i) {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    return Ok(info);
                }
                _ => return self.err("expected ',' or ']'"),
            }
        }
    }

    /// Leading whitespace of the line holding `offset`.
    fn indent_of(&self, offset: usize) -> &'a str {
        let line_start = self.text[..offset].rfind('\n').map_or(0, |i| i + 1);
        let rest = &self.text[line_start..];
        let len = rest.len() - rest.trim_start_matches([' ', '\t']).len();
        &rest[..len]
    }

    fn same_line(&self, a: usize, b: usize) -> bool {
        !self.text[a.min(b)..a.max(b)].contains('\n')
    }
}

/// How new text is laid out.
struct Style<'a> {
    pretty: bool,
    unit: String,
    nl: &'a str,
}

impl Style<'_> {
    fn emit(&self, node: &Node, indent: &str) -> String {
        match node {
            Node::Str(s) | Node::Quoted(s) => serde_json::to_string(s).expect("string"),
            Node::Num(n) => n.clone(),
            Node::Map { entries, flow } => {
                let parts: Vec<(String, &Node)> = entries
                    .iter()
                    .map(|(k, v)| (serde_json::to_string(k).expect("key"), v))
                    .collect();
                if !self.pretty {
                    let inner: Vec<String> = parts
                        .iter()
                        .map(|(k, v)| format!("{k}:{}", self.emit(v, "")))
                        .collect();
                    format!("{{{}}}", inner.join(","))
                } else if *flow {
                    let inner: Vec<String> = parts
                        .iter()
                        .map(|(k, v)| format!("{k}: {}", self.emit(v, indent)))
                        .collect();
                    format!("{{ {} }}", inner.join(", "))
                } else {
                    let deeper = format!("{indent}{}", self.unit);
                    let inner: Vec<String> = parts
                        .iter()
                        .map(|(k, v)| format!("{deeper}{k}: {}", self.emit(v, &deeper)))
                        .collect();
                    format!(
                        "{{{nl}{}{nl}{indent}}}",
                        inner.join(&format!(",{}", self.nl)),
                        nl = self.nl
                    )
                }
            }
            Node::Seq { items, flow } => {
                if items.is_empty() {
                    "[]".into()
                } else if !self.pretty {
                    let inner: Vec<String> = items.iter().map(|v| self.emit(v, "")).collect();
                    format!("[{}]", inner.join(","))
                } else if *flow {
                    let inner: Vec<String> = items.iter().map(|v| self.emit(v, indent)).collect();
                    format!("[{}]", inner.join(", "))
                } else {
                    let deeper = format!("{indent}{}", self.unit);
                    let inner: Vec<String> = items
                        .iter()
                        .map(|v| format!("{deeper}{}", self.emit(v, &deeper)))
                        .collect();
                    format!(
                        "[{nl}{}{nl}{indent}]",
                        inner.join(&format!(",{}", self.nl)),
                        nl = self.nl
                    )
                }
            }
        }
    }

    /// Items separated for an array whose elements start at `indent`.
    fn items(&self, items: &[Node], indent: &str, inline: bool) -> String {
        let sep = if !self.pretty {
            ",".to_owned()
        } else if inline {
            ", ".to_owned()
        } else {
            format!(",{}{indent}", self.nl)
        };
        items
            .iter()
            .map(|n| self.emit(n, indent))
            .collect::<Vec<_>>()
            .join(&sep)
    }
}

/// Plans the insertions adding `additions` to the JSON worksheet `text`.
pub fn plan(text: &str, additions: &[Addition]) -> Result<Vec<Insertion>, EditError> {
    let mut s = Scanner {
        text,
        b: text.as_bytes(),
        i: 0,
    };
    s.ws();
    let root_open = s.i;
    s.expect(b'{')?;
    let mut arrays: Vec<(String, ArrayInfo)> = Vec::new();
    let mut first_member: Option<usize> = None;
    let mut last_member_end: Option<usize> = None;
    loop {
        s.ws();
        if s.b.get(s.i) == Some(&b'}') {
            break;
        }
        let (ks, ke) = s.string()?;
        first_member.get_or_insert(ks);
        let key: String = serde_json::from_str(&text[ks..ke])
            .map_err(|e| EditError(format!("bad key at byte {ks}: {e}")))?;
        s.ws();
        s.expect(b':')?;
        s.ws();
        if additions.iter().any(|a| a.collection == key) {
            if s.b.get(s.i) != Some(&b'[') {
                return s.err(&format!("{key:?} is not an array"));
            }
            let info = s.array(ks)?;
            arrays.push((key, info));
        } else {
            s.value()?;
        }
        last_member_end = Some(s.i);
        s.ws();
        match s.b.get(s.i) {
            Some(b',') => s.i += 1,
            Some(b'}') => break,
            _ => return s.err("expected ',' or '}'"),
        }
    }

    let nl = line_ending(text);
    let pretty = text[root_open..s.i].contains('\n');
    let root_indent = s.indent_of(root_open).to_owned();
    let unit = match first_member {
        Some(m) if pretty && !s.same_line(root_open, m) => s
            .indent_of(m)
            .strip_prefix(root_indent.as_str())
            .filter(|u| !u.is_empty())
            .unwrap_or("  ")
            .to_owned(),
        _ => "  ".to_owned(),
    };
    let style = Style { pretty, unit, nl };
    let member = format!("{root_indent}{}", style.unit);

    let mut out = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    for add in additions {
        match arrays.iter().find(|(k, _)| *k == add.collection) {
            Some((_, info)) => match (info.first_elem, info.last_elem_end) {
                (Some(first), Some(last_end)) => {
                    let inline = s.same_line(info.open, first);
                    let indent = if inline {
                        s.indent_of(info.key_start).to_owned()
                    } else {
                        s.indent_of(first).to_owned()
                    };
                    let sep = if !pretty {
                        ",".to_owned()
                    } else if inline {
                        ", ".to_owned()
                    } else {
                        format!(",{nl}{indent}")
                    };
                    out.push(Insertion {
                        offset: last_end,
                        text: format!("{sep}{}", style.items(&add.items, &indent, inline)),
                    });
                }
                _ => {
                    let key_indent = s.indent_of(info.key_start);
                    let text = if pretty {
                        let indent = format!("{key_indent}{}", style.unit);
                        format!(
                            "{nl}{indent}{}{nl}{key_indent}",
                            style.items(&add.items, &indent, false)
                        )
                    } else {
                        style.items(&add.items, "", false)
                    };
                    out.push(Insertion {
                        offset: info.open + 1,
                        text,
                    });
                }
            },
            None => {
                let key = serde_json::to_string(&add.collection).expect("key");
                missing.push(if pretty {
                    let indent = format!("{member}{}", style.unit);
                    format!(
                        "{nl}{member}{key}: [{nl}{indent}{}{nl}{member}]",
                        style.items(&add.items, &indent, false)
                    )
                } else {
                    format!("{key}:[{}]", style.items(&add.items, "", false))
                });
            }
        }
    }
    if !missing.is_empty() {
        // All missing collections go in one insertion after the last member.
        let lead = if last_member_end.is_some() { "," } else { "" };
        let tail = match (last_member_end, pretty) {
            (None, true) => format!("{nl}{root_indent}"),
            _ => String::new(),
        };
        out.push(Insertion {
            offset: last_member_end.unwrap_or(root_open + 1),
            text: format!("{lead}{}{tail}", missing.join(",")),
        });
    }
    Ok(out)
}
