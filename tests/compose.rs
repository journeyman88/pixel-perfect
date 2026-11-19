//! Compositing, panels and scenes: scenarios of the composition spec.

mod common;

use common::load;
use pixel_perfect::compose::{self, blend, draw, source_over};
use pixel_perfect::model::Rgba;
use pixel_perfect::raster::{Image, TRANSPARENT, canonical};
use serde_json::{Value, json};

// Letter-coded images where `.` is transparent.

fn color(c: char) -> Rgba {
    if c == '.' {
        TRANSPARENT
    } else {
        Rgba([c as u8, 0, 0, 255])
    }
}

fn pic(rows: &[&str]) -> Image {
    let width = rows[0].chars().count() as u32;
    let pixels = rows.iter().flat_map(|r| r.chars().map(color)).collect();
    Image::new(width, rows.len() as u32, pixels)
}

fn show(image: &Image) -> Vec<String> {
    image
        .pixels()
        .chunks(image.width() as usize)
        .map(|row| {
            row.iter()
                .map(|p| {
                    if *p == TRANSPARENT {
                        '.'
                    } else {
                        p.0[0] as char
                    }
                })
                .collect()
        })
        .collect()
}

// Alpha compositing

#[test]
fn opaque_source_wins() {
    for dst in [TRANSPARENT, Rgba([1, 2, 3, 40]), Rgba([9, 9, 9, 255])] {
        assert_eq!(
            source_over(Rgba([10, 20, 30, 255]), dst),
            Rgba([10, 20, 30, 255])
        );
    }
}

#[test]
fn transparent_source_keeps_destination() {
    for dst in [TRANSPARENT, Rgba([1, 2, 3, 40]), Rgba([9, 9, 9, 255])] {
        assert_eq!(source_over(TRANSPARENT, dst), dst);
    }
}

#[test]
fn source_over_transparent_destination() {
    assert_eq!(
        source_over(Rgba([10, 20, 30, 77]), TRANSPARENT),
        Rgba([10, 20, 30, 77])
    );
}

#[test]
fn semi_transparent_over_opaque() {
    assert_eq!(
        source_over(Rgba([255, 0, 0, 128]), Rgba([0, 0, 255, 255])),
        Rgba([128, 0, 127, 255])
    );
}

#[test]
fn semi_transparent_over_semi_transparent() {
    assert_eq!(
        source_over(Rgba([255, 255, 255, 128]), Rgba([0, 0, 0, 128])),
        Rgba([170, 170, 170, 192])
    );
}

#[test]
fn result_alpha_is_zero_only_when_both_are_zero() {
    for sa in 0..=255u8 {
        for da in 0..=255u8 {
            let out = source_over(
                canonical(Rgba([200, 100, 50, sa])),
                canonical(Rgba([7, 8, 9, da])),
            );
            assert_eq!(out.0[3] == 0, sa == 0 && da == 0, "sa {sa}, da {da}");
            if out.0[3] == 0 {
                assert_eq!(out, TRANSPARENT);
            }
        }
    }
}

#[test]
fn shortcuts_agree_with_the_formula() {
    let colors = [[0, 0, 0], [255, 255, 255], [200, 100, 50], [1, 254, 128]];
    for sa in 0..=255u8 {
        for da in 0..=255u8 {
            for s in colors {
                for d in colors {
                    let src = canonical(Rgba([s[0], s[1], s[2], sa]));
                    let dst = canonical(Rgba([d[0], d[1], d[2], da]));
                    assert_eq!(
                        blend(src, dst),
                        source_over(src, dst),
                        "{src:?} over {dst:?}"
                    );
                }
            }
        }
    }
}

// Drawing with clipping

#[test]
fn placement() {
    let mut canvas = Image::transparent(8, 4);
    draw(&mut canvas, &pic(&["AB", "CD"]), 3, 1);
    assert_eq!(
        show(&canvas),
        ["........", "...AB...", "...CD...", "........"]
    );
}

