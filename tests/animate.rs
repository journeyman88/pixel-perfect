// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 The pixel-perfect Authors

//! Time model and animation evaluation: scenarios of the animation spec, plus
//! the composition scenarios about animated frames.

mod common;

use common::load;
use pixel_perfect::ResolvedWorksheet;
use pixel_perfect::animate::{self, Animation, ElementState, round_away, timeline_state};
use pixel_perfect::compose;
use pixel_perfect::model::{Axis, Rotation, Transform};
use pixel_perfect::raster::{Image, TRANSPARENT};
use pixel_perfect::time;
use serde_json::{Value, json};

// Worksheet builder: palette `p` maps `A`..`H` to opaque colors whose red
// channel is the letter, plus `w` (white at 0.1) and `t` (teal at 0.1).
// Sprites `a`..`h` are 1x1 of the matching letter; `ab` is the 2x1 `AB`.

fn sheet(extra: Value) -> ResolvedWorksheet {
    let mut colors: Vec<Value> = ('A'..='H')
        .map(|c| json!({ "char": c.to_string(), "rgba": [c as u8, 0, 0] }))
        .collect();
    colors.push(json!({ "char": "w", "rgba": [255, 255, 255, 0.1] }));
    colors.push(json!({ "char": "t", "rgba": [10, 200, 90, 0.1] }));
    let mut sprites: Vec<Value> = ('a'..='h')
        .map(|c| json!({ "id": c.to_string(), "_palette": "p", "lines": [c.to_ascii_uppercase().to_string()] }))
        .collect();
    sprites.push(json!({ "id": "ab", "_palette": "p", "lines": ["AB"] }));
    sprites.push(json!({ "id": "w", "_palette": "p", "lines": ["w"] }));
    sprites.push(json!({ "id": "t", "_palette": "p", "lines": ["t"] }));
    let mut doc = json!({ "palettes": [{ "id": "p", "colors": colors }], "sprites": sprites });
    for (k, v) in extra.as_object().unwrap() {
        doc[k] = v.clone();
    }
    load(doc)
}

/// A flipbook on a 1x1 canvas showing the given sprites with the given durations.
fn flipbook(id: &str, fps: u32, frames: &[(&str, u32)]) -> Value {
    let frames: Vec<Value> = frames
        .iter()
        .map(|(s, d)| json!({ "_sprite": s, "coordinates": [0, 0], "duration": d }))
        .collect();
    json!({ "id": id, "width": 1, "height": 1, "fps": fps, "frames": frames })
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

/// Frames of a resource as (rows joined by `/`, ticks).
fn frames(ws: &ResolvedWorksheet, collection: &str, id: &str) -> Vec<(String, u64)> {
    Animation::new(ws, collection, id)
        .unwrap()
        .frames()
        .iter()
        .map(|(image, ticks)| (show(image).join("/"), *ticks))
        .collect()
}

fn f(rows: &str, ticks: u64) -> (String, u64) {
    (rows.to_owned(), ticks)
}

fn anim<'a>(ws: &'a ResolvedWorksheet, collection: &str, id: &str) -> Animation<'a> {
    Animation::new(ws, collection, id).unwrap()
}

/// A timeline `tl` on panel `x` (`width` x 1) with the given elements and transitions.
fn timeline_sheet(
    fps: u32,
    duration: u64,
    width: u32,
    elements: Value,
    transitions: Value,
    flipbooks: Value,
) -> ResolvedWorksheet {
    let mut extra = json!({
        "panels": [{ "id": "x", "width": width, "height": 1, "elements": elements }],
        "timelines": [{ "id": "tl", "_panel": "x", "fps": fps, "duration": duration, "transitions": transitions }]
    });
    if flipbooks.as_array().is_some_and(|f| !f.is_empty()) {
        extra["flipbooks"] = flipbooks;
    }
    sheet(extra)
}

fn state_at(ws: &ResolvedWorksheet, t: u64) -> Vec<ElementState> {
    timeline_state(&ws.model.timelines[0], &ws.model.panels[0], t)
}

