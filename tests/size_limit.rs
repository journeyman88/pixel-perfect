// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 The pixel-perfect Authors

//! `--max-image-size`: parsing and the plan-time size walk. No test allocates
//! a large image: the plan fails before rendering.

mod common;

use std::path::Path;

use common::{assert_error, load};
use pixel_perfect::export::{
    self, DEFAULT_MAX_IMAGE_SIZE, OutputFormat, RenderOptions, parse_size,
};
use serde_json::{Value, json};

#[test]
fn parse_accepts_bytes_binary_units_and_none() {
    assert_eq!(parse_size("1048576"), Ok(Some(1 << 20)));
    assert_eq!(parse_size("512MiB"), Ok(Some(512 << 20)));
    assert_eq!(parse_size("2gib"), Ok(Some(2 << 30)));
    assert_eq!(parse_size("1GiB"), Ok(Some(DEFAULT_MAX_IMAGE_SIZE)));
    assert_eq!(parse_size("3KIB"), Ok(Some(3 << 10)));
    assert_eq!(parse_size("1TiB"), Ok(Some(1 << 40)));
    assert_eq!(parse_size("none"), Ok(None));
    assert_eq!(parse_size("NONE"), Ok(None));
}

#[test]
fn parse_rejects_invalid_sizes() {
    for text in [
        "0",
        "0GiB",
        "1.5GiB",
        "big",
        "-1",
        "",
        "GiB",
        "1 GiB",
        "99999999999TiB",
    ] {
        assert!(parse_size(text).is_err(), "{text:?} should be rejected");
    }
    let decimal = parse_size("1GB").unwrap_err();
    assert!(decimal.contains("binary units"), "{decimal}");
}

#[test]
fn human_sizes() {
    assert_eq!(export::human_size(1 << 30), "1 GiB");
    assert_eq!(export::human_size(20000 * 20000 * 4), "1.49 GiB");
    assert_eq!(export::human_size(512), "512 B");
}

fn options(scale: u32, max_image_size: Option<u64>) -> RenderOptions {
    RenderOptions {
        output_path: Path::new("out").to_owned(),
        format: OutputFormat::Png,
        scale,
        safe_build: false,
        max_image_size,
    }
}

/// One-pixel sprite `a` plus the given panels and exports.
fn sheet(panels: Value, exports: Value) -> pixel_perfect::ResolvedWorksheet {
    load(json!({
        "palettes": [{ "id": "p", "colors": [{ "char": "r", "rgba": [255, 0, 0] }] }],
        "sprites": [{ "id": "a", "_palette": "p", "lines": ["r"] }],
        "panels": panels,
        "exports": exports
    }))
}

fn panel(width: u32, height: u32) -> Value {
    json!({ "id": "big", "width": width, "height": height,
            "elements": [{ "_sprite": "a", "coordinates": [0, 0] }] })
}

const DEFAULT: Option<u64> = Some(DEFAULT_MAX_IMAGE_SIZE);

#[test]
fn default_limit_exceeded() {
    let ws = sheet(json!([panel(20000, 20000)]), json!(["/panels/big"]));
    let errors = export::plan(&ws, &options(1, DEFAULT)).unwrap_err();
    assert_error(
        &errors,
        "/panels/0",
        "20000x20000 (1.49 GiB), over the 1 GiB image size limit",
    );
    assert_error(&errors, "/panels/0", "--max-image-size");
    assert_eq!(errors.len(), 1);
}

#[test]
fn limit_applies_after_scale() {
    let ws = sheet(json!([panel(4096, 4096)]), json!(["/panels/big"]));
    assert!(export::plan(&ws, &options(1, DEFAULT)).is_ok());
    let errors = export::plan(&ws, &options(8, DEFAULT)).unwrap_err();
    assert_error(
        &errors,
        "/panels/0",
        "the image after --scale 8 is 32768x32768 (4 GiB)",
    );
}

#[test]
fn intermediate_image() {
    let small = json!({ "id": "small", "width": 16, "height": 16, "elements": [
        { "_sprite": "a", "coordinates": [0, 0],
          "transforms": [{ "kind": "scale", "width": 30000, "height": 30000 }] }
    ] });
    let ws = sheet(json!([small]), json!(["/panels/small"]));
    let errors = export::plan(&ws, &options(1, DEFAULT)).unwrap_err();
    assert_error(
        &errors,
        "/panels/0/elements/0/transforms/0",
        "the result of this transform is 30000x30000",
    );
    assert_eq!(errors.len(), 1, "the 16x16 panel itself is fine");
}

