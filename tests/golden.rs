// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 The pixel-perfect Authors

//! Golden files: rendering fixtures with `exports` removed (so every resource
//! is exported) must reproduce the committed files byte for byte.
//!
//! Regenerate after an intended change with:
//! `UPDATE_GOLDEN=1 cargo test --test golden`

mod common;
#[path = "common/images.rs"]
mod images;

use std::path::{Path, PathBuf};

use pixel_perfect::export::{self, OutputFormat, RenderOptions};

fn render_to(
    fixture: &str,
    dir: &Path,
    format: OutputFormat,
    scale: u32,
    safe_build: bool,
    single_frames: bool,
) {
    let path = common::fixture(fixture);
    let text = std::fs::read_to_string(&path).unwrap();
    let input = pixel_perfect::Format::from_path(&path).unwrap();
    let mut doc = pixel_perfect::input::parse(&text, input).unwrap();
    doc.as_object_mut().unwrap().remove("exports");
    let ws = common::load(doc);
    let options = RenderOptions {
        output_path: dir.to_owned(),
        format,
        scale,
        safe_build,
        single_frames,
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
    check_options(
        name,
        fixture,
        format,
        scale,
        safe_build,
        false,
        expected_files,
    );
}

fn check_options(
    name: &str,
    fixture: &str,
    format: OutputFormat,
    scale: u32,
    safe_build: bool,
    single_frames: bool,
    expected_files: usize,
) {
    let golden = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(name);
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        let _ = std::fs::remove_dir_all(&golden);
        render_to(fixture, &golden, format, scale, safe_build, single_frames);
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    render_to(
        fixture,
        dir.path(),
        format,
        scale,
        safe_build,
        single_frames,
    );
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
fn sequence_fixture() {
    // manifest + 5 sprites + 5 panels + timeline + animated scene + sequence
    check("sequence", "valid/sequence.yaml", 1, false, 14);
}

#[test]
fn sequence_fixture_webp() {
    check_format(
        "sequence-webp",
        "valid/sequence.yaml",
        OutputFormat::Webp,
        1,
        false,
        14,
    );
}

// Single frames: animations become one still image per frame.

/// manifest + 6 sprites + 2 static panels + the frames of flipbook `sparkle`,
/// the 2 timelines and scene `show` (44 frames).
const FRAMES_FILE_COUNT: usize = 53;

#[test]
fn animation_fixture_single_frames() {
    check_options(
        "animation-frames",
        "valid/animation.json",
        OutputFormat::Png,
        1,
        false,
        true,
        FRAMES_FILE_COUNT,
    );
}

#[test]
fn animation_fixture_single_frames_webp() {
    check_options(
        "animation-frames-webp",
        "valid/animation.json",
        OutputFormat::Webp,
        1,
        false,
        true,
        FRAMES_FILE_COUNT,
    );
}

#[test]
fn sequence_renders_are_byte_identical() {
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    for dir in [&a, &b] {
        render_to(
            "valid/sequence.yaml",
            dir.path(),
            OutputFormat::Png,
            2,
            false,
            false,
        );
    }
    for relative in tree(a.path()) {
        assert_eq!(
            std::fs::read(a.path().join(&relative)).unwrap(),
            std::fs::read(b.path().join(&relative)).unwrap(),
            "{}",
            relative.display()
        );
    }
}

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
    for name in [
        "full-webp",
        "scene-webp",
        "animation-webp",
        "sequence-webp",
        "animation-frames-webp",
    ] {
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
    // 40 in the four animated-container directories, 52 frames and stills in
    // animation-frames-webp.
    assert_eq!(checked, 40 + 52);
}

/// Every golden manifest is valid against `schema/manifest.schema.json`.
#[test]
fn golden_manifests_match_the_schema() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let mut checked = 0;
    for entry in std::fs::read_dir(&root).unwrap() {
        let path = entry.unwrap().path().join("manifest.json");
        let text = std::fs::read_to_string(&path).unwrap();
        common::assert_valid_manifest(&serde_json::from_str(&text).unwrap());
        checked += 1;
    }
    assert_eq!(checked, 11, "one manifest per golden directory");
}

/// Every golden APNG, decoded by compositing its frames as the APNG
/// specification defines, gives exactly the rendered frames of its resource.
#[test]
fn golden_apngs_decode_to_the_rendered_frames() {
    use pixel_perfect::animate::Animation;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let mut checked = 0;
    for (name, fixture) in [
        ("full", "valid/full.json"),
        ("full-x4-safe", "valid/full.json"),
        ("animation", "valid/animation.json"),
        ("sequence", "valid/sequence.yaml"),
    ] {
        let ws = pixel_perfect::load_file(&common::fixture(fixture)).unwrap();
        let manifest: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(name).join("manifest.json")).unwrap(),
        )
        .unwrap();
        let scale = manifest["options"]["scale"].as_u64().unwrap() as u32;
        for entry in manifest["files"].as_array().unwrap() {
            if entry["kind"] != "animation" {
                continue;
            }
            let pointer = entry["id"].as_str().unwrap();
            let (collection, id) = pointer[1..].split_once('/').unwrap();
            let bytes =
                std::fs::read(root.join(name).join(entry["path"].as_str().unwrap())).unwrap();
            let (decoded, _) = images::decode_apng(&bytes);
            let animation = Animation::new(&ws, collection, id).unwrap();
            let expected = animation.frames();
            assert_eq!(decoded.len(), expected.len(), "{name}{pointer}");
            for (i, (frame, (image, _))) in decoded.iter().zip(&expected).enumerate() {
                assert!(
                    frame.pixels == image.upscale(scale).pixels(),
                    "{name}{pointer}: frame {i}"
                );
            }
            checked += 1;
        }
    }
    // full: flipbook, timeline, scene (twice, at two scales); animation:
    // flipbook, 2 timelines, scene (its panels are static); sequence:
    // timeline, scene, sequence.
    assert_eq!(checked, 13);
}