fn hero(coordinates: [i64; 2]) -> Value {
    json!([{ "id": "hero", "_sprite": "a", "coordinates": coordinates }])
}

fn move_to(coordinates: [i64; 2], start: u64, duration: u64) -> Value {
    json!({ "kind": "moveTo", "_element": "hero", "coordinates": coordinates, "start": start, "duration": duration })
}

fn x_of(ws: &ResolvedWorksheet, t: u64) -> i64 {
    state_at(ws, t)[0].coordinates[0]
}

// Common clock

#[test]
fn two_rates() {
    let ws = sheet(json!({
        "flipbooks": [flipbook("fb", 8, &[("a", 1)])],
        "panels": [{ "id": "x", "width": 1, "height": 1, "elements": [{ "id": "e", "_flipbook": "fb", "coordinates": [0, 0] }] }],
        "timelines": [{ "id": "tl", "_panel": "x", "fps": 12, "duration": 1,
            "transitions": [{ "kind": "moveToZ", "_element": "e", "value": 0, "at": 0 }] }],
        "scenes": [{ "id": "s", "width": 1, "height": 1, "layers": [{ "_timeline": "tl", "coordinates": [0, 0] }] }]
    }));
    let a = anim(&ws, "scenes", "s");
    assert_eq!(a.clock, 24);
    assert_eq!(a.clock / 12, 2, "timeline frame-unit");
    assert_eq!(a.clock / 8, 3, "flipbook frame-unit");
    assert_eq!(a.rates, vec![8, 12]);
}

#[test]
fn single_rate() {
    let ws = sheet(json!({ "flipbooks": [flipbook("fb", 10, &[("a", 1)])] }));
    assert_eq!(anim(&ws, "flipbooks", "fb").clock, 10);
}

#[test]
fn clock_overflow_is_an_error() {
    let huge = 1u64 << 62;
    let ws = sheet(json!({
        "panels": [{ "id": "x", "width": 1, "height": 1, "elements": [{ "id": "e", "_sprite": "a", "coordinates": [0, 0] }] }],
        "timelines": [
            { "id": "t1", "_panel": "x", "fps": 7, "duration": huge, "transitions": [{ "kind": "setVisible", "_element": "e", "value": true, "at": 0 }] },
            { "id": "t2", "_panel": "x", "fps": 5, "duration": huge, "transitions": [{ "kind": "setVisible", "_element": "e", "value": true, "at": 0 }] }
        ],
        "scenes": [{ "id": "s", "width": 1, "height": 1, "layers": [
            { "_timeline": "t1", "coordinates": [0, 0] }, { "_timeline": "t2", "coordinates": [0, 0] }
        ] }]
    }));
    assert_eq!(
        Animation::new(&ws, "scenes", "s").unwrap_err(),
        animate::TimeError::TooLong
    );
}

// Loop lengths

#[test]
fn panel_with_two_flipbooks() {
    let ws = sheet(json!({
        "flipbooks": [flipbook("f4", 10, &[("a", 2), ("b", 2)]), flipbook("f6", 10, &[("c", 3), ("d", 3)])],
        "panels": [{ "id": "x", "width": 2, "height": 1, "elements": [
            { "_flipbook": "f4", "coordinates": [0, 0] }, { "_flipbook": "f6", "coordinates": [1, 0] }
        ] }]
    }));
    let a = anim(&ws, "panels", "x");
    assert_eq!((a.loop_len, a.clock), (12, 10), "1.2 s");
}