#[test]
fn intermediate_image_after_tile() {
    let small = json!({ "id": "small", "width": 16, "height": 16, "elements": [
        { "_sprite": "a", "coordinates": [0, 0],
          "transforms": [{ "kind": "tile", "width": 30000, "height": 30000 }] }
    ] });
    let ws = sheet(json!([small]), json!(["/panels/small"]));
    let errors = export::plan(&ws, &options(1, DEFAULT)).unwrap_err();
    assert_error(
        &errors,
        "/panels/0/elements/0/transforms/0",
        "the result of this transform is 30000x30000",
    );
    assert_eq!(errors.len(), 1, "the 16x16 panel itself is fine");
}

#[test]
fn hidden_elements_are_not_checked() {
    let small = json!({ "id": "small", "width": 16, "height": 16, "elements": [
        { "_sprite": "a", "coordinates": [0, 0], "visible": false,
          "transforms": [{ "kind": "scale", "width": 30000, "height": 30000 }] }
    ] });
    let ws = sheet(json!([small]), json!(["/panels/small"]));
    assert!(export::plan(&ws, &options(1, DEFAULT)).is_ok());
}

#[test]
fn scene_checks_its_panels_and_canvas() {
    let ws = load(json!({
        "palettes": [{ "id": "p", "colors": [{ "char": "r", "rgba": [255, 0, 0] }] }],
        "sprites": [{ "id": "a", "_palette": "p", "lines": ["r"] }],
        "panels": [panel(20000, 20000)],
        "scenes": [
            { "id": "s", "width": 30000, "height": 30000, "layers": [
                { "_panel": "big", "coordinates": [0, 0] },
                { "_panel": "big", "coordinates": [5, 5] }
            ] }
        ],
        "exports": ["/scenes/s", "/panels/big"]
    }));
    let errors = export::plan(&ws, &options(1, DEFAULT)).unwrap_err();
    assert_error(&errors, "/scenes/0", "scene \"s\" is 30000x30000");
    assert_error(&errors, "/panels/0", "panel \"big\" is 20000x20000");
    assert_eq!(
        errors.len(),
        2,
        "each pointer is reported once: {errors:#?}"
    );
}

#[test]
fn raised_limit() {
    let ws = sheet(json!([panel(20000, 20000)]), json!(["/panels/big"]));
    assert!(export::plan(&ws, &options(1, Some(2 << 30))).is_ok());
}

#[test]
fn no_limit() {
    let ws = sheet(json!([panel(20000, 20000)]), json!(["/panels/big"]));
    assert!(export::plan(&ws, &options(64, None)).is_ok());
}

#[test]
fn limit_applies_to_sprites() {
    let ws = load(json!({
        "palettes": [{ "id": "p", "colors": [{ "char": "r", "rgba": [255, 0, 0] }] }],
        "sprites": [{ "id": "a", "_palette": "p", "lines": ["r"] }]
    }));
    let errors = export::plan(&ws, &options(1, Some(3))).unwrap_err();
    assert_error(
        &errors,
        "/sprites/0",
        "sprite \"a\" is 1x1 (4 B), over the 3 B image size limit",
    );
}

#[test]
fn nothing_is_written_when_the_limit_is_exceeded() {
    let ws = sheet(json!([panel(20000, 20000)]), json!(["/panels/big"]));
    let dir = tempfile::tempdir().unwrap();
    let mut opts = options(1, DEFAULT);
    opts.output_path = dir.path().join("out");
    assert!(export::render(&ws, &opts).is_err());
    assert!(!opts.output_path.exists());
}

// Animations

