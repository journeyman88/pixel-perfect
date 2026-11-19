// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 The pixel-perfect Authors

//! Multi-format input (tasks 2.5, 2.6).

mod common;

use std::path::Path;

use common::fixture;
use pixel_perfect::input::{self, Format};
use pixel_perfect::{LoadError, Location};

fn read(relative: &str) -> String {
    std::fs::read_to_string(fixture(relative)).unwrap()
}

#[test]
fn format_is_chosen_by_extension_case_insensitively() {
    assert_eq!(Format::from_path(Path::new("a.json")), Ok(Format::Json));
    assert_eq!(Format::from_path(Path::new("a.JSON")), Ok(Format::Json));
    assert_eq!(Format::from_path(Path::new("a.yaml")), Ok(Format::Yaml));
    assert_eq!(Format::from_path(Path::new("a.Yml")), Ok(Format::Yaml));
    assert_eq!(Format::from_path(Path::new("a.toml")), Ok(Format::Toml));
    assert_eq!(Format::from_path(Path::new("a.txt")), Err("txt".to_owned()));
    assert_eq!(Format::from_path(Path::new("noext")), Err(String::new()));
}

#[test]
fn unknown_extension_is_rejected_before_parsing() {
    let err = pixel_perfect::load_file(&fixture("invalid/unsupported.txt")).unwrap_err();
    assert!(matches!(err, LoadError::UnsupportedExtension { ref ext, .. } if ext == "txt"));
    let message = err.to_string();
    assert!(
        message.contains(".json") && message.contains(".toml"),
        "{message}"
    );
}

#[test]
fn same_worksheet_in_three_formats_is_the_same_data() {
    let json = input::parse(&read("valid/readme-example.json"), Format::Json).unwrap();
    let yaml = input::parse(&read("valid/readme-example.yaml"), Format::Yaml).unwrap();
    let toml = input::parse(&read("valid/readme-example.toml"), Format::Toml).unwrap();
    assert_eq!(json, yaml);
    assert_eq!(json, toml);

    let models: Vec<_> = ["json", "yaml", "toml"]
        .iter()
        .map(|ext| {
            pixel_perfect::load_file(&fixture(&format!("valid/readme-example.{ext}"))).unwrap()
        })
        .collect();
    assert_eq!(models[0], models[1]);
    assert_eq!(models[0], models[2]);
}

#[test]
fn yaml_integer_keys_become_strings() {
    let ws = pixel_perfect::load_file(&fixture("valid/override-int-keys.yaml")).unwrap();
    let derived = &ws.sprites["derived"];
    let rows: Vec<String> = derived.rows.iter().map(|r| r.iter().collect()).collect();
    assert_eq!(rows, ["rr", "gr", "rr"]);
}

#[test]
fn yaml_non_scalar_key_is_rejected() {
    let errors = input::parse("palettes:\n  ? [1, 2]\n  : x\n", Format::Yaml).unwrap_err();
    assert_eq!(errors[0].location, Location::Pointer("/palettes".into()));
}

#[test]
fn yaml_tag_is_rejected() {
    let errors = input::parse("palettes: !custom value\n", Format::Yaml).unwrap_err();
    assert_eq!(errors[0].location, Location::Pointer("/palettes".into()));
    assert!(errors[0].message.contains("tag"));
}

#[test]
fn toml_date_time_is_rejected_with_its_location() {
    let err = pixel_perfect::load_file(&fixture("invalid/datetime.toml")).unwrap_err();
    let d = &err.diagnostics()[0];
    assert_eq!(d.location, Location::Pointer("/palettes/0/created".into()));
    assert!(d.message.contains("date-time"), "{d}");
}

#[test]
fn malformed_json_reports_its_line() {
    let err = pixel_perfect::load_file(&fixture("invalid/malformed.json")).unwrap_err();
    let d = &err.diagnostics()[0];
    assert!(
        matches!(d.location, Location::LineColumn { line: 12, .. }),
        "{d}"
    );
}

#[test]
fn malformed_yaml_and_toml_report_their_line() {
    let yaml = input::parse("palettes:\n  - id: p\n   bad: [\n", Format::Yaml).unwrap_err();
    assert!(
        matches!(yaml[0].location, Location::LineColumn { .. }),
        "{}",
        yaml[0]
    );
    let toml = input::parse("palettes = 1\nbroken = \n", Format::Toml).unwrap_err();
    assert!(
        matches!(toml[0].location, Location::LineColumn { line: 2, .. }),
        "{}",
        toml[0]
    );
}

#[test]
fn missing_file_is_an_io_error() {
    let err = pixel_perfect::load_file(&fixture("valid/does-not-exist.json")).unwrap_err();
    assert!(matches!(err, LoadError::Io { .. }));
}