fn layered_scene(layers: Value) -> ResolvedWorksheet {
    sheet(json!({
        "flipbooks": [flipbook("fb", 4, &[("a", 1), ("b", 2)])],
        "panels": [
            { "id": "anim", "width": 1, "height": 1, "elements": [{ "_flipbook": "fb", "coordinates": [0, 0] }] },
            { "id": "still", "width": 1, "height": 1, "elements": [{ "id": "e", "_sprite": "c", "coordinates": [0, 0] }] }
        ],
        "timelines": [{ "id": "tl", "_panel": "still", "fps": 10, "duration": 20,
            "transitions": [{ "kind": "setVisible", "_element": "e", "value": false, "at": 10 }] }],
        "scenes": [{ "id": "s", "width": 1, "height": 1, "layers": layers }]
    }))
}

#[test]
fn scene_with_a_timeline_and_an_animated_panel() {
    // 2 s timeline, 0.75 s flipbook: 6 s.
    let ws = layered_scene(json!([
        { "_timeline": "tl", "coordinates": [0, 0] },
        { "_panel": "anim", "coordinates": [0, 0] }
    ]));
    let a = anim(&ws, "scenes", "s");
    assert_eq!(a.clock, 20);
    assert_eq!(a.loop_len, 6 * 20);
}

#[test]
fn static_layer_does_not_count() {
    let ws = layered_scene(json!([
        { "_panel": "still", "coordinates": [0, 0] },
        { "_timeline": "tl", "coordinates": [0, 0] }
    ]));
    let a = anim(&ws, "scenes", "s");
    assert_eq!(a.loop_len, 2 * a.clock);
}

// Change ticks

#[test]
fn change_ticks_of_a_flipbook() {
    let ws = sheet(json!({ "flipbooks": [flipbook("fb", 10, &[("a", 2), ("b", 1)])] }));
    let a = anim(&ws, "flipbooks", "fb");
    assert_eq!(a.change_ticks(100), Ok(vec![0, 2]));
    assert_eq!(a.loop_len, 3);
}

#[test]
fn change_ticks_of_set_visible() {
    let ws = timeline_sheet(
        10,
        10,
        1,
        hero([0, 0]),
        json!([{ "kind": "setVisible", "_element": "hero", "value": false, "at": 5 }]),
        json!([]),
    );
    assert_eq!(
        anim(&ws, "timelines", "tl").change_ticks(100),
        Ok(vec![0, 5])
    );
}

#[test]
fn change_ticks_of_move_to() {
    let ws = timeline_sheet(
        10,
        10,
        8,
        hero([0, 0]),
        json!([move_to([3, 0], 2, 3)]),
        json!([]),
    );
    assert_eq!(
        anim(&ws, "timelines", "tl").change_ticks(100),
        Ok(vec![0, 3, 4, 5])
    );
}

fn coprime_panel() -> ResolvedWorksheet {
    sheet(json!({
        "flipbooks": [flipbook("f2", 2, &[("a", 1), ("b", 1)]), flipbook("f3", 3, &[("c", 1), ("d", 1)])],
        "panels": [{ "id": "x", "width": 2, "height": 1, "elements": [
            { "_flipbook": "f2", "coordinates": [0, 0] }, { "_flipbook": "f3", "coordinates": [1, 0] }
        ] }]
    }))
}

#[test]
fn change_ticks_at_coprime_rates() {
    let ws = coprime_panel();
    let a = anim(&ws, "panels", "x");
    assert_eq!((a.clock, a.loop_len), (6, 12));
    assert_eq!(a.change_ticks(100), Ok(vec![0, 2, 3, 4, 6, 8, 9, 10]));
}

#[test]
fn change_ticks_stop_early() {
    let ws = coprime_panel();
    assert_eq!(
        anim(&ws, "panels", "x").change_ticks(3),
        Err(time::Exceeded)
    );
}

// Delays

#[test]
fn two_directly_played_rates_always_fit() {
    for a in 1..=100u64 {
        for b in a..=100u64 {
            let clock = time::lcm(a, b).unwrap();
            // Boundaries are multiples of clock/a and clock/b; every distance
            // between two of them is a multiple of their gcd.
            let step = time::gcd((clock / a).into(), (clock / b).into()) as u64;
            assert!(time::reduced(step, clock).1 <= 10_000, "{a} and {b}");
        }
    }
}

