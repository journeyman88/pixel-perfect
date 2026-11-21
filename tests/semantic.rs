// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 The pixel-perfect Authors

//! Semantic validation: one test per scenario of the worksheet-loading spec (task 5.8).

mod common;

use common::{assert_error, errors, full, load, set};
use pixel_perfect::Format;
use serde_json::json;

fn invalid_with(changes: &[(&str, serde_json::Value)]) -> Vec<pixel_perfect::Diagnostic> {
    let mut doc = full();
    for (pointer, value) in changes {
        set(&mut doc, pointer, value.clone());
    }
    errors(doc)
}

// Unique ids

#[test]
fn duplicate_sprite_id() {
    let e = invalid_with(&[("/sprites/2/id", json!("block"))]);
    assert_error(&e, "/sprites/2/id", "duplicate sprite id \"block\"");
}

#[test]
fn same_id_in_different_collections() {
    let mut doc = full();
    set(&mut doc, "/scenes/0/id", json!("stage"));
    set(&mut doc, "/exports/4", json!("/scenes/stage"));
    load(doc);
}

#[test]
fn duplicate_element_id_in_panel() {
    let e = invalid_with(&[("/panels/0/elements/2/id", json!("hero"))]);
    assert_error(
        &e,
        "/panels/0/elements/2/id",
        "duplicate element id \"hero\"",
    );
}

// References

#[test]
fn missing_palette() {
    let e = invalid_with(&[("/sprites/0/_palette", json!("nope"))]);
    assert_error(&e, "/sprites/0/_palette", "unknown palette \"nope\"");
}

#[test]
fn unknown_references_of_every_kind() {
    let cases = [
        ("/palettes/1/_palette", "unknown palette"),
        ("/sprites/1/_sprite", "unknown sprite"),
        ("/flipbooks/0/frames/0/_sprite", "unknown sprite"),
        ("/panels/0/elements/1/_flipbook", "unknown flipbook"),
        ("/timelines/0/_panel", "unknown panel"),
        ("/scenes/0/layers/0/_panel", "unknown panel"),
        ("/scenes/0/layers/1/_timeline", "unknown timeline"),
    ];
    for (pointer, message) in cases {
        let e = invalid_with(&[(pointer, json!("nope"))]);
        assert_error(&e, pointer, message);
    }
}

#[test]
fn unknown_element_in_transition() {
    let e = invalid_with(&[("/timelines/0/transitions/1/_element", json!("door"))]);
    assert_error(
        &e,
        "/timelines/0/transitions/1/_element",
        "no element with id \"door\"",
    );
}

#[test]
fn seek_flipbook_on_a_sprite_element() {
    let e = invalid_with(&[("/timelines/0/transitions/5/_element", json!("hero"))]);
    assert_error(
        &e,
        "/timelines/0/transitions/5/_element",
        "seekFlipbook targets element \"hero\", which is not a flipbook",
    );
}

#[test]
fn seek_flipbook_frame_index_out_of_range() {
    let e = invalid_with(&[("/timelines/0/transitions/5/frameIndex", json!(4))]);
    assert_error(
        &e,
        "/timelines/0/transitions/5/frameIndex",
        "frameIndex 4 is out of range: flipbook \"spin\" has 4 frames",
    );
}

// Timeline transitions (the `walk` timeline of full.json has `duration` 40)

#[test]
fn instant_transition_after_the_end() {
    let e = invalid_with(&[("/timelines/0/transitions/1/at", json!(40))]);
    assert_error(
        &e,
        "/timelines/0/transitions/1/at",
        "at 40 is not inside the timeline (duration 40)",
    );
}

#[test]
fn movement_ending_at_the_loop_boundary() {
    let mut doc = full();
    set(&mut doc, "/timelines/0/transitions/0/start", json!(30));
    set(&mut doc, "/timelines/0/transitions/0/duration", json!(10));
    load(doc);
}

#[test]
fn movement_past_the_end() {
    let e = invalid_with(&[
        ("/timelines/0/transitions/0/start", json!(35)),
        ("/timelines/0/transitions/0/duration", json!(10)),
    ]);
    assert_error(
        &e,
        "/timelines/0/transitions/0/duration",
        "ends at 45, after the end of the timeline (40)",
    );
}

#[test]
fn movement_starting_after_the_end() {
    let e = invalid_with(&[("/timelines/0/transitions/0/start", json!(40))]);
    assert_error(
        &e,
        "/timelines/0/transitions/0/start",
        "start 40 is not inside",
    );
}

fn second_move(start: u64) -> serde_json::Value {
    json!({ "kind": "moveTo", "_element": "hero", "coordinates": [0, 0], "start": start, "duration": 5 })
}

