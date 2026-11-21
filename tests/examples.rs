// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 The pixel-perfect Authors

//! The worksheets in `examples/` stay valid and render with the encodings
//! their comments promise.

#[path = "common/images.rs"]
mod images;

use std::path::PathBuf;

use images::header;
use pixel_perfect::export::{self, OutputFormat, RenderOptions};

fn example(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join(name)
}

#[test]
fn sprites_example_covers_every_encoding() {
    let ws = pixel_perfect::load_file(&example("sprites.yaml")).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let options = RenderOptions {
        output_path: dir.path().to_owned(),
        format: OutputFormat::Png,
        scale: 1,
        safe_build: false,
        max_image_size: Some(export::DEFAULT_MAX_IMAGE_SIZE),
    };
    let (plan, _) = export::render(&ws, &options).unwrap();
    assert!(plan.notices.is_empty());

    // (sprite, width, height, bit depth, color type: 3 indexed, 6 RGBA)
    let expected = [
        ("coin", 8, 8, 1, 3),
        ("heart", 7, 6, 2, 3),
        ("slime", 12, 9, 4, 3),
        ("slime-ice", 12, 9, 4, 3),
        ("sunset", 24, 24, 8, 3),
        ("spectrum", 16, 18, 8, 6),
    ];
    assert_eq!(plan.outputs.len(), expected.len());
    for (id, width, height, depth, color_type) in expected {
        let png = std::fs::read(dir.path().join(format!("sprites/{id}.png"))).unwrap();
        assert_eq!(header(&png), (width, height, depth, color_type), "{id}");
    }
}

#[test]
fn composition_example_renders_its_scene() {
    let ws = pixel_perfect::load_file(&example("composition.yaml")).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let options = RenderOptions {
        output_path: dir.path().to_owned(),
        format: OutputFormat::Png,
        scale: 1,
        safe_build: false,
        max_image_size: Some(export::DEFAULT_MAX_IMAGE_SIZE),
    };
    let (plan, _) = export::render(&ws, &options).unwrap();
    assert!(plan.notices.is_empty());
    assert_eq!(plan.outputs.len(), 8 + 3 + 1, "sprites, panels, scene");

    // (file, bit depth): every composition stays indexed.
    for (file, depth) in [
        ("panels/mist.png", 1),
        ("panels/actors.png", 4),
        ("panels/backdrop.png", 4),
        ("scenes/dusk.png", 8),
    ] {
        let png = std::fs::read(dir.path().join(file)).unwrap();
        assert_eq!(header(&png), (32, 20, depth, 3), "{file}");
    }
}

#[test]
fn masquerade_example_renders_its_scene() {
    let ws = pixel_perfect::load_file(&example("masquerade.yaml")).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let options = RenderOptions {
        output_path: dir.path().to_owned(),
        format: OutputFormat::Png,
        scale: 1,
        safe_build: false,
        max_image_size: Some(export::DEFAULT_MAX_IMAGE_SIZE),
    };
    let (plan, _) = export::render(&ws, &options).unwrap();
    assert!(plan.notices.is_empty());
    assert_eq!(plan.outputs.len(), 1, "only the scene is exported");
    let png = std::fs::read(dir.path().join("scenes/masquerade.png")).unwrap();
    assert_eq!(header(&png), (256, 256, 8, 3));
}

#[test]
fn campfire_example_renders_its_animations() {
    let ws = pixel_perfect::load_file(&example("campfire.yaml")).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let options = RenderOptions {
        output_path: dir.path().to_owned(),
        format: OutputFormat::Png,
        scale: 1,
        safe_build: false,
        max_image_size: Some(export::DEFAULT_MAX_IMAGE_SIZE),
    };
    let (plan, manifest) = export::render(&ws, &options).unwrap();
    assert!(plan.notices.is_empty());
    assert_eq!(plan.outputs.len(), 3);
    // (pointer, width, height, loop in seconds as num/den)
    for (pointer, width, height, seconds) in [
        ("/flipbooks/flames", 56, 74, (1, 1)),
        ("/timelines/fire", 256, 256, (2, 1)),
        ("/scenes/campfire", 256, 256, (2, 1)),
    ] {
        let entry = &manifest.files[pointer];
        assert_eq!((entry.width, entry.height), (width, height), "{pointer}");
        let fraction = entry.loop_seconds.expect("animated");
        assert_eq!((fraction.num, fraction.den), seconds, "{pointer}");
        let png = std::fs::read(dir.path().join(&entry.path)).unwrap();
        assert!(
            images::chunks(&png).iter().any(|c| c == "acTL"),
            "{pointer} is an APNG"
        );
    }
}

#[test]
fn animation_example_renders_its_animations() {
    let ws = pixel_perfect::load_file(&example("animation.yaml")).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let options = RenderOptions {
        output_path: dir.path().to_owned(),
        format: OutputFormat::Png,
        scale: 1,
        safe_build: false,
        max_image_size: Some(export::DEFAULT_MAX_IMAGE_SIZE),
    };
    let (plan, manifest) = export::render(&ws, &options).unwrap();
    assert!(plan.notices.is_empty());
    assert_eq!(plan.outputs.len(), 4);
    // (pointer, width, height, loop in seconds as num/den)
    for (pointer, width, height, seconds) in [
        ("/flipbooks/walk", 8, 8, (1, 2)),
        ("/panels/torch-post", 3, 12, (2, 5)),
        ("/timelines/stroll", 48, 16, (8, 1)),
        ("/scenes/evening", 48, 16, (8, 1)),
    ] {
        let entry = &manifest.files[pointer];
        assert_eq!((entry.width, entry.height), (width, height), "{pointer}");
        let fraction = entry.loop_seconds.expect("animated");
        assert_eq!((fraction.num, fraction.den), seconds, "{pointer}");
        let png = std::fs::read(dir.path().join(&entry.path)).unwrap();
        assert!(
            images::chunks(&png).iter().any(|c| c == "acTL"),
            "{pointer} is an APNG"
        );
    }
}

#[test]
fn slime_png_imports_back_into_the_sprites_example() {
    // examples/slime.png is `slime` rendered with --scale 4.
    let ws = pixel_perfect::load_file(&example("sprites.yaml")).unwrap();
    let mut options = pixel_perfect::import::ImportOptions::new(
        example("sprites.yaml"),
        example("slime.png"),
        "slime-copy",
    );
    options.palette = Some("slime".into());
    options.scale = 4;
    options.dry_run = true;
    let imported = pixel_perfect::import::run(&options).unwrap();
    let new = pixel_perfect::load_str(&imported.text, pixel_perfect::Format::Yaml).unwrap();
    assert_eq!(new.sprites["slime-copy"].rows, ws.sprites["slime"].rows);
}