#[test]
fn nested_coprime_rates_can_overflow_the_denominator() {
    let ws = sheet(json!({
        "flipbooks": [flipbook("f89", 89, &[("a", 1), ("b", 1)]), flipbook("f83", 83, &[("c", 1), ("d", 1)])],
        "panels": [
            { "id": "t", "width": 1, "height": 1, "elements": [{ "id": "e", "_flipbook": "f89", "coordinates": [0, 0] }] },
            { "id": "p", "width": 1, "height": 1, "elements": [{ "_flipbook": "f83", "coordinates": [0, 0] }] }
        ],
        "timelines": [{ "id": "tl", "_panel": "t", "fps": 97, "duration": 97,
            "transitions": [{ "kind": "seekFlipbook", "_element": "e", "at": 1 }] }],
        "scenes": [{ "id": "s", "width": 1, "height": 1, "layers": [
            { "_timeline": "tl", "coordinates": [0, 0] }, { "_panel": "p", "coordinates": [0, 0] }
        ] }]
    }));
    let a = anim(&ws, "scenes", "s");
    assert_eq!(a.clock, 716_539);
    let ticks = a.change_ticks(1_000_000).unwrap();
    assert!(a.delay_overflow(&ticks).is_some());
}

// Flipbook playback

#[test]
fn frame_durations() {
    let ws = sheet(json!({ "flipbooks": [flipbook("fb", 10, &[("a", 2), ("b", 1)])] }));
    assert_eq!(frames(&ws, "flipbooks", "fb"), vec![f("A", 2), f("B", 1)]);
    assert_eq!(anim(&ws, "flipbooks", "fb").clock, 10);
}

#[test]
fn frame_clipped_to_the_canvas() {
    let ws = sheet(
        json!({ "flipbooks": [{ "id": "fb", "width": 2, "height": 2, "fps": 1,
        "frames": [{ "_sprite": "ab", "coordinates": [1, 0], "duration": 1 }] }] }),
    );
    assert_eq!(
        show(&anim(&ws, "flipbooks", "fb").render_at(0)),
        [".A", ".."]
    );
}

#[test]
fn frame_transforms() {
    let ws = sheet(
        json!({ "flipbooks": [{ "id": "fb", "width": 2, "height": 1, "fps": 1,
        "frames": [{ "_sprite": "ab", "coordinates": [0, 0], "duration": 1, "transforms": [{ "kind": "flip", "axis": "x" }] }] }] }),
    );
    assert_eq!(show(&anim(&ws, "flipbooks", "fb").render_at(0)), ["BA"]);
}

#[test]
fn frame_lookup_wraps() {
    let ws = sheet(json!({ "flipbooks": [flipbook("fb", 1, &[("a", 2), ("b", 1)])] }));
    let fb = &ws.model.flipbooks[0];
    let shown: Vec<usize> = (0..7).map(|u| animate::flipbook_frame(fb, u)).collect();
    assert_eq!(shown, [0, 0, 1, 0, 0, 1, 0]);
}

// Flipbook elements

#[test]
fn element_plays_its_own_rate() {
    let ws = sheet(json!({
        "flipbooks": [flipbook("fb", 4, &[("a", 1), ("b", 1)])],
        "panels": [{ "id": "x", "width": 1, "height": 1, "elements": [{ "_flipbook": "fb", "coordinates": [0, 0] }] }]
    }));
    let a = anim(&ws, "panels", "x");
    assert_eq!(
        (a.clock, a.loop_len),
        (4, 2),
        "0.25 s per frame, 0.5 s loop"
    );
    assert_eq!(frames(&ws, "panels", "x"), vec![f("A", 1), f("B", 1)]);
}