#[test]
fn negative_coordinates() {
    let mut canvas = Image::transparent(3, 3);
    draw(&mut canvas, &pic(&["ABCD", "EFGH", "IJKL", "MNOP"]), -2, -1);
    assert_eq!(show(&canvas), ["GH.", "KL.", "OP."]);
}

#[test]
fn partly_outside_right_and_bottom() {
    let mut canvas = Image::transparent(3, 2);
    draw(&mut canvas, &pic(&["AB", "CD"]), 2, 1);
    assert_eq!(show(&canvas), ["...", "..A"]);
}

#[test]
fn completely_outside() {
    let mut canvas = Image::transparent(4, 2);
    for (x, y) in [(100, 0), (-5, 0), (0, 2), (0, -2), (i64::MAX, i64::MIN)] {
        draw(&mut canvas, &pic(&["AB", "CD"]), x, y);
    }
    assert_eq!(canvas, Image::transparent(4, 2));
}

#[test]
fn image_larger_than_the_canvas() {
    let mut canvas = Image::transparent(2, 1);
    draw(&mut canvas, &pic(&["ABCD", "EFGH"]), -1, -1);
    assert_eq!(show(&canvas), ["FG"]);
}

#[test]
fn drawing_blends() {
    let mut canvas = Image::new(1, 1, vec![Rgba([0, 0, 255, 255])]);
    draw(
        &mut canvas,
        &Image::new(1, 1, vec![Rgba([255, 0, 0, 128])]),
        0,
        0,
    );
    assert_eq!(canvas.pixels(), [Rgba([128, 0, 127, 255])]);
}

// Panels and scenes, from worksheets

/// Palette `p`: `A`, `B`, `C` opaque; `w` white at 0.1; `t` teal at 0.1;
/// `k` opaque black.
fn sheet(extra: Value) -> Value {
    let mut doc = json!({
        "palettes": [{ "id": "p", "colors": [
            { "char": "A", "rgba": [65, 0, 0] },
            { "char": "B", "rgba": [66, 0, 0] },
            { "char": "C", "rgba": [67, 0, 0] },
            { "char": "w", "rgba": [255, 255, 255, 0.1] },
            { "char": "t", "rgba": [10, 200, 90, 0.1] },
            { "char": "k", "rgba": [0, 0, 0] }
        ] }],
        "sprites": [
            { "id": "a", "_palette": "p", "lines": ["A"] },
            { "id": "b", "_palette": "p", "lines": ["B"] },
            { "id": "abc", "_palette": "p", "lines": ["ABC", "CBA"] },
            { "id": "big", "_palette": "p", "lines": ["AAAA", "AAAA", "AAAA", "AAAA"] },
            { "id": "w", "_palette": "p", "lines": ["w"] },
            { "id": "t", "_palette": "p", "lines": ["t"] },
            { "id": "k", "_palette": "p", "lines": ["k"] }
        ],
        "flipbooks": [{ "id": "f", "width": 1, "height": 1, "fps": 1,
            "frames": [{ "_sprite": "a", "coordinates": [0, 0], "duration": 1 }] }]
    });
    for (k, v) in extra.as_object().unwrap() {
        doc[k] = v.clone();
    }
    doc
}

fn panel_of(elements: Value, size: [u32; 2]) -> Image {
    let ws = load(sheet(json!({
        "panels": [{ "id": "x", "width": size[0], "height": size[1], "elements": elements }]
    })));
    compose::panel(&ws, "x")
}

#[test]
fn higher_z_index_on_top() {
    let p = panel_of(
        json!([
            { "_sprite": "a", "coordinates": [0, 0], "zIndex": 2 },
            { "_sprite": "b", "coordinates": [0, 0] }
        ]),
        [1, 1],
    );
    assert_eq!(show(&p), ["A"]);
}

