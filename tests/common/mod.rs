// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 The pixel-perfect Authors

//! Helpers shared by the integration tests.
#![allow(dead_code)]

use std::path::PathBuf;

use pixel_perfect::{Diagnostic, LoadError, Location, ResolvedWorksheet};
use serde_json::Value;

pub fn fixture(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(relative)
}

/// `schema/manifest.schema.json` as a JSON value.
pub fn manifest_schema() -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("schema/manifest.schema.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// Fails, listing every violation, unless `manifest` is valid against
/// `schema/manifest.schema.json`.
pub fn assert_valid_manifest(manifest: &Value) {
    static VALIDATOR: std::sync::OnceLock<jsonschema::Validator> = std::sync::OnceLock::new();
    let validator = VALIDATOR.get_or_init(|| {
        jsonschema::draft202012::new(&manifest_schema()).expect("manifest schema compiles")
    });
    let errors: Vec<String> = validator
        .iter_errors(manifest)
        .map(|e| format!("{}: {e}", e.instance_path()))
        .collect();
    assert!(
        errors.is_empty(),
        "manifest is not valid against the schema:\n{}\n{manifest:#}",
        errors.join("\n")
    );
}

/// The `valid/full.json` fixture as a JSON value, ready to be mutated.
pub fn full() -> Value {
    let text = std::fs::read_to_string(fixture("valid/full.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

/// The `valid/descriptions.yaml` fixture as a JSON value, ready to be mutated.
pub fn descriptions() -> Value {
    let text = std::fs::read_to_string(fixture("valid/descriptions.yaml")).unwrap();
    pixel_perfect::input::parse(&text, pixel_perfect::Format::Yaml).unwrap()
}

/// Removes every `description` property, at any depth.
pub fn strip_descriptions(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.remove("description");
            map.values_mut().for_each(strip_descriptions);
        }
        Value::Array(items) => items.iter_mut().for_each(strip_descriptions),
        _ => {}
    }
}

/// Sets the value at `pointer`, creating the last segment if needed
/// (an object key, or the next index of an array).
pub fn set(doc: &mut Value, pointer: &str, value: Value) {
    let (parent, last) = pointer.rsplit_once('/').expect("pointer has a parent");
    let parent = doc
        .pointer_mut(parent)
        .unwrap_or_else(|| panic!("no parent for {pointer}"));
    match parent {
        Value::Object(map) => {
            map.insert(last.to_owned(), value);
        }
        Value::Array(items) => {
            let i: usize = last.parse().unwrap();
            if i == items.len() {
                items.push(value);
            } else {
                items[i] = value;
            }
        }
        _ => panic!("parent of {pointer} is not a container"),
    }
}

/// Removes the value at `pointer`.
pub fn remove(doc: &mut Value, pointer: &str) {
    let (parent, last) = pointer.rsplit_once('/').unwrap();
    match doc.pointer_mut(parent).unwrap() {
        Value::Object(map) => {
            map.remove(last)
                .unwrap_or_else(|| panic!("nothing at {pointer}"));
        }
        Value::Array(items) => {
            items.remove(last.parse().unwrap());
        }
        _ => panic!("parent of {pointer} is not a container"),
    }
}

pub fn load(doc: Value) -> ResolvedWorksheet {
    pixel_perfect::load_value(doc)
        .unwrap_or_else(|e| panic!("expected a valid worksheet, got {:#?}", e.diagnostics()))
}

/// Loads `doc`, expecting it to be invalid, and returns the diagnostics.
pub fn errors(doc: Value) -> Vec<Diagnostic> {
    match pixel_perfect::load_value(doc) {
        Ok(_) => panic!("expected the worksheet to be invalid"),
        Err(LoadError::Invalid { diagnostics }) => diagnostics,
        Err(e) => panic!("unexpected error kind: {e}"),
    }
}

fn pointer_of(d: &Diagnostic) -> &str {
    match &d.location {
        Location::Pointer(p) => p,
        _ => "",
    }
}

/// Asserts that some diagnostic points exactly at `pointer` and mentions `text`.
pub fn assert_error(diagnostics: &[Diagnostic], pointer: &str, text: &str) {
    assert!(
        diagnostics
            .iter()
            .any(|d| pointer_of(d) == pointer && d.message.contains(text)),
        "no diagnostic at {pointer} containing {text:?}; got:\n{}",
        render(diagnostics)
    );
}

/// Asserts that some diagnostic points at `prefix` or inside it.
pub fn assert_error_within(diagnostics: &[Diagnostic], prefix: &str) {
    assert!(
        diagnostics.iter().any(|d| {
            let p = pointer_of(d);
            p == prefix || p.starts_with(&format!("{prefix}/"))
        }),
        "no diagnostic at or below {prefix}; got:\n{}",
        render(diagnostics)
    );
}

pub fn render(diagnostics: &[Diagnostic]) -> String {
    diagnostics.iter().map(|d| format!("  {d}\n")).collect()
}