#[test]
fn element_transforms_apply_to_the_canvas() {
    let ws = sheet(json!({
        "flipbooks": [{ "id": "fb", "width": 2, "height": 2, "fps": 1,
            "frames": [{ "_sprite": "ab", "coordinates": [0, 1], "duration": 1 }] }],
        "panels": [{ "id": "x", "width": 2, "height": 1, "elements": [
            { "_flipbook": "fb", "coordinates": [0, 0], "transforms": [{ "kind": "crop", "area": [0, 1, 2, 1] }] }
        ] }]
    }));
    assert_eq!(show(&anim(&ws, "panels", "x").render_at(0)), ["AB"]);
}

#[test]
fn tiled_flipbook_element() {
    let ws = sheet(json!({
        "flipbooks": [flipbook("fb", 1, &[("a", 1), ("b", 1)])],
        "panels": [{ "id": "x", "width": 3, "height": 1, "elements": [
            { "_flipbook": "fb", "coordinates": [0, 0], "transforms": [{ "kind": "tile", "width": 3, "height": 1 }] }
        ] }]
    }));
    assert_eq!(frames(&ws, "panels", "x"), vec![f("AAA", 1), f("BBB", 1)]);
}

// Timeline evaluation

#[test]
fn loop_restarts_from_the_panel() {
    let ws = timeline_sheet(
        10,
        10,
        1,
        hero([0, 0]),
        json!([{ "kind": "setVisible", "_element": "hero", "value": false, "at": 5 }]),
        json!([]),
    );
    assert_eq!(frames(&ws, "timelines", "tl"), vec![f("A", 5), f(".", 5)]);
    let a = anim(&ws, "timelines", "tl");
    assert_eq!(a.render_at(10), a.render_at(0));
}

#[test]
fn flipbook_faster_than_the_timeline() {
    let ws = timeline_sheet(
        5,
        1,
        1,
        json!([{ "id": "e", "_flipbook": "fb", "coordinates": [0, 0] }]),
        json!([{ "kind": "moveToZ", "_element": "e", "value": 0, "at": 0 }]),
        json!([flipbook("fb", 10, &[("a", 1), ("b", 1)])]),
    );
    assert_eq!(frames(&ws, "timelines", "tl"), vec![f("A", 1), f("B", 1)]);
}

// Instant transitions

#[test]
fn visible_from_at() {
    let ws = timeline_sheet(
        10,
        20,
        1,
        json!([{ "id": "hero", "_sprite": "a", "coordinates": [0, 0], "visible": false }]),
        json!([{ "kind": "setVisible", "_element": "hero", "value": true, "at": 10 }]),
        json!([]),
    );
    assert!(!state_at(&ws, 9)[0].visible);
    assert!(state_at(&ws, 10)[0].visible);
}

#[test]
fn same_at_array_order() {
    let ws = timeline_sheet(
        10,
        20,
        1,
        hero([0, 0]),
        json!([
            { "kind": "moveToZ", "_element": "hero", "value": 5, "at": 3 },
            { "kind": "moveToZ", "_element": "hero", "value": 1, "at": 3 }
        ]),
        json!([]),
    );
    assert_eq!(state_at(&ws, 2)[0].z_index, 0);
    assert_eq!(state_at(&ws, 3)[0].z_index, 1);
}

#[test]
fn append_twice_then_reset() {
    let flip = |axis| json!({ "kind": "flip", "axis": axis });
    let ws = timeline_sheet(
        10,
        20,
        1,
        json!([{ "id": "hero", "_sprite": "a", "coordinates": [0, 0], "transforms": [flip("x")] }]),
        json!([
            { "kind": "resetTransforms", "_element": "hero", "at": 6 },
            { "kind": "appendTransforms", "_element": "hero", "transforms": [{ "kind": "rotate", "degrees": 90 }], "at": 2 },
            { "kind": "appendTransforms", "_element": "hero", "transforms": [flip("y")], "at": 4 }
        ]),
        json!([]),
    );
    let fx = Transform::Flip { axis: Axis::X };
    let fy = Transform::Flip { axis: Axis::Y };
    let r90 = Transform::Rotate {
        degrees: Rotation::Deg90,
    };
    let chain = |t| state_at(&ws, t)[0].transforms.clone();
    assert_eq!(chain(1), vec![fx]);
    assert_eq!(chain(2), vec![fx, r90]);
    assert_eq!(chain(4), vec![fx, r90, fy]);
    assert_eq!(chain(6), vec![fx]);
}