#[test]
fn overlapping_movements() {
    // The first moveTo runs from 0 to 30.
    let e = invalid_with(&[("/timelines/0/transitions/2", second_move(5))]);
    assert_error(
        &e,
        "/timelines/0/transitions/2/start",
        "moveTo on element \"hero\" starts at 5, while the moveTo at index 0 runs until 30",
    );
}

#[test]
fn back_to_back_movements() {
    let mut doc = full();
    set(&mut doc, "/timelines/0/transitions/2", second_move(30));
    load(doc);
}

#[test]
fn movements_on_different_elements_may_overlap() {
    let mut doc = full();
    let mut other = second_move(5);
    other["_element"] = json!("spinner");
    set(&mut doc, "/timelines/0/transitions/2", other);
    load(doc);
}

// Crops on chains extended by appendTransforms (`hero` is a 4x8 sprite)

fn append(transform: serde_json::Value, at: u64) -> serde_json::Value {
    json!({ "kind": "appendTransforms", "_element": "hero", "transforms": [transform], "at": at })
}

fn rotate() -> serde_json::Value {
    json!({ "kind": "rotate", "degrees": 90 })
}

fn crop() -> serde_json::Value {
    json!({ "kind": "crop", "area": [0, 0, 8, 4] })
}

#[test]
fn appended_crop_after_an_appended_rotation() {
    let mut doc = full();
    set(&mut doc, "/timelines/0/transitions/3", append(rotate(), 2));
    set(&mut doc, "/timelines/0/transitions/2", append(crop(), 4));
    load(doc);
}

#[test]
fn appended_crop_out_of_bounds() {
    let e = invalid_with(&[("/timelines/0/transitions/3", append(crop(), 4))]);
    assert_error(
        &e,
        "/timelines/0/transitions/3/transforms/0/area",
        "does not fit in the 4x8 image",
    );
}

#[test]
fn reset_restores_the_declared_chain() {
    let e = invalid_with(&[
        ("/timelines/0/transitions/3", append(rotate(), 2)),
        (
            "/timelines/0/transitions/4",
            json!({ "kind": "resetTransforms", "_element": "hero", "at": 3 }),
        ),
        ("/timelines/0/transitions/2", append(crop(), 4)),
    ]);
    assert_error(
        &e,
        "/timelines/0/transitions/2/transforms/0/area",
        "does not fit in the 4x8 image",
    );
}

#[test]
fn appended_chain_follows_at_order_not_array_order() {
    // The crop comes first in the array but is applied after the rotation.
    let mut doc = full();
    set(&mut doc, "/timelines/0/transitions/2", append(crop(), 30));
    set(&mut doc, "/timelines/0/transitions/3", append(rotate(), 20));
    load(doc);
}

#[test]
fn appended_crop_after_an_appended_tile() {
    let tile = json!({ "kind": "tile", "width": 12, "height": 8 });
    let wide_crop = json!({ "kind": "crop", "area": [6, 0, 6, 8] });
    let mut doc = full();
    set(
        &mut doc,
        "/timelines/0/transitions/3",
        append(tile.clone(), 2),
    );
    set(
        &mut doc,
        "/timelines/0/transitions/2",
        append(wide_crop.clone(), 4),
    );
    load(doc);

    // Without the tile, the same crop does not fit the 4x8 sprite.
    let e = invalid_with(&[("/timelines/0/transitions/2", append(wide_crop, 4))]);
    assert_error(
        &e,
        "/timelines/0/transitions/2/transforms/0/area",
        "does not fit in the 4x8 image",
    );
}

// Inheritance cycles

#[test]
fn palette_cycle() {
    let e = invalid_with(&[("/palettes/0/_palette", json!("night-deep"))]);
    assert_error(
        &e,
        "/palettes/1/_palette",
        "base -> night-deep -> night -> base",
    );
    assert_eq!(
        e.iter().filter(|d| d.message.contains("cycle")).count(),
        1,
        "one report per cycle"
    );
}

#[test]
fn self_derived_sprite() {
    let e = invalid_with(&[("/sprites/2/_sprite", json!("block-deep"))]);
    assert_error(&e, "/sprites/2/_sprite", "block-deep -> block-deep");
}

// Palette characters

#[test]
fn duplicate_char_in_one_palette() {
    let e = invalid_with(&[("/palettes/0/colors/1/char", json!("r"))]);
    assert_error(
        &e,
        "/palettes/0/colors/1/char",
        "'r' is already defined at index 0",
    );
}

// Sprite grids

#[test]
fn ragged_rows() {
    let e = invalid_with(&[("/sprites/0/lines/3", json!("www"))]);
    assert_error(
        &e,
        "/sprites/0/lines/3",
        "row 3 is 3 pixels wide, but row 0 is 4",
    );
}

