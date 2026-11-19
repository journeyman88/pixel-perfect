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