// Linear movement

#[test]
fn exact_steps() {
    let ws = timeline_sheet(
        10,
        20,
        16,
        hero([0, 0]),
        json!([move_to([10, 0], 0, 5)]),
        json!([]),
    );
    let xs: Vec<i64> = (0..7).map(|t| x_of(&ws, t)).collect();
    assert_eq!(xs, [0, 2, 4, 6, 8, 10, 10]);
}

#[test]
fn halves_round_away_from_zero() {
    let ws = timeline_sheet(
        10,
        20,
        4,
        hero([0, 0]),
        json!([move_to([1, 0], 0, 2)]),
        json!([]),
    );
    assert_eq!([x_of(&ws, 0), x_of(&ws, 1), x_of(&ws, 2)], [0, 1, 1]);
    assert_eq!(round_away(1, 2), 1);
    assert_eq!(round_away(-1, 2), -1);
    assert_eq!(round_away(-3, 4), -1);
    assert_eq!(round_away(-1, 4), 0);
}

#[test]
fn mirrored_movement() {
    let ws = timeline_sheet(
        10,
        20,
        4,
        hero([0, 0]),
        json!([move_to([-1, 0], 0, 2)]),
        json!([]),
    );
    assert_eq!(x_of(&ws, 1), -1);
}

#[test]
fn chained_movements() {
    let ws = timeline_sheet(
        10,
        20,
        8,
        hero([0, 0]),
        json!([move_to([4, 8], 6, 2), move_to([4, 0], 0, 4)]),
        json!([]),
    );
    let at = |t| state_at(&ws, t)[0].coordinates;
    assert_eq!([at(4), at(5), at(6)], [[4, 0]; 3]);
    assert_eq!(at(7), [4, 4]);
    assert_eq!(at(8), [4, 8]);
    assert_eq!(at(19), [4, 8]);
}

#[test]
fn seamless_loop() {
    let ws = timeline_sheet(
        10,
        8,
        16,
        hero([0, 0]),
        json!([move_to([8, 0], 0, 4), move_to([0, 0], 4, 4)]),
        json!([]),
    );
    let xs: Vec<i64> = (0..8).map(|t| x_of(&ws, t)).collect();
    assert_eq!(xs, [0, 2, 4, 6, 8, 6, 4, 2]);
}

// Flipbook phase in timelines

fn abcd() -> Value {
    json!([flipbook(
        "fb",
        10,
        &[("a", 1), ("b", 1), ("c", 1), ("d", 1)]
    )])
}

fn seek_sheet(element: Value, transitions: Value) -> ResolvedWorksheet {
    timeline_sheet(10, 20, 1, json!([element]), transitions, abcd())
}

fn shown_at(ws: &ResolvedWorksheet, tick: u64) -> String {
    show(&anim(ws, "timelines", "tl").render_at(tick)).join("/")
}

#[test]
fn phase_reset_at_loop() {
    let ws = timeline_sheet(
        10,
        5,
        1,
        json!([{ "id": "e", "_flipbook": "fb", "coordinates": [0, 0] }]),
        json!([{ "kind": "moveToZ", "_element": "e", "value": 0, "at": 0 }]),
        json!([flipbook("fb", 10, &[("a", 2), ("b", 2), ("c", 2)])]),
    );
    assert_eq!(
        frames(&ws, "timelines", "tl"),
        vec![f("A", 2), f("B", 2), f("C", 1)]
    );
    assert_eq!(shown_at(&ws, 5), "A");
}

