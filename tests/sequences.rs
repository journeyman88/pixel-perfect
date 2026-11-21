// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 The pixel-perfect Authors

//! Sequences and layer visibility: scenarios of the sequences spec and of the
//! composition and animation changes they bring.

mod common;

use common::load;
use pixel_perfect::ResolvedWorksheet;
use pixel_perfect::animate::{self, Animation, LayerState, sequence_state};
use pixel_perfect::raster::{Image, TRANSPARENT};
use pixel_perfect::{compose, render};
use serde_json::{Value, json};

// Worksheet builder. Palette `p` maps `A`..`H` to opaque colors whose red
// channel is the letter; sprites `a`..`h` are 1x1 of that letter and panels
// `pa`..`ph` are 1x1 panels showing them.

fn sheet(extra: Value) -> ResolvedWorksheet {
    let colors: Vec<Value> = ('A'..='H')
        .map(|c| json!({ "char": c.to_string(), "rgba": [c as u8, 0, 0] }))
        .collect();
    let sprites: Vec<Value> = ('a'..='h')
        .map(|c| json!({ "id": c.to_string(), "_palette": "p", "lines": [c.to_ascii_uppercase().to_string()] }))
        .collect();
    let mut panels: Vec<Value> = ('a'..='h')
        .map(|c| {
            json!({ "id": format!("p{c}"), "width": 1, "height": 1,
                    "elements": [{ "_sprite": c.to_string(), "coordinates": [0, 0] }] })
        })
        .collect();
    if let Some(more) = extra.get("panels") {
        panels.extend(more.as_array().unwrap().iter().cloned());
    }
    let mut doc = json!({ "palettes": [{ "id": "p", "colors": colors }], "sprites": sprites, "panels": panels });
    for (k, v) in extra.as_object().unwrap() {
        if k != "panels" {
            doc[k] = v.clone();
        }
    }
    load(doc)
}

/// A worksheet with scene `s` (`width` x 1, `layers`) and sequence `q` over it.
fn seq_sheet(
    width: u32,
    layers: Value,
    fps: u32,
    duration: u64,
    actions: Value,
    extra: Value,
) -> ResolvedWorksheet {
    let mut extra = extra;
    if extra.is_null() {
        extra = json!({});
    }
    extra["scenes"] = json!([{ "id": "s", "width": width, "height": 1, "layers": layers }]);
    extra["sequences"] =
        json!([{ "id": "q", "_scene": "s", "fps": fps, "duration": duration, "actions": actions }]);
    sheet(extra)
}

fn layer(id: &str, panel: &str, x: i64) -> Value {
    json!({ "id": id, "_panel": panel, "coordinates": [x, 0] })
}

fn show(image: &Image) -> String {
    image
        .pixels()
        .iter()
        .map(|p| {
            if *p == TRANSPARENT {
                '.'
            } else {
                p.0[0] as char
            }
        })
        .collect()
}

fn anim<'a>(ws: &'a ResolvedWorksheet) -> Animation<'a> {
    Animation::new(ws, "sequences", "q").unwrap()
}

fn frames(ws: &ResolvedWorksheet) -> Vec<(String, u64)> {
    anim(ws)
        .frames()
        .iter()
        .map(|(i, t)| (show(i), *t))
        .collect()
}

fn state(ws: &ResolvedWorksheet, t: u64) -> Vec<LayerState> {
    sequence_state(&ws.model.sequences[0], &ws.model.scenes[0], t)
}

/// A 1x1 panel `x` with element `e` (sprite `c`) and timeline `tl` hiding it
/// at frame-unit 1 of `duration` at `fps`.
fn blink(fps: u32, duration: u64) -> Value {
    json!({
        "panels": [{ "id": "x", "width": 1, "height": 1, "elements": [{ "id": "e", "_sprite": "c", "coordinates": [0, 0] }] }],
        "timelines": [{ "id": "tl", "_panel": "x", "fps": fps, "duration": duration,
                        "transitions": [{ "kind": "setVisible", "_element": "e", "value": false, "at": 1 }] }]
    })
}

// ------------------------------------------------------------ layer visibility

#[test]
fn hidden_layer_is_not_drawn() {
    let ws = sheet(
        json!({ "scenes": [{ "id": "s", "width": 1, "height": 1, "layers": [
        { "_panel": "pa", "coordinates": [0, 0] },
        { "id": "light", "_panel": "pb", "coordinates": [0, 0], "zIndex": 1, "visible": false }
    ] }] }),
    );
    assert_eq!(show(&compose::scene(&ws, "s")), "A");
}

#[test]
fn hidden_timeline_layer_makes_the_scene_static() {
    let mut extra = blink(1, 2);
    extra["scenes"] = json!([{ "id": "s", "width": 1, "height": 1, "layers": [
        { "_panel": "pa", "coordinates": [0, 0] },
        { "_timeline": "tl", "coordinates": [0, 0], "zIndex": 1, "visible": false }
    ] }]);
    let ws = sheet(extra);
    assert!(!animate::is_animated(&ws, "scenes", "s"));
    assert_eq!(show(&render::resource(&ws, "scenes", "s")), "A");
}