#[test]
fn tie_broken_by_array_order() {
    let p = panel_of(
        json!([
            { "_sprite": "a", "coordinates": [0, 0], "zIndex": 1 },
            { "_sprite": "b", "coordinates": [0, 0], "zIndex": 1 }
        ]),
        [1, 1],
    );
    assert_eq!(show(&p), ["B"]);
}

#[test]
fn negative_z_index_below_default() {
    let p = panel_of(
        json!([
            { "_sprite": "a", "coordinates": [0, 0] },
            { "_sprite": "b", "coordinates": [0, 0], "zIndex": -1 }
        ]),
        [1, 1],
    );
    assert_eq!(show(&p), ["A"]);
}

#[test]
fn hidden_element_is_not_drawn() {
    let with = panel_of(
        json!([
            { "_sprite": "a", "coordinates": [0, 0] },
            { "_sprite": "b", "coordinates": [0, 0], "visible": false }
        ]),
        [2, 1],
    );
    let without = panel_of(json!([{ "_sprite": "a", "coordinates": [0, 0] }]), [2, 1]);
    assert_eq!(with, without);
    assert_eq!(show(&with), ["A."], "uncovered pixels are transparent");
}

#[test]
fn panel_placement_and_clipping() {
    let p = panel_of(
        json!([
            { "_sprite": "abc", "coordinates": [3, 1] },
            { "_sprite": "big", "coordinates": [-3, -3] },
            { "_sprite": "big", "coordinates": [100, 0] }
        ]),
        [8, 4],
    );
    assert_eq!(show(&p), ["A.......", "...ABC..", "...CBA..", "........"]);
}

#[test]
fn transforms_before_placement() {
    let p = panel_of(
        json!([{ "_sprite": "abc", "coordinates": [0, 0],
                 "transforms": [{ "kind": "rotate", "degrees": 90 }] }]),
        [3, 3],
    );
    assert_eq!(show(&p), ["CA.", "BB.", "AC."]);
}

#[test]
fn hidden_flipbook_element_is_ignored() {
    let p = panel_of(
        json!([
            { "_sprite": "a", "coordinates": [0, 0] },
            { "_flipbook": "f", "coordinates": [1, 0], "visible": false }
        ]),
        [2, 1],
    );
    assert_eq!(show(&p), ["A."]);
}

fn scene_sheet(layers: Value, panels: Value) -> pixel_perfect::ResolvedWorksheet {
    load(sheet(json!({
        "panels": panels,
        "scenes": [{ "id": "s", "width": 4, "height": 2, "layers": layers }]
    })))
}

#[test]
fn scene_layer_order() {
    let ws = scene_sheet(
        json!([
            { "_panel": "pa", "coordinates": [0, 0], "zIndex": 1 },
            { "_panel": "pb", "coordinates": [0, 0] }
        ]),
        json!([
            { "id": "pa", "width": 1, "height": 1, "elements": [{ "_sprite": "a", "coordinates": [0, 0] }] },
            { "id": "pb", "width": 1, "height": 1, "elements": [{ "_sprite": "b", "coordinates": [0, 0] }] }
        ]),
    );
    assert_eq!(show(&compose::scene(&ws, "s")), ["A...", "...."]);
}

#[test]
fn scene_layer_clipped_to_the_scene() {
    let ws = scene_sheet(
        json!([{ "_panel": "wide", "coordinates": [2, 1] }]),
        json!([{ "id": "wide", "width": 4, "height": 2, "elements": [
            { "_sprite": "big", "coordinates": [0, 0] }
        ] }]),
    );
    assert_eq!(show(&compose::scene(&ws, "s")), ["....", "..AA"]);
}

#[test]
fn layer_is_clipped_to_its_panel_first() {
    // The sprite is larger than its panel: only the panel area reaches the scene.
    let ws = scene_sheet(
        json!([{ "_panel": "small", "coordinates": [1, 0] }]),
        json!([{ "id": "small", "width": 1, "height": 1, "elements": [
            { "_sprite": "big", "coordinates": [0, 0] }
        ] }]),
    );
    assert_eq!(show(&compose::scene(&ws, "s")), [".A..", "...."]);
}