#[test]
fn seek_to_a_frame() {
    let ws = seek_sheet(
        json!({ "id": "e", "_flipbook": "fb", "coordinates": [0, 0] }),
        json!([{ "kind": "seekFlipbook", "_element": "e", "frameIndex": 2, "at": 15 }]),
    );
    assert_eq!(shown_at(&ws, 14), "C", "before the seek: 14 mod 4 = 2");
    assert_eq!(shown_at(&ws, 15), "C");
    assert_eq!(shown_at(&ws, 16), "D");
    assert_eq!(shown_at(&ws, 17), "A");
    assert_eq!(state_at(&ws, 15)[0].seek, Some((15, 2)));
}

#[test]
fn restart() {
    let ws = seek_sheet(
        json!({ "id": "e", "_flipbook": "fb", "coordinates": [0, 0] }),
        json!([{ "kind": "seekFlipbook", "_element": "e", "at": 15 }]),
    );
    assert_eq!(shown_at(&ws, 15), "A");
    assert_eq!(shown_at(&ws, 16), "B");
}

#[test]
fn seek_while_hidden() {
    let ws = seek_sheet(
        json!({ "id": "e", "_flipbook": "fb", "coordinates": [0, 0], "visible": false }),
        json!([
            { "kind": "seekFlipbook", "_element": "e", "frameIndex": 1, "at": 4 },
            { "kind": "setVisible", "_element": "e", "value": true, "at": 6 }
        ]),
    );
    assert_eq!(shown_at(&ws, 5), ".");
    assert_eq!(shown_at(&ws, 6), "D", "frame 1 plus 2 frame-units");
}

#[test]
fn seek_ticks_are_change_ticks() {
    let ws = timeline_sheet(
        10,
        10,
        1,
        json!([{ "id": "e", "_flipbook": "fb", "coordinates": [0, 0] }]),
        json!([{ "kind": "seekFlipbook", "_element": "e", "frameIndex": 1, "at": 3 }]),
        json!([flipbook("fb", 10, &[("a", 2), ("b", 2), ("c", 2)])]),
    );
    // Phase from 0: boundaries 2; seek at 3 to B (unit 2): boundaries 3+2=5 (C), 3+4=7 (A), 3+6=9 (B).
    assert_eq!(
        anim(&ws, "timelines", "tl").change_ticks(100),
        Ok(vec![0, 2, 3, 5, 7, 9])
    );
    assert_eq!(
        frames(&ws, "timelines", "tl"),
        vec![f("A", 2), f("B", 3), f("C", 2), f("A", 2), f("B", 1)]
    );
}

// Animated scenes

#[test]
fn layers_loop_independently() {
    let ws = sheet(json!({
        "panels": [
            { "id": "p1", "width": 6, "height": 1, "elements": [{ "id": "hero", "_sprite": "a", "coordinates": [0, 0] }] },
            { "id": "p2", "width": 6, "height": 1, "elements": [{ "id": "hero", "_sprite": "b", "coordinates": [0, 0] }] }
        ],
        "timelines": [
            { "id": "t1", "_panel": "p1", "fps": 2, "duration": 4, "transitions": [move_to([3, 0], 0, 3)] },
            { "id": "t2", "_panel": "p2", "fps": 2, "duration": 6, "transitions": [move_to([5, 0], 0, 5)] }
        ],
        "scenes": [{ "id": "s", "width": 6, "height": 2, "layers": [
            { "_timeline": "t1", "coordinates": [0, 0] }, { "_timeline": "t2", "coordinates": [0, 1] }
        ] }]
    }));
    let a = anim(&ws, "scenes", "s");
    assert_eq!((a.clock, a.loop_len), (2, 12));
    // 4.5 s = tick 9: t1 at 0.5 s (unit 1), t2 at 1.5 s (unit 3).
    assert_eq!(show(&a.render_at(9)), [".A....", "...B.."]);
}

// Composition of animated frames

