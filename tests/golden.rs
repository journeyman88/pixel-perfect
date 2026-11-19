// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 The pixel-perfect Authors

//! Golden files: rendering fixtures with `exports` removed (so every resource
//! is exported) must reproduce the committed files byte for byte.
//!
//! Regenerate after an intended change with:
//! `UPDATE_GOLDEN=1 cargo test --test golden`

mod common;

use std::path::{Path, PathBuf};

use pixel_perfect::export::{self, OutputFormat, RenderOptions};

fn render_to(fixture: &str, dir: &Path, scale: u32, safe_build: bool) {
    let text = std::fs::read_to_string(common::fixture(fixture)).unwrap();
    let mut doc: serde_json::Value = serde_json::from_str(&text).unwrap();
    doc.as_object_mut().unwrap().remove("exports");
    let ws = common::load(doc);
    let options = RenderOptions {
        output_path: dir.to_owned(),
        format: OutputFormat::Png,
        scale,
        safe_build,
        max_image_size: Some(export::DEFAULT_MAX_IMAGE_SIZE),
    };
    export::render(&ws, &options).unwrap();
}

/// Files under `dir`, relative, sorted.
fn tree(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_owned()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.push(path.strip_prefix(dir).unwrap().to_owned());
            }
        }
    }
    out.sort();
    out
}

fn check(name: &str, fixture: &str, scale: u32, safe_build: bool, expected_files: usize) {
    let golden = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(name);
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        let _ = std::fs::remove_dir_all(&golden);
        render_to(fixture, &golden, scale, safe_build);
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    render_to(fixture, dir.path(), scale, safe_build);
    let expected = tree(&golden);
    assert_eq!(tree(dir.path()), expected, "produced files");
    for relative in &expected {
        assert!(
            std::fs::read(dir.path().join(relative)).unwrap()
                == std::fs::read(golden.join(relative)).unwrap(),
            "{} differs from golden",
            relative.display()
        );
    }
    assert_eq!(expected.len(), expected_files, "{name}");
}

#[test]
fn full_fixture() {
    // manifest + 3 sprites + panel `stage` + flipbook `spin` + timeline `walk`
    // + animated scene `night`
    check("full", "valid/full.json", 1, false, 8);
}

#[test]
fn full_fixture_scaled_safe_build() {
    check("full-x4-safe", "valid/full.json", 4, true, 8);
}

#[test]
fn scene_fixture() {
    // manifest + 5 sprites + 2 panels + 1 scene
    check("scene", "valid/scene.json", 1, false, 9);
}

#[test]
fn animation_fixture() {
    // manifest + 6 sprites + flipbook + 2 static panels + 2 timelines + scene
    check("animation", "valid/animation.json", 1, false, 13);
}