#[test]
fn panel_is_composited_as_a_group() {
    // white 0.1 then teal 0.1, over opaque black. Flattening the panel first
    // gives (24, 43, 32); drawing both directly on black would give (24, 44, 33).
    let ws = scene_sheet(
        json!([
            { "_panel": "bg", "coordinates": [0, 0] },
            { "_panel": "glass", "coordinates": [0, 0] }
        ]),
        json!([
            { "id": "bg", "width": 1, "height": 1, "elements": [{ "_sprite": "k", "coordinates": [0, 0] }] },
            { "id": "glass", "width": 1, "height": 1, "elements": [
                { "_sprite": "w", "coordinates": [0, 0] },
                { "_sprite": "t", "coordinates": [0, 0] }
            ] }
        ]),
    );
    let glass = compose::panel(&ws, "glass");
    assert_eq!(glass.pixels(), [Rgba([126, 226, 168, 49])]);
    let scene = compose::scene(&ws, "s");
    assert_eq!(scene.get(0, 0), Rgba([24, 43, 32, 255]));
}

// Static or animated

fn animated(extra: Value) -> (bool, bool) {
    let ws = load(sheet(extra));
    let panel = ws.model.panels.iter().find(|p| p.id == "x");
    let scene = ws.model.scenes.iter().find(|s| s.id == "s");
    (
        panel.is_some_and(compose::is_animated_panel),
        scene.is_some_and(|s| compose::is_animated_scene(&ws, s)),
    )
}

#[test]
fn visible_flipbook_makes_a_panel_animated() {
    let (panel, _) = animated(json!({ "panels": [{ "id": "x", "width": 1, "height": 1,
        "elements": [{ "_flipbook": "f", "coordinates": [0, 0] }] }] }));
    assert!(panel);
}

#[test]
fn hidden_flipbook_keeps_a_panel_static() {
    let (panel, _) = animated(json!({ "panels": [{ "id": "x", "width": 1, "height": 1,
        "elements": [
            { "_sprite": "a", "coordinates": [0, 0] },
            { "_flipbook": "f", "coordinates": [0, 0], "visible": false }
        ] }] }));
    assert!(!panel);
}

#[test]
fn timeline_layer_makes_a_scene_animated() {
    let (_, scene) = animated(json!({
        "panels": [{ "id": "x", "width": 1, "height": 1, "elements": [{ "id": "e", "_sprite": "a", "coordinates": [0, 0] }] }],
        "timelines": [{ "id": "tl", "_panel": "x", "fps": 1, "duration": 2,
            "transitions": [{ "kind": "setVisible", "_element": "e", "value": false, "at": 1 }] }],
        "scenes": [{ "id": "s", "width": 1, "height": 1, "layers": [
            { "_panel": "x", "coordinates": [0, 0] },
            { "_timeline": "tl", "coordinates": [0, 0] }
        ] }]
    }));
    assert!(scene);
}

#[test]
fn animated_panel_makes_a_scene_animated() {
    let (panel, scene) = animated(json!({
        "panels": [{ "id": "x", "width": 1, "height": 1, "elements": [{ "_flipbook": "f", "coordinates": [0, 0] }] }],
        "scenes": [{ "id": "s", "width": 1, "height": 1, "layers": [{ "_panel": "x", "coordinates": [0, 0] }] }]
    }));
    assert!(panel && scene);
}

#[test]
fn static_scene() {
    let (panel, scene) = animated(json!({
        "panels": [{ "id": "x", "width": 1, "height": 1, "elements": [{ "_sprite": "a", "coordinates": [0, 0] }] }],
        "scenes": [{ "id": "s", "width": 1, "height": 1, "layers": [{ "_panel": "x", "coordinates": [0, 0] }] }]
    }));
    assert!(!panel && !scene);
}