#[test]
fn frames_use_the_static_rules() {
    let elements = |source: Value| {
        let mut first = json!({ "coordinates": [0, 0], "zIndex": 1 });
        for (k, v) in source.as_object().unwrap() {
            first[k] = v.clone();
        }
        json!([first, { "_sprite": "w", "coordinates": [0, 0] }, { "_sprite": "t", "coordinates": [0, 0], "zIndex": 2 }])
    };
    let ws = sheet(json!({
        "flipbooks": [flipbook("fb", 1, &[("c", 1), ("d", 1)])],
        "panels": [
            { "id": "animated", "width": 1, "height": 1, "elements": elements(json!({ "_flipbook": "fb" })) },
            { "id": "still", "width": 1, "height": 1, "elements": elements(json!({ "_sprite": "d" })) }
        ]
    }));
    assert_eq!(
        anim(&ws, "panels", "animated").render_at(1),
        compose::panel(&ws, "still")
    );
}

#[test]
fn timeline_layer_flattened_as_a_group() {
    let ws = sheet(json!({
        "panels": [
            { "id": "bg", "width": 1, "height": 1, "elements": [{ "_sprite": "a", "coordinates": [0, 0] }] },
            { "id": "fg", "width": 1, "height": 1, "elements": [
                { "id": "e", "_sprite": "w", "coordinates": [0, 0] }, { "_sprite": "t", "coordinates": [0, 0] }
            ] }
        ],
        "timelines": [{ "id": "tl", "_panel": "fg", "fps": 1, "duration": 2,
            "transitions": [{ "kind": "setVisible", "_element": "e", "value": false, "at": 1 }] }],
        "scenes": [{ "id": "s", "width": 1, "height": 1, "layers": [
            { "_panel": "bg", "coordinates": [0, 0] }, { "_timeline": "tl", "coordinates": [0, 0], "zIndex": 1 }
        ] }]
    }));
    let scene = anim(&ws, "scenes", "s");
    let timeline = anim(&ws, "timelines", "tl");
    for tick in 0..2 {
        let mut expected = compose::panel(&ws, "bg");
        compose::draw(&mut expected, &timeline.render_at(tick), 0, 0);
        assert_eq!(scene.render_at(tick), expected, "tick {tick}");
    }
}

#[test]
fn timeline_over_a_static_panel() {
    let ws = timeline_sheet(
        10,
        10,
        1,
        hero([0, 0]),
        json!([{ "kind": "moveToZ", "_element": "hero", "value": 3, "at": 2 }]),
        json!([]),
    );
    assert!(animate::is_animated(&ws, "timelines", "tl"));
    assert_eq!(frames(&ws, "timelines", "tl"), vec![f("A", 10)]);
}

// Frame sequence

#[test]
fn merged_still_frames() {
    let ws = timeline_sheet(
        10,
        10,
        1,
        hero([0, 0]),
        json!([{ "kind": "setVisible", "_element": "hero", "value": false, "at": 5 }]),
        json!([]),
    );
    assert_eq!(frames(&ws, "timelines", "tl").len(), 2);
}

#[test]
fn change_hidden_by_clipping() {
    let ws = timeline_sheet(
        10,
        10,
        1,
        json!([{ "id": "hero", "_sprite": "a", "coordinates": [100, 100], "visible": false }]),
        json!([{ "kind": "setVisible", "_element": "hero", "value": true, "at": 5 }]),
        json!([]),
    );
    assert_eq!(frames(&ws, "timelines", "tl"), vec![f(".", 10)]);
}

#[test]
fn durations_sum_to_the_loop() {
    let ws = coprime_panel();
    let a = anim(&ws, "panels", "x");
    let total: u64 = a.frames().iter().map(|(_, t)| t).sum();
    assert_eq!(total, a.loop_len);
    let ws = layered_scene(json!([
        { "_timeline": "tl", "coordinates": [0, 0] }, { "_panel": "anim", "coordinates": [0, 0] }
    ]));
    let a = anim(&ws, "scenes", "s");
    let total: u64 = a.frames().iter().map(|(_, t)| t).sum();
    assert_eq!(total, a.loop_len);
}