#[test]
fn width_is_counted_in_chars_not_bytes() {
    let mut doc = full();
    set(
        &mut doc,
        "/palettes/0/colors/3",
        json!({ "char": "é", "rgba": [1, 2, 3] }),
    );
    set(&mut doc, "/sprites/0/lines/4", json!("éééé"));
    let ws = load(doc);
    assert_eq!(ws.sprites["block"].width(), 4);
}

#[test]
fn declared_size_mismatch() {
    let e = invalid_with(&[("/sprites/0/height", json!(9))]);
    assert_error(&e, "/sprites/0/height", "declared height 9");
    let e = invalid_with(&[("/sprites/0/width", json!(3))]);
    assert_error(&e, "/sprites/0/width", "declared width 3");
}

#[test]
fn unknown_character() {
    let e = invalid_with(&[("/sprites/0/lines/2", json!("wwzw"))]);
    assert_error(
        &e,
        "/sprites/0/lines/2",
        "'z' at row 2, column 2 is not in palette \"base\"",
    );
}

#[test]
fn palette_swap_removes_a_character() {
    // block-deep inherits `b` from block-night's overrides; `base` has no `b`.
    let e = invalid_with(&[("/sprites/2/_palette", json!("base"))]);
    assert_error(
        &e,
        "/sprites/2/_palette",
        "'b' at row 0, column 2 is not in palette \"base\"",
    );
}

// Line overrides

#[test]
fn override_index_out_of_range() {
    let e = invalid_with(&[("/sprites/1/overrideLines/9", json!("rrrr"))]);
    assert_error(
        &e,
        "/sprites/1/overrideLines/9",
        "row index 9 is out of range",
    );
}

#[test]
fn override_wrong_width() {
    let e = invalid_with(&[("/sprites/1/overrideLines/7", json!("bbb"))]);
    assert_error(
        &e,
        "/sprites/1/overrideLines/7",
        "override is 3 pixels wide",
    );
}

#[test]
fn override_unknown_character_points_to_the_override() {
    let e = invalid_with(&[("/sprites/1/overrideLines/7", json!("bbzb"))]);
    assert_error(&e, "/sprites/1/overrideLines/7", "'z' at row 7");
}

#[test]
fn pass_through_character() {
    let text = r#"{
        "palettes": [{ "id": "p", "colors": [{ "char": "h", "rgba": [1, 1, 1] }, { "char": "r", "rgba": [2, 2, 2] }] }],
        "sprites": [
            { "id": "base", "_palette": "p", "lines": [".hhhhh."] },
            { "id": "derived", "_sprite": "base", "overrideLines": { "0": "==rr===" } }
        ]
    }"#;
    let ws = pixel_perfect::load_str(text, Format::Json).unwrap();
    let row: String = ws.sprites["derived"].rows[0].iter().collect();
    assert_eq!(row, ".hrrhh.");
}

// Crop bounds

#[test]
fn crop_after_rotation_is_valid() {
    // full.json: frame 1 rotates the 4x8 `block` by 90 degrees and crops [0, 0, 8, 4].
    load(full());
}

#[test]
fn crop_out_of_bounds() {
    let e = invalid_with(&[(
        "/flipbooks/0/frames/1/transforms",
        json!([{ "kind": "crop", "area": [2, 0, 4, 8] }]),
    )]);
    assert_error(
        &e,
        "/flipbooks/0/frames/1/transforms/0/area",
        "does not fit in the 4x8 image",
    );
}

#[test]
fn crop_follows_scale_and_previous_crop() {
    let e = invalid_with(&[(
        "/flipbooks/0/frames/3/transforms",
        json!([
            { "kind": "scale", "width": 8, "height": 16 },
            { "kind": "crop", "area": [0, 8, 8, 8] },
            { "kind": "crop", "area": [4, 4, 5, 1] }
        ]),
    )]);
    assert_error(
        &e,
        "/flipbooks/0/frames/3/transforms/2/area",
        "does not fit in the 8x8 image",
    );
}

#[test]
fn crop_after_tile() {
    // `hero` is the 4x8 `block`.
    let mut doc = full();
    set(
        &mut doc,
        "/panels/0/elements/0/transforms",
        json!([
            { "kind": "tile", "width": 32, "height": 8 },
            { "kind": "crop", "area": [20, 0, 12, 8] }
        ]),
    );
    load(doc);
}

#[test]
fn crop_outside_the_tiled_area() {
    let e = invalid_with(&[(
        "/panels/0/elements/0/transforms",
        json!([
            { "kind": "tile", "width": 16, "height": 8 },
            { "kind": "crop", "area": [8, 0, 12, 8] }
        ]),
    )]);
    assert_error(
        &e,
        "/panels/0/elements/0/transforms/1/area",
        "does not fit in the 16x8 image",
    );
}

#[test]
fn crop_on_flipbook_element_uses_the_canvas_size() {
    let e = invalid_with(&[(
        "/panels/0/elements/1/transforms/0/area",
        json!([4, 4, 6, 6]),
    )]);
    assert_error(&e, "/panels/0/elements/1/transforms/0/area", "8x8 image");
}

