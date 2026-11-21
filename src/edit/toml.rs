// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 The pixel-perfect Authors

//! Additions to TOML worksheets, through `toml_edit`.
//!
//! `toml_edit` keeps the original text of everything it does not touch.
//! Collections of tables are extended with new `[[...]]` tables, inline
//! arrays with new values. Multi-line arrays are written one item per line,
//! like the hand-written examples.

use toml_edit::{Array, ArrayOfTables, DocumentMut, InlineTable, Item, Table, Value};

use super::{Addition, EditError, Node, line_ending};

/// Returns `text` with `additions` appended.
pub fn add(text: &str, additions: &[Addition]) -> Result<String, EditError> {
    // toml_edit writes LF; a CRLF file is edited as LF and converted back,
    // which is lossless only when the file has no lone LF.
    let crlf = line_ending(text) == "\r\n";
    if crlf && text.replace("\r\n", "").contains('\n') {
        return Err(EditError("the file mixes CRLF and LF line endings".into()));
    }
    let source = if crlf {
        text.replace("\r\n", "\n")
    } else {
        text.to_owned()
    };
    let mut doc: DocumentMut = source
        .parse()
        .map_err(|e| EditError(format!("cannot parse TOML: {e}")))?;
    for add in additions {
        let key = add.collection.as_str();
        match doc.get_mut(key) {
            None => {
                if add.items.iter().all(|n| matches!(n, Node::Map { .. })) {
                    let mut aot = ArrayOfTables::new();
                    for item in &add.items {
                        aot.push(table(item));
                    }
                    doc.insert(key, Item::ArrayOfTables(aot));
                } else {
                    doc.insert(key, Item::Value(Value::Array(array(&add.items, false))));
                }
            }
            Some(Item::ArrayOfTables(aot)) => {
                for item in &add.items {
                    aot.push(table(item));
                }
            }
            Some(Item::Value(Value::Array(arr))) => {
                let multiline = arr.iter().any(|v| {
                    v.decor()
                        .prefix()
                        .and_then(|p| p.as_str())
                        .is_some_and(|p| p.contains('\n'))
                });
                for item in &add.items {
                    let v = value(item);
                    if multiline {
                        arr.push_formatted(v.decorated("\n  ", ""));
                    } else {
                        arr.push(v);
                    }
                }
            }
            Some(_) => {
                return Err(EditError(format!(
                    "`{key}` is neither an array of tables nor an array"
                )));
            }
        }
    }
    let out = doc.to_string();
    Ok(if crlf { out.replace('\n', "\r\n") } else { out })
}

fn table(node: &Node) -> Table {
    let Node::Map { entries, .. } = node else {
        unreachable!("only mappings become tables")
    };
    let mut t = Table::new();
    for (k, v) in entries {
        t.insert(k, Item::Value(value(v)));
    }
    t
}

/// An array; `multiline` puts each item on its own line.
fn array(items: &[Node], multiline: bool) -> Array {
    let mut arr = Array::new();
    for item in items {
        if multiline {
            arr.push_formatted(value(item).decorated("\n  ", ""));
        } else {
            arr.push(value(item));
        }
    }
    if multiline {
        arr.set_trailing_comma(true);
        arr.set_trailing("\n");
    }
    arr
}

fn value(node: &Node) -> Value {
    match node {
        Node::Str(s) | Node::Quoted(s) => Value::from(s.as_str()),
        Node::Num(n) => match n.parse::<i64>() {
            Ok(i) => Value::from(i),
            Err(_) => Value::from(n.parse::<f64>().expect("numeric literal")),
        },
        Node::Seq { items, flow } => Value::Array(array(items, !flow)),
        Node::Map { entries, .. } => {
            let mut t = InlineTable::new();
            for (k, v) in entries {
                t.insert(k, value(v));
            }
            t.fmt();
            Value::InlineTable(t)
        }
    }
}