/// Panel `stage` (`width` x `height`) with element `hero`, and timeline `tl`.
fn timeline_sheet(
    width: u32,
    height: u32,
    fps: u32,
    duration: u64,
    transitions: Value,
) -> pixel_perfect::ResolvedWorksheet {
    load(json!({
        "palettes": [{ "id": "p", "colors": [{ "char": "r", "rgba": [255, 0, 0] }] }],
        "sprites": [{ "id": "a", "_palette": "p", "lines": ["r"] }],
        "panels": [{ "id": "stage", "width": width, "height": height,
            "elements": [{ "id": "hero", "_sprite": "a", "coordinates": [0, 0] }] }],
        "timelines": [{ "id": "tl", "_panel": "stage", "fps": fps, "duration": duration, "transitions": transitions }],
        "exports": ["/timelines/tl"]
    }))
}

#[test]
fn appended_transform() {
    let ws = timeline_sheet(
        16,
        16,
        10,
        10,
        json!([{ "kind": "appendTransforms", "_element": "hero",
        "transforms": [{ "kind": "scale", "width": 30000, "height": 30000 }], "at": 5 }]),
    );
    let errors = export::plan(&ws, &options(1, DEFAULT)).unwrap_err();
    assert_error(
        &errors,
        "/timelines/0/transitions/0/transforms/0",
        "the result of this transform is 30000x30000",
    );
}

#[test]
fn appended_tile() {
    let ws = timeline_sheet(
        16,
        16,
        10,
        10,
        json!([{ "kind": "appendTransforms", "_element": "hero",
        "transforms": [{ "kind": "tile", "width": 30000, "height": 30000 }], "at": 5 }]),
    );
    let errors = export::plan(&ws, &options(1, DEFAULT)).unwrap_err();
    assert_error(
        &errors,
        "/timelines/0/transitions/0/transforms/0",
        "the result of this transform is 30000x30000",
    );
}

#[test]
fn hidden_element_of_a_timeline_is_checked() {
    // It may become visible during the timeline.
    let ws = load(json!({
        "palettes": [{ "id": "p", "colors": [{ "char": "r", "rgba": [255, 0, 0] }] }],
        "sprites": [{ "id": "a", "_palette": "p", "lines": ["r"] }],
        "panels": [{ "id": "stage", "width": 16, "height": 16, "elements": [
            { "id": "hero", "_sprite": "a", "coordinates": [0, 0], "visible": false,
              "transforms": [{ "kind": "scale", "width": 30000, "height": 30000 }] }
        ] }],
        "timelines": [{ "id": "tl", "_panel": "stage", "fps": 10, "duration": 10,
            "transitions": [{ "kind": "setVisible", "_element": "hero", "value": true, "at": 5 }] }],
        "exports": ["/timelines/tl"]
    }));
    let errors = export::plan(&ws, &options(1, DEFAULT)).unwrap_err();
    assert_error(&errors, "/panels/0/elements/0/transforms/0", "30000x30000");
}

/// 512 x 512 (1 MiB per frame) moving every frame-unit: 2000 frames.
fn long_animation() -> pixel_perfect::ResolvedWorksheet {
    timeline_sheet(
        512,
        512,
        100,
        2000,
        json!([{ "kind": "moveTo", "_element": "hero",
        "coordinates": [511, 511], "start": 0, "duration": 1999 }]),
    )
}

#[test]
fn long_animation_exceeds_the_limit() {
    let errors = export::plan(&long_animation(), &options(1, DEFAULT)).unwrap_err();
    assert_error(&errors, "/timelines/0", "more than 1024 frames of 512x512");
    assert_error(
        &errors,
        "/timelines/0",
        "1 GiB image size limit; raise it with --max-image-size",
    );
}

#[test]
fn long_animation_fits_a_raised_limit() {
    assert!(export::plan(&long_animation(), &options(1, Some(2 << 30))).is_ok());
}

#[test]
fn no_limit_for_animations() {
    assert!(export::plan(&long_animation(), &options(1, None)).is_ok());
}

#[test]
fn animation_limit_applies_after_scale() {
    // 1000 frames of 32x32 scaled by 16 (1 MiB each) exceed 512 MiB.
    let ws = timeline_sheet(
        32,
        32,
        100,
        1000,
        json!([{ "kind": "moveTo", "_element": "hero",
        "coordinates": [31, 31], "start": 0, "duration": 999 }]),
    );
    assert!(export::plan(&ws, &options(16, Some(1 << 30))).is_ok());
    let errors = export::plan(&ws, &options(16, Some(512 << 20))).unwrap_err();
    assert_error(&errors, "/timelines/0", "more than 512 frames");
}