// Exports

#[test]
fn unknown_export() {
    let e = invalid_with(&[("/exports/4", json!("/scenes/nope"))]);
    assert_error(&e, "/exports/4", "\"/scenes/nope\" does not resolve");
}

#[test]
fn errors_are_collected_not_stopped_at_the_first() {
    let e = invalid_with(&[
        ("/sprites/0/_palette", json!("nope")),
        ("/exports/4", json!("/scenes/nope")),
        ("/timelines/0/transitions/1/_element", json!("door")),
    ]);
    assert!(e.len() >= 3, "{}", common::render(&e));
}

// ---------------------------------------------------------------- sequences

fn sequence_doc() -> serde_json::Value {
    let text = std::fs::read_to_string(common::fixture("valid/sequence.yaml")).unwrap();
    pixel_perfect::input::parse(&text, Format::Yaml).unwrap()
}

fn sequence_errors(changes: &[(&str, serde_json::Value)]) -> Vec<pixel_perfect::Diagnostic> {
    let mut doc = sequence_doc();
    for (pointer, value) in changes {
        set(&mut doc, pointer, value.clone());
    }
    errors(doc)
}

#[test]
fn duplicate_layer_id() {
    let errs = sequence_errors(&[("/scenes/0/layers/2/id", json!("far"))]);
    assert_error(
        &errs,
        "/scenes/0/layers/2/id",
        "duplicate layer id \"far\" in scene \"valley\"",
    );
}

#[test]
fn same_layer_id_in_two_scenes() {
    let mut doc = sequence_doc();
    let scene = doc["scenes"][0].clone();
    let mut copy = scene;
    copy["id"] = json!("valley-2");
    doc["scenes"].as_array_mut().unwrap().push(copy);
    load(doc);
}

#[test]
fn unknown_scene() {
    let errs = sequence_errors(&[("/sequences/0/_scene", json!("night"))]);
    assert_error(&errs, "/sequences/0/_scene", "unknown scene \"night\"");
}

#[test]
fn unknown_layer_fixture() {
    let err = pixel_perfect::load_file(&common::fixture("invalid/sequence-unknown-layer.yaml"))
        .unwrap_err();
    assert_error(
        err.diagnostics(),
        "/sequences/0/actions/0/_layer",
        "scene \"night\" has no layer with id \"lamp\"",
    );
}

#[test]
fn sequence_instant_action_after_the_end() {
    let errs = sequence_errors(&[("/sequences/0/actions/2/at", json!(8))]);
    assert_error(
        &errs,
        "/sequences/0/actions/2/at",
        "at 8 is not inside the sequence (duration 8)",
    );
}

#[test]
fn sequence_movement_ending_at_the_loop_boundary() {
    let mut doc = sequence_doc();
    set(&mut doc, "/sequences/0/actions/0/start", json!(4));
    set(&mut doc, "/sequences/0/actions/0/duration", json!(4));
    load(doc);
}

#[test]
fn sequence_movement_past_the_end() {
    let errs = sequence_errors(&[
        ("/sequences/0/actions/0/start", json!(5)),
        ("/sequences/0/actions/0/duration", json!(4)),
    ]);
    assert_error(
        &errs,
        "/sequences/0/actions/0/duration",
        "ends at 9, after the end of the sequence (8)",
    );
    let errs = sequence_errors(&[("/sequences/0/actions/0/start", json!(8))]);
    assert_error(
        &errs,
        "/sequences/0/actions/0/start",
        "start 8 is not inside the sequence",
    );
}

#[test]
fn overlapping_layer_movements() {
    let mut doc = sequence_doc();
    set(&mut doc, "/sequences/0/actions/0/duration", json!(4));
    let second = json!({ "kind": "moveTo", "_layer": "far", "coordinates": [2, 1], "start": 2, "duration": 2 });
    doc["sequences"][0]["actions"]
        .as_array_mut()
        .unwrap()
        .push(second);
    let errs = errors(doc);
    assert_error(
        &errs,
        "/sequences/0/actions/4/start",
        "moveTo on layer \"far\" starts at 2, while the moveTo at index 0 runs until 4",
    );
}

#[test]
fn duplicate_sequence_id_and_export() {
    let mut doc = sequence_doc();
    let copy = doc["sequences"][0].clone();
    doc["sequences"].as_array_mut().unwrap().push(copy);
    assert_error(
        &errors(doc),
        "/sequences/1/id",
        "duplicate sequence id \"drift\"",
    );
    let errs = sequence_errors(&[("/exports", json!(["/sequences/nope"]))]);
    assert_error(&errs, "/exports/0", "does not resolve");
    let mut doc = sequence_doc();
    doc["exports"] = json!(["/sequences/drift"]);
    load(doc);
}
