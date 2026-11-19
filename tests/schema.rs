// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 The pixel-perfect Authors

//! Schema validation (tasks 3.3-3.5).

mod common;

use common::{assert_error_within, errors, full, load, remove, set};
use serde_json::json;

#[test]
fn full_fixture_is_valid_and_covers_every_kind() {
    // full.json uses every transform kind and every transition kind.
    load(full());
}

#[test]
fn control_character_is_rejected_as_char() {
    // Exercises the `\p{Cc}` pattern in the embedded schema.
    for c in ["\u{7}", "\u{0}", "\u{9f}"] {
        let mut doc = full();
        set(&mut doc, "/palettes/0/colors/0/char", json!(c));
        assert_error_within(&errors(doc), "/palettes/0/colors/0/char");
    }
}

#[test]
fn reserved_and_whitespace_chars_are_rejected() {
    for c in [".", "=", " ", "rr"] {
        let mut doc = full();
        set(&mut doc, "/palettes/0/colors/0/char", json!(c));
        assert_error_within(&errors(doc), "/palettes/0/colors/0/char");
    }
}

#[test]
fn missing_id() {
    let mut doc = full();
    remove(&mut doc, "/sprites/0/id");
    assert_error_within(&errors(doc), "/sprites/0");
}

#[test]
fn lines_together_with_base_sprite() {
    let mut doc = full();
    set(&mut doc, "/sprites/1/lines", json!(["rrgg"]));
    assert_error_within(&errors(doc), "/sprites/1");
}

#[test]
fn element_with_two_sources() {
    let mut doc = full();
    set(&mut doc, "/panels/0/elements/0/_flipbook", json!("spin"));
    assert_error_within(&errors(doc), "/panels/0/elements/0");
}

#[test]
fn unknown_top_level_property() {
    let mut doc = full();
    set(&mut doc, "/colours", json!([]));
    assert!(!errors(doc).is_empty());
}

#[test]
fn several_violations_are_all_reported() {
    let mut doc = full();
    remove(&mut doc, "/sprites/0/id");
    set(&mut doc, "/palettes/0/colors/0/char", json!("."));
    set(&mut doc, "/scenes/0/width", json!(0));
    let diagnostics = errors(doc);
    assert_error_within(&diagnostics, "/sprites/0");
    assert_error_within(&diagnostics, "/palettes/0/colors/0/char");
    assert_error_within(&diagnostics, "/scenes/0/width");
}

#[test]
fn invalid_transforms() {
    let base = "/flipbooks/0/frames/2/transforms/0";
    let cases = [
        json!({ "kind": "flip" }),
        json!({ "kind": "flip", "axis": "z" }),
        json!({ "kind": "flip", "axis": "x", "degrees": 90 }),
        json!({ "kind": "rotate", "degrees": 45 }),
        json!({ "kind": "scale", "width": 8 }),
        json!({ "kind": "scale", "width": 0, "height": 8 }),
        json!({ "kind": "crop", "area": [0, 0, 8] }),
        json!({ "kind": "crop", "area": [0, 0, 0, 1] }),
        json!({ "kind": "tile", "width": 8 }),
        json!({ "kind": "tile", "height": 8 }),
        json!({ "kind": "tile", "width": 0, "height": 8 }),
        json!({ "kind": "tile", "width": 8, "height": 8, "offset": [1, 1] }),
        json!({ "kind": "skew" }),
    ];
    for case in cases {
        let mut doc = full();
        set(&mut doc, base, case.clone());
        let diagnostics = errors(doc);
        assert!(
            diagnostics
                .iter()
                .any(|d| d.location.to_string().starts_with(base)),
            "{case} was not rejected at {base}:\n{}",
            common::render(&diagnostics)
        );
    }
}

#[test]
fn tile_is_accepted_in_every_transform_chain() {
    let tile = json!({ "kind": "tile", "width": 12, "height": 9 });
    for pointer in [
        "/flipbooks/0/frames/0/transforms",
        "/panels/0/elements/0/transforms",
        "/timelines/0/transitions/3/transforms",
    ] {
        let mut doc = full();
        set(&mut doc, pointer, json!([tile.clone()]));
        load(doc);
    }
}

#[test]
fn invalid_transitions() {
    let cases = [
        (
            "/timelines/0/transitions/0",
            json!({ "kind": "moveTo", "_element": "hero", "coordinates": [1, 1], "duration": 3 }),
        ),
        (
            "/timelines/0/transitions/0",
            json!({ "kind": "moveTo", "_element": "hero", "coordinates": [1, 1], "start": 0, "duration": 3, "at": 1 }),
        ),
        (
            "/timelines/0/transitions/1",
            json!({ "kind": "setVisible", "_element": "spinner", "value": "yes", "at": 1 }),
        ),
        (
            "/timelines/0/transitions/2",
            json!({ "kind": "moveToZ", "_element": "spinner", "value": 1.5, "at": 1 }),
        ),
        (
            "/timelines/0/transitions/3",
            json!({ "kind": "appendTransforms", "_element": "hero", "transforms": [], "at": 1 }),
        ),
        (
            "/timelines/0/transitions/4",
            json!({ "kind": "resetTransforms", "_element": "hero", "at": 1, "value": true }),
        ),
        (
            "/timelines/0/transitions/4",
            json!({ "kind": "resetTransforms", "at": 1 }),
        ),
        (
            "/timelines/0/transitions/4",
            json!({ "kind": "teleport", "_element": "hero", "at": 1 }),
        ),
        (
            "/timelines/0/transitions/5",
            json!({ "kind": "seekFlipbook", "_element": "spinner", "frameIndex": -1, "at": 1 }),
        ),
        (
            "/timelines/0/transitions/5",
            json!({ "kind": "seekFlipbook", "_element": "spinner", "frameIndex": 0 }),
        ),
        (
            "/timelines/0/transitions/5",
            json!({ "kind": "seekFlipbook", "_element": "spinner", "at": 1, "value": 2 }),
        ),
    ];
    for (pointer, case) in cases {
        let mut doc = full();
        set(&mut doc, pointer, case.clone());
        let diagnostics = errors(doc);
        assert!(
            diagnostics
                .iter()
                .any(|d| d.location.to_string().starts_with(pointer)),
            "{case} was not rejected at {pointer}:\n{}",
            common::render(&diagnostics)
        );
    }
}
