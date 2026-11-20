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

fn render_to(fixture: &str, dir: &Path, format: OutputFormat, scale: u32, safe_build: bool) {
    let text = std::fs::read_to_string(common::fixture(fixture)).unwrap();
    let mut doc: serde_json::Value = serde_json::from_str(&text).unwrap();
    doc.as_object_mut().unwrap().remove("exports");
    let ws = common::load(doc);
    let options = RenderOptions {
        output_path: dir.to_owned(),
        format,
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
    check_format(
        name,
        fixture,
        OutputFormat::Png,
        scale,
        safe_build,
        expected_files,
    );
}

fn check_format(
    name: &str,
    fixture: &str,
    format: OutputFormat,
    scale: u32,
    safe_build: bool,
    expected_files: usize,
) {
    let golden = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(name);
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        let _ = std::fs::remove_dir_all(&golden);
        render_to(fixture, &golden, format, scale, safe_build);
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    render_to(fixture, dir.path(), format, scale, safe_build);
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

// WebP: same fixtures, same file counts.

#[test]
fn full_fixture_webp() {
    check_format(
        "full-webp",
        "valid/full.json",
        OutputFormat::Webp,
        1,
        false,
        8,
    );
}

#[test]
fn scene_fixture_webp() {
    check_format(
        "scene-webp",
        "valid/scene.json",
        OutputFormat::Webp,
        1,
        false,
        9,
    );
}

#[test]
fn animation_fixture_webp() {
    check_format(
        "animation-webp",
        "valid/animation.json",
        OutputFormat::Webp,
        1,
        false,
        13,
    );
}

/// libwebp's `webpinfo` accepts every golden WebP file. Skipped when
/// `webpinfo` is not installed: the other tests decode with `image-webp`,
/// which is more lenient than libwebp.
#[test]
fn webp_goldens_pass_webpinfo() {
    if std::process::Command::new("webpinfo")
        .arg("-version")
        .output()
        .is_err()
    {
        eprintln!("webpinfo not installed; skipping");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let mut checked = 0;
    for name in ["full-webp", "scene-webp", "animation-webp"] {
        for relative in tree(&root.join(name)) {
            if relative.extension().is_none_or(|e| e != "webp") {
                continue;
            }
            let path = root.join(name).join(&relative);
            let output = std::process::Command::new("webpinfo")
                .arg("-quiet")
                .arg(&path)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}: {}",
                path.display(),
                String::from_utf8_lossy(&output.stdout)
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 27);
}