#[test]
fn hidden_layer_does_not_lengthen_the_scene_loop() {
    // Visible timeline: 2 s; hidden one: 3 s.
    let mut extra = blink(1, 2);
    extra["timelines"].as_array_mut().unwrap().push(
        json!({ "id": "long", "_panel": "x", "fps": 1, "duration": 3,
        "transitions": [{ "kind": "setVisible", "_element": "e", "value": false, "at": 1 }] }),
    );
    extra["scenes"] = json!([{ "id": "s", "width": 1, "height": 1, "layers": [
        { "_timeline": "tl", "coordinates": [0, 0] },
        { "_timeline": "long", "coordinates": [0, 0], "visible": false }
    ] }]);
    let ws = sheet(extra);
    let a = Animation::new(&ws, "scenes", "s").unwrap();
    assert_eq!(a.loop_len / a.clock, 2);
}

// ------------------------------------------------------------- layer state

#[test]
fn loop_restarts_from_the_scene() {
    let ws = seq_sheet(
        1,
        json!([layer("lamp", "pa", 0)]),
        1,
        8,
        json!([{ "kind": "setVisible", "_layer": "lamp", "value": false, "at": 4 }]),
        Value::Null,
    );
    assert!(state(&ws, 3)[0].visible);
    assert!(!state(&ws, 4)[0].visible);
    assert_eq!(frames(&ws), vec![("A".into(), 4), (".".into(), 4)]);
}

#[test]
fn untouched_layers_keep_their_state() {
    let ws = seq_sheet(
        2,
        json!([layer("sky", "pa", 0), layer("lamp", "pb", 1)]),
        1,
        4,
        json!([{ "kind": "setVisible", "_layer": "lamp", "value": false, "at": 2 }]),
        Value::Null,
    );
    for t in 0..4 {
        assert_eq!(
            state(&ws, t)[0],
            LayerState {
                coordinates: [0, 0],
                z_index: 0,
                visible: true
            }
        );
    }
}

#[test]
fn light_switches_on_at_its_frame_unit() {
    let mut light = layer("light", "pb", 0);
    light["visible"] = json!(false);
    let ws = seq_sheet(
        1,
        json!([light]),
        1,
        10,
        json!([{ "kind": "setVisible", "_layer": "light", "value": true, "at": 6 }]),
        Value::Null,
    );
    assert!(!state(&ws, 5)[0].visible);
    assert!(state(&ws, 6)[0].visible);
}

#[test]
fn same_at_follows_array_order() {
    let ws = seq_sheet(
        1,
        json!([layer("fog", "pa", 0)]),
        1,
        8,
        json!([
            { "kind": "moveToZ", "_layer": "fog", "value": 5, "at": 3 },
            { "kind": "moveToZ", "_layer": "fog", "value": 1, "at": 3 }
        ]),
        Value::Null,
    );
    assert_eq!(state(&ws, 2)[0].z_index, 0);
    assert_eq!(state(&ws, 3)[0].z_index, 1);
}

#[test]
fn moving_in_front_of_another_layer() {
    let mut tree = layer("tree", "pb", 0);
    tree["zIndex"] = json!(5);
    let ws = seq_sheet(
        1,
        json!([layer("light", "pa", 0), tree]),
        1,
        4,
        json!([{ "kind": "moveToZ", "_layer": "light", "value": 10, "at": 2 }]),
        Value::Null,
    );
    assert_eq!(frames(&ws), vec![("B".into(), 2), ("A".into(), 2)]);
}

#[test]
fn parallax_planes() {
    let ws = seq_sheet(
        32,
        json!([layer("far", "pa", 0), layer("near", "pb", 0)]),
        4,
        16,
        json!([
            { "kind": "moveTo", "_layer": "far", "coordinates": [-4, 0], "start": 0, "duration": 16 },
            { "kind": "moveTo", "_layer": "near", "coordinates": [-16, 0], "start": 0, "duration": 16 }
        ]),
        Value::Null,
    );
    let s = state(&ws, 8);
    assert_eq!(s[0].coordinates, [-2, 0]);
    assert_eq!(s[1].coordinates, [-8, 0]);
}

#[test]
fn chained_layer_movements_and_clipping() {
    let ws = seq_sheet(
        8,
        json!([layer("l", "pa", 0)]),
        1,
        10,
        json!([
            { "kind": "moveTo", "_layer": "l", "coordinates": [4, 0], "start": 0, "duration": 4 },
            { "kind": "moveTo", "_layer": "l", "coordinates": [12, 0], "start": 6, "duration": 2 }
        ]),
        Value::Null,
    );
    let x = |t| state(&ws, t)[0].coordinates[0];
    assert_eq!(
        [x(0), x(2), x(4), x(6), x(7), x(8), x(9)],
        [0, 2, 4, 4, 8, 12, 12]
    );
    // At x = 12 the layer is outside the 8-pixel canvas.
    let a = anim(&ws);
    assert_eq!(show(&a.render_at(8 * a.clock)), "........");
}

// ----------------------------------------------------------- clock and loop

#[test]
fn clock_covers_the_sequence_and_its_layers() {
    let mut extra = blink(6, 2);
    extra["scenes"] = Value::Null;
    let ws = seq_sheet(
        1,
        json!([{ "id": "fire", "_timeline": "tl", "coordinates": [0, 0] }]),
        4,
        4,
        json!([{ "kind": "setVisible", "_layer": "fire", "value": false, "at": 3 }]),
        extra,
    );
    assert_eq!(anim(&ws).clock, 12);
}

#[test]
fn loop_is_the_lcm_with_animated_layers() {
    // Sequence: 4 s; timeline layer: 3 frame-units at 2 fps = 1.5 s.
    let ws = seq_sheet(
        1,
        json!([{ "id": "fire", "_timeline": "tl", "coordinates": [0, 0] }]),
        1,
        4,
        json!([{ "kind": "moveToZ", "_layer": "fire", "value": 1, "at": 1 }]),
        blink(2, 3),
    );
    let a = anim(&ws);
    assert_eq!((a.loop_len, a.clock), (24, 2), "12 s");
    let total: u64 = a.frames().iter().map(|f| f.1).sum();
    assert_eq!(total, a.loop_len);
}

#[test]
fn layer_that_is_never_drawn_is_ignored() {
    let mut fire =
        json!({ "id": "fire", "_timeline": "tl", "coordinates": [0, 0], "visible": false });
    let ws = seq_sheet(
        1,
        json!([layer("sky", "pa", 0), fire.clone()]),
        1,
        4,
        json!([{ "kind": "moveToZ", "_layer": "fire", "value": 1, "at": 1 }]),
        blink(2, 3),
    );
    let a = anim(&ws);
    assert_eq!((a.loop_len, a.clock), (4, 1));
    // Shown by a setVisible: it counts again.
    fire["visible"] = json!(false);
    let ws = seq_sheet(
        1,
        json!([layer("sky", "pa", 0), fire]),
        1,
        4,
        json!([{ "kind": "setVisible", "_layer": "fire", "value": true, "at": 1 }]),
        blink(2, 3),
    );
    assert_eq!(anim(&ws).loop_len / anim(&ws).clock, 12);
}

#[test]
fn animated_layers_keep_their_rhythm() {
    // Timeline: 3 frame-units at 4 fps (0.75 s), element visible only in the
    // first frame-unit. Sequence: 4 s at 4 fps, moving the layer.
    let ws = seq_sheet(
        2,
        json!([{ "id": "lamp", "_timeline": "tl", "coordinates": [0, 0] }]),
        4,
        16,
        json!([{ "kind": "moveTo", "_layer": "lamp", "coordinates": [1, 0], "start": 0, "duration": 16 }]),
        blink(4, 3),
    );
    let a = anim(&ws);
    assert_eq!(a.clock, 4);
    assert_eq!(a.loop_len, 48, "12 s");
    // 4.5 s: sequence at 0.5 s (lamp at x = 0), timeline at 0 s (visible).
    assert_eq!(show(&a.render_at(18)), "C.");
    // 4.75 s: timeline at 0.25 s (hidden).
    assert_eq!(show(&a.render_at(19)), "..");
    // 6 s: sequence at 2 s (x = 1 after rounding 0.5 away), timeline at 0 s.
    assert_eq!(show(&a.render_at(24)), ".C");
}

#[test]
fn shown_layer_plays_its_global_time() {
    // Animated panel: flipbook a, b (1 s each). Hidden until 1 s.
    let extra = json!({
        "flipbooks": [{ "id": "fb", "width": 1, "height": 1, "fps": 1, "frames": [
            { "_sprite": "a", "coordinates": [0, 0], "duration": 1 },
            { "_sprite": "b", "coordinates": [0, 0], "duration": 1 }
        ] }],
        "panels": [{ "id": "anim", "width": 1, "height": 1, "elements": [{ "_flipbook": "fb", "coordinates": [0, 0] }] }]
    });
    let ws = seq_sheet(
        1,
        json!([{ "id": "l", "_panel": "anim", "coordinates": [0, 0], "visible": false }]),
        1,
        4,
        json!([{ "kind": "setVisible", "_layer": "l", "value": true, "at": 1 }]),
        extra,
    );
    let a = anim(&ws);
    assert_eq!(show(&a.render_at(0)), ".");
    assert_eq!(show(&a.render_at(1)), "B", "global time 1 s, not 0 s");
}

#[test]
fn single_switch_gives_two_frames() {
    let ws = seq_sheet(
        1,
        json!([layer("lamp", "pa", 0)]),
        4,
        8,
        json!([{ "kind": "setVisible", "_layer": "lamp", "value": false, "at": 4 }]),
        Value::Null,
    );
    assert_eq!(anim(&ws).clock, 4);
    assert_eq!(frames(&ws), vec![("A".into(), 4), (".".into(), 4)]);
}
