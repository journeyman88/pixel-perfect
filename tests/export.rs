// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 The pixel-perfect Authors

//! Sprite rasterization, export selection, planning, safe build and manifest.

mod common;
#[path = "common/images.rs"]
mod images;

use std::path::Path;
use std::process::Command;

use common::assert_error;
use pixel_perfect::encode::{self, Encoding};
use pixel_perfect::export::{self, OutputFormat, RenderOptions};
use pixel_perfect::model::Rgba;
use pixel_perfect::raster::TRANSPARENT;
use pixel_perfect::{Format, ResolvedWorksheet, render};
use serde_json::{Value, json};

/// A worksheet with one palette (`r` red, `g` half-green, `x` transparent red)
/// plus the given sprites and exports.
fn worksheet(sprites: Value, exports: Option<Value>) -> ResolvedWorksheet {
    let mut doc = json!({
        "palettes": [
            { "id": "p", "colors": [
                { "char": "r", "rgba": [255, 0, 0] },
                { "char": "g", "rgba": [0, 255, 0, 0.5] },
                { "char": "x", "rgba": [200, 10, 10, 0.0] }
            ] },
            { "id": "dark", "_palette": "p", "colors": [{ "char": "r", "rgba": [90, 0, 0] }] }
        ],
        "sprites": sprites,
    });
    if let Some(e) = exports {
        doc["exports"] = e;
    }
    pixel_perfect::load_value(doc).unwrap_or_else(|e| panic!("{:#?}", e.diagnostics()))
}

fn sprite(id: &str) -> Value {
    json!({ "id": id, "_palette": "p", "lines": ["r.g", "grr"] })
}

fn options(dir: &Path) -> RenderOptions {
    RenderOptions {
        output_path: dir.to_owned(),
        format: OutputFormat::Png,
        scale: 1,
        safe_build: false,
        single_frames: false,
        max_image_size: Some(export::DEFAULT_MAX_IMAGE_SIZE),
    }
}

fn files(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for entry in walk(dir) {
        out.push(
            entry
                .strip_prefix(dir)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/"),
        );
    }
    out.sort();
    out
}

fn walk(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries {
            let p = e.unwrap().path();
            if p.is_dir() {
                out.extend(walk(&p))
            } else {
                out.push(p)
            }
        }
    }
    out
}

fn plan_errors(ws: &ResolvedWorksheet, opts: &RenderOptions) -> Vec<pixel_perfect::Diagnostic> {
    export::plan(ws, opts).expect_err("plan should fail")
}

// Rasterization

#[test]
fn palette_lookup_with_alpha() {
    let ws = worksheet(json!([sprite("s")]), None);
    let image = render::sprite(&ws, "s");
    assert_eq!(image.get(0, 0), Rgba([255, 0, 0, 255]));
    assert_eq!(image.get(1, 0), TRANSPARENT);
    assert_eq!(image.get(2, 0), Rgba([0, 255, 0, 128]));
}

#[test]
fn derived_sprite_uses_its_own_palette() {
    let ws = worksheet(
        json!([sprite("s"), { "id": "d", "_sprite": "s", "_palette": "dark" }]),
        None,
    );
    assert_eq!(render::sprite(&ws, "d").get(0, 0), Rgba([90, 0, 0, 255]));
}

#[test]
fn transparent_palette_color_is_canonical() {
    let ws = worksheet(
        json!([{ "id": "t", "_palette": "p", "lines": ["x."] }]),
        None,
    );
    let image = render::sprite(&ws, "t");
    assert_eq!(image.pixels(), [TRANSPARENT, TRANSPARENT]);
}

// Selection and paths

#[test]
fn nested_output_paths() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("build/art");
    let ws = worksheet(json!([sprite("hero"), sprite("coin")]), None);
    export::render(&ws, &options(&out)).unwrap();
    assert_eq!(
        files(&out),
        ["manifest.json", "sprites/coin.png", "sprites/hero.png"]
    );
}

#[test]
fn explicit_exports_select_resources() {
    let dir = tempfile::tempdir().unwrap();
    let ws = worksheet(
        json!([sprite("hero"), sprite("coin")]),
        Some(json!(["/sprites/hero"])),
    );
    export::render(&ws, &options(dir.path())).unwrap();
    assert_eq!(files(dir.path()), ["manifest.json", "sprites/hero.png"]);
}

/// Sprites `a` (red) and `g` (half green), a static panel `stage`, an
/// animated panel `moving`, a flipbook, a timeline, a static scene `day` and an
/// animated scene `night`.
fn compositions(exports: Option<Value>) -> ResolvedWorksheet {
    let mut doc = json!({
        "palettes": [{ "id": "p", "colors": [
            { "char": "r", "rgba": [255, 0, 0] },
            { "char": "g", "rgba": [0, 255, 0, 0.5] }
        ] }],
        "sprites": [
            { "id": "a", "_palette": "p", "lines": ["rr"] },
            { "id": "g", "_palette": "p", "lines": ["g"] }
        ],
        "flipbooks": [{ "id": "spin", "width": 2, "height": 1, "fps": 1,
            "frames": [{ "_sprite": "a", "coordinates": [0, 0], "duration": 1 }] }],
        "panels": [
            { "id": "stage", "width": 3, "height": 1, "elements": [
                { "id": "base", "_sprite": "a", "coordinates": [0, 0] },
                { "_sprite": "g", "coordinates": [0, 0] },
                { "_flipbook": "spin", "coordinates": [0, 0], "visible": false }
            ] },
            { "id": "moving", "width": 2, "height": 1, "elements": [
                { "_flipbook": "spin", "coordinates": [0, 0] }
            ] }
        ],
        "timelines": [{ "id": "walk", "_panel": "stage", "fps": 1, "duration": 2,
            "transitions": [{ "kind": "setVisible", "_element": "base", "value": false, "at": 1 }] }],
        "scenes": [
            { "id": "day", "width": 4, "height": 2, "layers": [{ "_panel": "stage", "coordinates": [1, 1] }] },
            { "id": "night", "width": 4, "height": 2, "layers": [{ "_timeline": "walk", "coordinates": [0, 0] }] }
        ]
    });
    if let Some(e) = exports {
        doc["exports"] = e;
    }
    common::load(doc)
}

#[test]
fn implicit_exports_include_animations() {
    let ws = compositions(None);
    let dir = tempfile::tempdir().unwrap();
    let (plan, manifest) = export::render(&ws, &options(dir.path())).unwrap();
    assert_eq!(
        files(dir.path()),
        [
            "flipbooks/spin.png",
            "manifest.json",
            "panels/moving.png",
            "panels/stage.png",
            "scenes/day.png",
            "scenes/night.png",
            "sprites/a.png",
            "sprites/g.png",
            "timelines/walk.png",
        ]
    );
    assert!(plan.notices.is_empty());
    for (pointer, animated) in [
        ("/sprites/a", false),
        ("/panels/stage", false),
        ("/scenes/day", false),
        ("/flipbooks/spin", true),
        ("/panels/moving", true),
        ("/timelines/walk", true),
        ("/scenes/night", true),
    ] {
        let path = dir
            .path()
            .join(manifest.entry(pointer).unwrap().path.as_ref().unwrap());
        let bytes = std::fs::read(path).unwrap();
        let has_actl = images::chunks(&bytes).iter().any(|c| c == "acTL");
        assert_eq!(has_actl, animated, "{pointer}");
        assert_eq!(
            manifest.entry(pointer).unwrap().frame_count.is_some(),
            animated,
            "{pointer}"
        );
    }
}

#[test]
fn static_panel_and_scene_are_exported() {
    let ws = compositions(Some(json!(["/panels/stage", "/scenes/day"])));
    let dir = tempfile::tempdir().unwrap();
    let (_, manifest) = export::render(&ws, &options(dir.path())).unwrap();
    assert_eq!(
        files(dir.path()),
        ["manifest.json", "panels/stage.png", "scenes/day.png"]
    );
    let stage = manifest.entry("/panels/stage").unwrap();
    assert_eq!((stage.width, stage.height), (3, 1));
    let day = manifest.entry("/scenes/day").unwrap();
    assert_eq!((day.width, day.height), (4, 2));
    let (_, _, pixels) = images::decode(&std::fs::read(dir.path().join("scenes/day.png")).unwrap());
    let mixed = Rgba([127, 128, 0, 255]);
    let red = Rgba([255, 0, 0, 255]);
    let t = TRANSPARENT;
    assert_eq!(pixels, [t, t, t, t, t, mixed, red, t]);
}

#[test]
fn animated_resources_are_written_as_apng() {
    let ws = compositions(Some(json!([
        "/flipbooks/spin",
        "/timelines/walk",
        "/scenes/night"
    ])));
    let dir = tempfile::tempdir().unwrap();
    let (_, manifest) = export::render(&ws, &options(dir.path())).unwrap();
    assert_eq!(
        files(dir.path()),
        [
            "flipbooks/spin.png",
            "manifest.json",
            "scenes/night.png",
            "timelines/walk.png"
        ]
    );
    let size = |p: &str| {
        (
            manifest.entry(p).unwrap().width,
            manifest.entry(p).unwrap().height,
        )
    };
    assert_eq!(size("/flipbooks/spin"), (2, 1));
    assert_eq!(size("/timelines/walk"), (3, 1));
    assert_eq!(size("/scenes/night"), (4, 2));
    // walk: `base` (red) under half green, then hidden at frame-unit 1.
    let bytes = std::fs::read(dir.path().join("timelines/walk.png")).unwrap();
    let (frames, plays) = images::decode_apng(&bytes);
    assert_eq!(plays, 0);
    let mixed = Rgba([127, 128, 0, 255]);
    let red = Rgba([255, 0, 0, 255]);
    let green = Rgba([0, 255, 0, 128]);
    assert_eq!(frames.len(), 2);
    assert_eq!(frames[0].pixels, [mixed, red, TRANSPARENT]);
    assert_eq!(frames[1].pixels, [green, TRANSPARENT, TRANSPARENT]);
    assert_eq!((frames[0].delay, frames[1].delay), ((1, 1), (1, 1)));
}

#[test]
fn animated_entry_in_the_manifest() {
    let sprites: Vec<Value> = ["a", "b", "c", "d"]
        .iter()
        .map(|c| json!({ "id": c, "_palette": "p", "lines": vec![c.repeat(8); 8] }))
        .collect();
    let doc = json!({
        "palettes": [{ "id": "p", "colors": [
            { "char": "a", "rgba": [1, 0, 0] }, { "char": "b", "rgba": [2, 0, 0] },
            { "char": "c", "rgba": [3, 0, 0] }, { "char": "d", "rgba": [4, 0, 0] }
        ] }],
        "sprites": sprites,
        "flipbooks": [{ "id": "spin", "width": 8, "height": 8, "fps": 12, "frames": [
            { "_sprite": "a", "coordinates": [0, 0], "duration": 2 },
            { "_sprite": "b", "coordinates": [0, 0], "duration": 2 },
            { "_sprite": "c", "coordinates": [0, 0], "duration": 1 },
            { "_sprite": "d", "coordinates": [0, 0], "duration": 1 }
        ] }],
        "exports": ["/flipbooks/spin"]
    });
    let ws = common::load(doc);
    let dir = tempfile::tempdir().unwrap();
    export::render(&ws, &options(dir.path())).unwrap();
    let manifest: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.path().join("manifest.json")).unwrap())
            .unwrap();
    assert_eq!(
        manifest["files"],
        json!([{
            "id": "/flipbooks/spin", "kind": "animation", "path": "flipbooks/spin.png",
            "width": 8, "height": 8, "frameCount": 4, "loop": { "num": 1, "den": 2 }
        }])
    );
    common::assert_valid_manifest(&manifest);
}

#[test]
fn manifest_schema_is_valid_draft_2020_12() {
    jsonschema::meta::validate(&common::manifest_schema()).expect("valid against the meta-schema");
}

#[test]
fn manifest_entries_are_sorted_by_id() {
    let mut doc = json!({
        "palettes": [{ "id": "p", "colors": [
            { "char": "a", "rgba": [1, 0, 0] }, { "char": "b", "rgba": [2, 0, 0] }
        ] }],
        "sprites": [
            { "id": "hero", "_palette": "p", "lines": ["a"] },
            { "id": "coin", "_palette": "p", "lines": ["b"] }
        ],
        "flipbooks": [{ "id": "spin", "width": 1, "height": 1, "fps": 2, "frames": [
            { "_sprite": "hero", "coordinates": [0, 0], "duration": 1 },
            { "_sprite": "coin", "coordinates": [0, 0], "duration": 1 }
        ] }]
    });
    doc["exports"] = json!(["/sprites/hero", "/sprites/coin", "/flipbooks/spin"]);
    let ws = common::load(doc);
    let dir = tempfile::tempdir().unwrap();
    export::render(&ws, &options(dir.path())).unwrap();
    let manifest: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.path().join("manifest.json")).unwrap())
            .unwrap();
    let ids: Vec<&str> = manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["/flipbooks/spin", "/sprites/coin", "/sprites/hero"]);
    common::assert_valid_manifest(&manifest);
}

#[test]
fn manifest_with_nothing_to_export() {
    let ws = pixel_perfect::load_file(&common::fixture("valid/palettes-only.json")).unwrap();
    let dir = tempfile::tempdir().unwrap();
    export::render(&ws, &options(dir.path())).unwrap();
    let manifest: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.path().join("manifest.json")).unwrap())
            .unwrap();
    assert_eq!(manifest["files"], json!([]));
    common::assert_valid_manifest(&manifest);
}

#[test]
fn rendered_apng_round_trip_with_scale() {
    let ws = compositions(Some(json!(["/scenes/night"])));
    let dir = tempfile::tempdir().unwrap();
    let mut opts = options(dir.path());
    opts.scale = 3;
    export::render(&ws, &opts).unwrap();
    let bytes = std::fs::read(dir.path().join("scenes/night.png")).unwrap();
    let (decoded, _) = images::decode_apng(&bytes);
    let animation = pixel_perfect::animate::Animation::new(&ws, "scenes", "night").unwrap();
    let expected = animation.frames();
    assert_eq!(decoded.len(), expected.len());
    for (frame, (image, ticks)) in decoded.iter().zip(&expected) {
        assert_eq!(frame.pixels, image.upscale(3).pixels());
        let (num, den) = pixel_perfect::time::reduced(*ticks, animation.clock);
        assert_eq!(frame.delay, (num as u16, den as u16));
    }
}

#[test]
fn repeated_render_is_byte_identical() {
    let ws = compositions(None);
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    export::render(&ws, &options(a.path())).unwrap();
    export::render(&ws, &options(b.path())).unwrap();
    for file in files(a.path()) {
        assert_eq!(
            std::fs::read(a.path().join(&file)).unwrap(),
            std::fs::read(b.path().join(&file)).unwrap(),
            "{file}"
        );
    }
}

/// The apng-export scenario "Denominator too large": some change ticks are 1
/// tick apart on a 716539 ticks-per-second clock.
fn denominator_scene() -> ResolvedWorksheet {
    let doc = json!({
        "palettes": [{ "id": "p", "colors": [{ "char": "a", "rgba": [1, 0, 0] }, { "char": "b", "rgba": [2, 0, 0] }] }],
        "sprites": [{ "id": "a", "_palette": "p", "lines": ["a"] }, { "id": "b", "_palette": "p", "lines": ["b"] }],
        "flipbooks": [
            { "id": "f89", "width": 1, "height": 1, "fps": 89, "frames": [
                { "_sprite": "a", "coordinates": [0, 0], "duration": 1 }, { "_sprite": "b", "coordinates": [0, 0], "duration": 1 }] },
            { "id": "f83", "width": 1, "height": 1, "fps": 83, "frames": [
                { "_sprite": "a", "coordinates": [0, 0], "duration": 1 }, { "_sprite": "b", "coordinates": [0, 0], "duration": 1 }] }
        ],
        "panels": [
            { "id": "t", "width": 1, "height": 1, "elements": [{ "id": "e", "_flipbook": "f89", "coordinates": [0, 0] }] },
            { "id": "p", "width": 1, "height": 1, "elements": [{ "_flipbook": "f83", "coordinates": [0, 0] }] }
        ],
        "timelines": [{ "id": "tl", "_panel": "t", "fps": 97, "duration": 97,
            "transitions": [{ "kind": "seekFlipbook", "_element": "e", "at": 1 }] }],
        "scenes": [{ "id": "s", "width": 1, "height": 1, "layers": [
            { "_timeline": "tl", "coordinates": [0, 0] }, { "_panel": "p", "coordinates": [0, 0] }
        ] }],
        "exports": ["/scenes/s"]
    });
    common::load(doc)
}

#[test]
fn delay_denominator_too_large_is_a_plan_error() {
    let ws = denominator_scene();
    let dir = tempfile::tempdir().unwrap();
    let errors = plan_errors(&ws, &options(dir.path()));
    assert_error(&errors, "/scenes/0", "combining the rates 83, 89, 97 fps");
    assert!(files(dir.path()).is_empty());
}

#[test]
fn same_id_in_different_collections_does_not_conflict() {
    let doc = json!({
        "palettes": [{ "id": "p", "colors": [{ "char": "r", "rgba": [255, 0, 0] }] }],
        "sprites": [{ "id": "hero", "_palette": "p", "lines": ["r"] }],
        "panels": [{ "id": "hero", "width": 2, "height": 2,
            "elements": [{ "_sprite": "hero", "coordinates": [1, 1] }] }]
    });
    let ws = common::load(doc);
    let dir = tempfile::tempdir().unwrap();
    export::render(&ws, &options(dir.path())).unwrap();
    assert_eq!(
        files(dir.path()),
        ["manifest.json", "panels/hero.png", "sprites/hero.png"]
    );
}

#[test]
fn composited_panel_gets_the_expected_palette() {
    // Half green over red gives a new opaque color, (127, 128, 0).
    let ws = compositions(Some(json!(["/panels/stage"])));
    let image = export::image(
        &ws,
        &export::plan(&ws, &options(Path::new("out")))
            .unwrap()
            .outputs[0],
        1,
    );
    assert_eq!(
        encode::choose(&image),
        Encoding::Indexed {
            palette: vec![
                TRANSPARENT,
                Rgba([127, 128, 0, 255]),
                Rgba([255, 0, 0, 255])
            ],
            transparent: 1,
            bit_depth: 2,
        }
    );
}

#[test]
fn many_blended_colors_give_rgba() {
    // 17 opaque reds by column under 17 half-transparent greens by row:
    // 289 distinct blended colors from 34 palette entries.
    let reds: Vec<char> = ('a'..='q').collect();
    let greens: Vec<char> = ('A'..='Q').collect();
    let mut colors = Vec::new();
    for (i, c) in reds.iter().enumerate() {
        colors.push(json!({ "char": c.to_string(), "rgba": [i * 15, 0, 0] }));
    }
    for (i, c) in greens.iter().enumerate() {
        colors.push(json!({ "char": c.to_string(), "rgba": [0, i * 15, 0, 0.5] }));
    }
    let row: String = reds.iter().collect();
    let columns: Vec<String> = greens.iter().map(|c| c.to_string().repeat(17)).collect();
    let doc = json!({
        "palettes": [{ "id": "p", "colors": colors }],
        "sprites": [
            { "id": "reds", "_palette": "p", "lines": vec![row; 17] },
            { "id": "greens", "_palette": "p", "lines": columns }
        ],
        "panels": [{ "id": "mix", "width": 17, "height": 17, "elements": [
            { "_sprite": "reds", "coordinates": [0, 0] },
            { "_sprite": "greens", "coordinates": [0, 0] }
        ] }],
        "exports": ["/panels/mix"]
    });
    let ws = common::load(doc);
    let dir = tempfile::tempdir().unwrap();
    export::render(&ws, &options(dir.path())).unwrap();
    let png = std::fs::read(dir.path().join("panels/mix.png")).unwrap();
    assert_eq!(images::header(&png), (17, 17, 8, 6));
}

#[test]
fn existing_files_are_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("sprites")).unwrap();
    std::fs::write(dir.path().join("sprites/hero.png"), b"old").unwrap();
    std::fs::write(dir.path().join("sprites/orphan.png"), b"old").unwrap();
    let ws = worksheet(json!([sprite("hero")]), None);
    export::render(&ws, &options(dir.path())).unwrap();
    assert_ne!(
        std::fs::read(dir.path().join("sprites/hero.png")).unwrap(),
        b"old"
    );
    assert!(
        dir.path().join("sprites/orphan.png").exists(),
        "files not produced are kept"
    );
}

// File-safe ids

#[test]
fn forbidden_ids_are_rejected_with_a_hint() {
    for id in [
        "ui/button",
        "a\\b",
        "c:d",
        "e*",
        "f?",
        "g\"",
        "h<",
        "i>",
        "j|",
        "k\u{1}",
        ".",
        "..",
        "dot.",
        "space ",
    ] {
        let ws = worksheet(json!([sprite(id)]), None);
        let e = plan_errors(&ws, &options(Path::new("out")));
        assert_error(&e, "/sprites/0/id", "--safe-build");
    }
}

#[test]
fn case_only_difference_is_rejected() {
    let ws = worksheet(json!([sprite("Hero"), sprite("hero")]), None);
    let e = plan_errors(&ws, &options(Path::new("out")));
    assert_error(
        &e,
        "/sprites/1/id",
        "differs only by letter case from \"Hero\"",
    );
}

#[test]
fn nothing_is_written_when_the_plan_fails() {
    let dir = tempfile::tempdir().unwrap();
    let ws = worksheet(json!([sprite("fine"), sprite("ui/button")]), None);
    assert!(export::render(&ws, &options(dir.path())).is_err());
    assert!(files(dir.path()).is_empty());
}

// Safe build

#[test]
fn id_hash_matches_b2sum() {
    assert_eq!(export::id_hash("hero"), "8737b11b474b218b");
    assert_eq!(export::id_hash("Hero"), "b46e11bab0d34796");
    // Cross-check more ids against coreutils when it is available.
    for id in ["base", "ui/button", "é", "a b"] {
        let Ok(out) = Command::new("sh")
            .arg("-c")
            .arg("printf '%s' \"$1\" | b2sum -l 64")
            .arg("sh")
            .arg(id)
            .output()
        else {
            return;
        };
        if !out.status.success() {
            return;
        }
        let expected = String::from_utf8(out.stdout).unwrap();
        assert_eq!(
            export::id_hash(id),
            expected.split_whitespace().next().unwrap(),
            "{id}"
        );
    }
}

#[test]
fn safe_build_names_by_hash() {
    let dir = tempfile::tempdir().unwrap();
    let ws = worksheet(
        json!([sprite("hero"), sprite("Hero"), sprite("ui/button")]),
        None,
    );
    let opts = RenderOptions {
        safe_build: true,
        ..options(dir.path())
    };
    let (_, manifest) = export::render(&ws, &opts).unwrap();
    let hashed = files(dir.path());
    assert!(
        hashed.contains(&"sprites/8737b11b474b218b.png".to_owned()),
        "{hashed:?}"
    );
    assert!(
        hashed.contains(&"sprites/b46e11bab0d34796.png".to_owned()),
        "{hashed:?}"
    );
    assert_eq!(
        manifest
            .entry("/sprites/ui/button")
            .unwrap()
            .path
            .as_deref()
            .unwrap(),
        format!("sprites/{}.png", export::id_hash("ui/button"))
    );
}

#[test]
fn safe_build_name_is_stable_when_pixels_change() {
    let opts = RenderOptions {
        safe_build: true,
        ..options(Path::new("out"))
    };
    let before = worksheet(json!([sprite("hero")]), None);
    let after = worksheet(
        json!([{ "id": "hero", "_palette": "p", "lines": ["ggg", "..."] }]),
        None,
    );
    assert_eq!(
        export::plan(&before, &opts).unwrap().outputs[0].path,
        export::plan(&after, &opts).unwrap().outputs[0].path
    );
}

// Manifest and scale

#[test]
fn manifest_content() {
    let dir = tempfile::tempdir().unwrap();
    let ws = worksheet(json!([sprite("hero")]), None);
    let opts = RenderOptions {
        scale: 2,
        ..options(dir.path())
    };
    export::render(&ws, &opts).unwrap();
    let text = std::fs::read_to_string(dir.path().join("manifest.json")).unwrap();
    assert!(text.ends_with("}\n"));
    let manifest: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        manifest,
        json!({
            "options": { "format": "png", "scale": 2, "safeBuild": false, "singleFrames": false },
            "files": [
                { "id": "/sprites/hero", "kind": "static", "path": "sprites/hero.png", "width": 6, "height": 4 }
            ]
        })
    );
    common::assert_valid_manifest(&manifest);
    let (w, h, _) = images::decode(&std::fs::read(dir.path().join("sprites/hero.png")).unwrap());
    assert_eq!((w, h), (6, 4));
}

#[test]
fn manifest_in_safe_build() {
    let ws = worksheet(json!([sprite("hero")]), None);
    let opts = RenderOptions {
        safe_build: true,
        ..options(Path::new("out"))
    };
    let plan = export::plan(&ws, &opts).unwrap();
    let manifest = export::manifest(&plan, &opts);
    assert!(manifest.options.safe_build);
    assert_eq!(
        manifest
            .entry("/sprites/hero")
            .unwrap()
            .path
            .as_deref()
            .unwrap(),
        "sprites/8737b11b474b218b.png"
    );
}

#[test]
fn renders_are_byte_identical() {
    let ws = worksheet(json!([sprite("hero"), sprite("Hero")]), None);
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    for safe_build in [false, true] {
        if !safe_build {
            // Case collision only matters without safe build; drop `Hero`.
            let ws = worksheet(json!([sprite("hero")]), None);
            export::render(&ws, &options(a.path())).unwrap();
            export::render(&ws, &options(b.path())).unwrap();
        } else {
            let opts = |d: &Path| RenderOptions {
                safe_build: true,
                ..options(d)
            };
            export::render(&ws, &opts(a.path())).unwrap();
            export::render(&ws, &opts(b.path())).unwrap();
        }
        for f in files(a.path()) {
            assert_eq!(
                std::fs::read(a.path().join(&f)).unwrap(),
                std::fs::read(b.path().join(&f)).unwrap(),
                "{f}"
            );
        }
    }
}

#[test]
fn written_png_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let ws = worksheet(json!([sprite("hero")]), None);
    export::render(&ws, &options(dir.path())).unwrap();
    let (_, _, pixels) =
        images::decode(&std::fs::read(dir.path().join("sprites/hero.png")).unwrap());
    assert_eq!(pixels, render::sprite(&ws, "hero").pixels());
}

#[test]
fn yaml_worksheets_render_too() {
    let ws = pixel_perfect::load_str(
        "palettes: [{ id: p, colors: [{ char: r, rgba: [1, 2, 3] }] }]\nsprites: [{ id: s, _palette: p, lines: [r] }]\n",
        Format::Yaml,
    )
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    export::render(&ws, &options(dir.path())).unwrap();
    assert!(dir.path().join("sprites/s.png").exists());
}

/// Every file under `dir`, by relative path, with its bytes.
fn contents(dir: &Path) -> Vec<(String, Vec<u8>)> {
    files(dir)
        .into_iter()
        .map(|f| {
            let bytes = std::fs::read(dir.join(&f)).unwrap();
            (f, bytes)
        })
        .collect()
}

#[test]
fn descriptions_do_not_change_the_output() {
    let annotated = pixel_perfect::load_value(common::descriptions()).unwrap();
    let mut doc = common::descriptions();
    common::strip_descriptions(&mut doc);
    let stripped = pixel_perfect::load_value(doc).unwrap();

    for safe_build in [false, true] {
        let with = tempfile::tempdir().unwrap();
        let without = tempfile::tempdir().unwrap();
        for (ws, dir) in [(&annotated, &with), (&stripped, &without)] {
            let opts = RenderOptions {
                safe_build,
                ..options(dir.path())
            };
            export::render(ws, &opts).unwrap();
        }
        let written = contents(with.path());
        assert!(
            written.iter().any(|(f, _)| f == "manifest.json"),
            "{:?}",
            files(with.path())
        );
        assert!(written.len() > 1, "only the manifest was written");
        assert!(
            written == contents(without.path()),
            "output differs (safe_build: {safe_build})"
        );
    }
}

// WebP

fn webp_options(dir: &Path) -> RenderOptions {
    RenderOptions {
        format: OutputFormat::Webp,
        ..options(dir)
    }
}

#[test]
fn webp_paths_and_manifest() {
    let ws = compositions(None);
    let dir = tempfile::tempdir().unwrap();
    let (_, manifest) = export::render(&ws, &webp_options(dir.path())).unwrap();
    assert_eq!(manifest.options.format, "webp");
    let written = files(dir.path());
    assert!(written.contains(&"manifest.json".to_owned()));
    for file in written.iter().filter(|f| *f != "manifest.json") {
        assert!(file.ends_with(".webp"), "{file}");
        let bytes = std::fs::read(dir.path().join(file)).unwrap();
        let chunks = images::webp_chunks(&bytes);
        let entry = manifest
            .files
            .iter()
            .find(|e| e.path.as_deref() == Some(file.as_str()))
            .expect("listed in the manifest");
        match entry.frame_count {
            Some(frames) => {
                assert_eq!(chunks[0].fourcc, "VP8X", "{file}");
                assert_eq!(
                    chunks.len(),
                    2 + frames,
                    "{file}: VP8X, ANIM and the frames"
                );
            }
            None => assert_eq!(chunks.len(), 1, "{file}: a single VP8L chunk"),
        }
    }
}

#[test]
fn rendered_animated_webp_round_trip_with_scale() {
    let ws = compositions(Some(json!(["/scenes/night"])));
    let dir = tempfile::tempdir().unwrap();
    let mut opts = webp_options(dir.path());
    opts.scale = 3;
    export::render(&ws, &opts).unwrap();
    let bytes = std::fs::read(dir.path().join("scenes/night.webp")).unwrap();
    let decoded = images::decode_webp_frames(&bytes);
    let animation = pixel_perfect::animate::Animation::new(&ws, "scenes", "night").unwrap();
    let expected = animation.ms_frames();
    assert_eq!(decoded.len(), expected.len());
    for ((pixels, ms), (image, expected_ms)) in decoded.iter().zip(&expected) {
        assert_eq!(pixels.as_slice(), image.upscale(3).pixels());
        assert_eq!(u128::from(*ms), *expected_ms);
    }
}

#[test]
fn webp_side_limit() {
    let lines: Vec<String> = vec!["r".repeat(300); 300];
    let ws = worksheet(
        json!([{ "id": "big", "_palette": "p", "lines": lines }]),
        None,
    );
    let dir = tempfile::tempdir().unwrap();
    let opts = RenderOptions {
        scale: 64,
        max_image_size: None,
        ..webp_options(dir.path())
    };
    let errors = plan_errors(&ws, &opts);
    assert_error(
        &errors,
        "/sprites/0/id",
        "19200x19200 is too large for WebP",
    );
    assert_error(&errors, "/sprites/0/id", "PNG has no such limit");
    assert!(files(dir.path()).is_empty());

    // The same render as PNG passes the plan.
    let png = RenderOptions {
        format: OutputFormat::Png,
        ..opts
    };
    export::plan(&ws, &png).unwrap();
}

#[test]
fn webp_accepts_rates_that_apng_rejects() {
    let ws = denominator_scene();
    let dir = tempfile::tempdir().unwrap();
    let (_, manifest) = export::render(&ws, &webp_options(dir.path())).unwrap();
    assert!(manifest.entry("/scenes/s").unwrap().frame_count.unwrap() > 0);
}

#[test]
fn webp_frame_count_after_splitting_is_bounded() {
    let doc = json!({
        "palettes": [{ "id": "p", "colors": [{ "char": "a", "rgba": [1, 0, 0] }] }],
        "sprites": [{ "id": "a", "_palette": "p", "lines": ["a"] }],
        "panels": [{ "id": "still", "width": 1, "height": 1, "elements": [
            { "id": "e", "_sprite": "a", "coordinates": [0, 0] }] }],
        "timelines": [{ "id": "long", "_panel": "still", "fps": 1, "duration": 100_000_000_000_000u64,
            "transitions": [{ "kind": "setVisible", "_element": "e", "value": true, "at": 0 }] }],
        "exports": ["/timelines/long"]
    });
    let ws = common::load(doc);
    let dir = tempfile::tempdir().unwrap();
    let errors = plan_errors(&ws, &webp_options(dir.path()));
    assert_error(&errors, "/timelines/0", "WebP frames");
    assert!(files(dir.path()).is_empty());
}

#[test]
fn repeated_webp_render_is_byte_identical_without_metadata() {
    let ws = compositions(None);
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    export::render(&ws, &webp_options(a.path())).unwrap();
    export::render(&ws, &webp_options(b.path())).unwrap();
    let written = files(a.path());
    assert!(
        written.iter().any(|f| f.starts_with("timelines/")),
        "{written:?}"
    );
    for file in written {
        let bytes = std::fs::read(a.path().join(&file)).unwrap();
        assert!(
            bytes == std::fs::read(b.path().join(&file)).unwrap(),
            "{file}"
        );
        if file.ends_with(".webp") {
            for chunk in images::webp_chunks(&bytes) {
                assert!(
                    ["VP8L", "VP8X", "ANIM", "ANMF"].contains(&chunk.fourcc.as_str()),
                    "{file}: unexpected chunk {}",
                    chunk.fourcc
                );
            }
        }
    }
}

// ---------------------------------------------------------------- sequences

fn sequence_value() -> Value {
    let text = std::fs::read_to_string(common::fixture("valid/sequence.yaml")).unwrap();
    pixel_perfect::input::parse(&text, Format::Yaml).unwrap()
}

fn render_sequence(
    doc: Value,
    format: OutputFormat,
) -> (tempfile::TempDir, export::Plan, export::Manifest) {
    let ws = pixel_perfect::load_value(doc).unwrap_or_else(|e| panic!("{:#?}", e.diagnostics()));
    let dir = tempfile::tempdir().unwrap();
    let options = RenderOptions {
        output_path: dir.path().to_owned(),
        format,
        scale: 1,
        safe_build: false,
        single_frames: false,
        max_image_size: Some(export::DEFAULT_MAX_IMAGE_SIZE),
    };
    let (plan, manifest) = export::render(&ws, &options).unwrap();
    (dir, plan, manifest)
}

#[test]
fn sequence_is_written_as_apng_with_the_scene_size() {
    let (dir, plan, manifest) = render_sequence(sequence_value(), OutputFormat::Png);
    let png = std::fs::read(dir.path().join("sequences/drift.png")).unwrap();
    assert!(images::chunks(&png).iter().any(|c| c == "acTL"));
    assert_eq!(images::header(&png).0, 8);
    assert_eq!(images::header(&png).1, 4);
    let entry = manifest.entry("/sequences/drift").unwrap();
    assert_eq!(
        (entry.path.as_deref().unwrap(), entry.width, entry.height),
        ("sequences/drift.png", 8, 4)
    );
    assert!(entry.frame_count.is_some_and(|n| n > 1));
    assert_eq!(
        entry.loop_seconds,
        Some(export::Fraction { num: 2, den: 1 })
    );
    // Implicit export: the scene is written too, and no notice is needed.
    assert!(dir.path().join("scenes/valley.png").exists());
    assert!(plan.notices.is_empty(), "{:?}", plan.notices);
}

#[test]
fn sequence_is_written_as_animated_webp() {
    let (dir, _, manifest) = render_sequence(sequence_value(), OutputFormat::Webp);
    let webp = std::fs::read(dir.path().join("sequences/drift.webp")).unwrap();
    let names: Vec<String> = images::webp_chunks(&webp)
        .iter()
        .map(|c| c.fourcc.clone())
        .collect();
    assert_eq!(&names[..2], ["VP8X", "ANIM"]);
    assert_eq!(
        manifest
            .entry("/sequences/drift")
            .unwrap()
            .path
            .as_deref()
            .unwrap(),
        "sequences/drift.webp"
    );
}

#[test]
fn explicit_sequence_export() {
    let mut doc = sequence_value();
    doc["exports"] = json!(["/sequences/drift"]);
    let (_, _, manifest) = render_sequence(doc, OutputFormat::Png);
    assert_eq!(
        manifest
            .files
            .iter()
            .map(|e| e.id.as_str())
            .collect::<Vec<_>>(),
        ["/sequences/drift"]
    );
}

#[test]
fn longer_loop_prints_a_notice() {
    // Sequence: 5 frame-units at 4 fps = 5/4 s; flicker layer: 1/2 s.
    let mut doc = sequence_value();
    doc["sequences"][0]["duration"] = json!(5);
    doc["sequences"][0]["actions"] =
        json!([{ "kind": "setVisible", "_layer": "light", "value": true, "at": 4 }]);
    doc["exports"] = json!(["/sequences/drift"]);
    let (_, plan, manifest) = render_sequence(doc, OutputFormat::Png);
    assert_eq!(
        plan.notices,
        [
            "/sequences/drift loops every 5/2 s (its own duration is 5/4 s) because of its animated layers"
        ]
    );
    assert_eq!(
        manifest.entry("/sequences/drift").unwrap().loop_seconds,
        Some(export::Fraction { num: 5, den: 2 })
    );
}

// Single frames (--single-frames)

/// Flipbook `spin` (8x8, `fps` 12, durations 2, 2, 1, 1, four solid colors),
/// sprite `a` and the scene-level nested rates whose APNG delays overflow.
fn spin_sheet() -> ResolvedWorksheet {
    let sprites: Vec<Value> = ["a", "b", "c", "d"]
        .iter()
        .map(|c| json!({ "id": c, "_palette": "p", "lines": vec![c.repeat(8); 8] }))
        .collect();
    common::load(json!({
        "palettes": [{ "id": "p", "colors": [
            { "char": "a", "rgba": [1, 0, 0] }, { "char": "b", "rgba": [2, 0, 0] },
            { "char": "c", "rgba": [3, 0, 0] }, { "char": "d", "rgba": [4, 0, 0] }
        ] }],
        "sprites": sprites,
        "flipbooks": [{ "id": "spin", "width": 8, "height": 8, "fps": 12, "frames": [
            { "_sprite": "a", "coordinates": [0, 0], "duration": 2 },
            { "_sprite": "b", "coordinates": [0, 0], "duration": 2 },
            { "_sprite": "c", "coordinates": [0, 0], "duration": 1 },
            { "_sprite": "d", "coordinates": [0, 0], "duration": 1 }
        ] }],
        "exports": ["/flipbooks/spin", "/sprites/a"]
    }))
}

fn single_frames(dir: &Path) -> RenderOptions {
    RenderOptions {
        single_frames: true,
        ..options(dir)
    }
}

#[test]
fn single_frames_plan_a_directory_per_animation() {
    let ws = spin_sheet();
    let plan = export::plan(&ws, &single_frames(Path::new("out"))).unwrap();
    let paths: Vec<(&str, export::EntryKind)> = plan
        .outputs
        .iter()
        .map(|o| (o.path.as_str(), o.kind))
        .collect();
    assert_eq!(
        paths,
        [
            ("flipbooks/spin", export::EntryKind::FrameCollection),
            ("sprites/a.png", export::EntryKind::Static)
        ]
    );
    let safe = RenderOptions {
        safe_build: true,
        ..single_frames(Path::new("out"))
    };
    let plan = export::plan(&ws, &safe).unwrap();
    assert_eq!(
        plan.outputs[0].path,
        format!("flipbooks/{}", export::id_hash("spin"))
    );
}

#[test]
fn single_frames_skip_the_apng_delay_check() {
    // Nested coprime rates (see tests/animate.rs): some frame lasts a
    // fraction whose denominator no APNG delay can hold.
    let sprites: Vec<Value> = ["a", "b", "c", "d"]
        .iter()
        .map(|c| json!({ "id": c, "_palette": "p", "lines": [c] }))
        .collect();
    let ws = common::load(json!({
        "palettes": [{ "id": "p", "colors": [
            { "char": "a", "rgba": [1, 0, 0] }, { "char": "b", "rgba": [2, 0, 0] },
            { "char": "c", "rgba": [3, 0, 0] }, { "char": "d", "rgba": [4, 0, 0] }
        ] }],
        "sprites": sprites,
        "flipbooks": [
            { "id": "f89", "width": 1, "height": 1, "fps": 89, "frames": [
                { "_sprite": "a", "coordinates": [0, 0], "duration": 1 },
                { "_sprite": "b", "coordinates": [0, 0], "duration": 1 } ] },
            { "id": "f83", "width": 1, "height": 1, "fps": 83, "frames": [
                { "_sprite": "c", "coordinates": [0, 0], "duration": 1 },
                { "_sprite": "d", "coordinates": [0, 0], "duration": 1 } ] }
        ],
        "panels": [
            { "id": "t", "width": 1, "height": 1, "elements": [{ "id": "e", "_flipbook": "f89", "coordinates": [0, 0] }] },
            { "id": "p", "width": 1, "height": 1, "elements": [{ "_flipbook": "f83", "coordinates": [0, 0] }] }
        ],
        "timelines": [{ "id": "tl", "_panel": "t", "fps": 97, "duration": 97,
            "transitions": [{ "kind": "seekFlipbook", "_element": "e", "at": 1 }] }],
        "scenes": [{ "id": "s", "width": 1, "height": 1, "layers": [
            { "_timeline": "tl", "coordinates": [0, 0] }, { "_panel": "p", "coordinates": [0, 0] }
        ] }],
        "exports": ["/scenes/s"]
    }));
    let errors = export::plan(&ws, &options(Path::new("out"))).unwrap_err();
    assert_error(&errors, "/scenes/0", "APNG cannot store exactly");
    assert!(export::plan(&ws, &single_frames(Path::new("out"))).is_ok());
}

#[test]
fn frame_collection_entry() {
    let dir = tempfile::tempdir().unwrap();
    export::render(&spin_sheet(), &single_frames(dir.path())).unwrap();
    let manifest: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.path().join("manifest.json")).unwrap())
            .unwrap();
    assert_eq!(manifest["options"]["singleFrames"], true);
    let frame = |i: usize, den: u64| json!({ "index": i, "path": format!("flipbooks/spin/{i:04}.png"), "duration": { "num": 1, "den": den } });
    assert_eq!(
        manifest["files"][0],
        json!({
            "id": "/flipbooks/spin", "kind": "frameCollection", "width": 8, "height": 8,
            "frames": [frame(0, 6), frame(1, 6), frame(2, 12), frame(3, 12)],
            "loop": { "num": 1, "den": 2 }
        })
    );
    assert_eq!(manifest["files"][1]["path"], "sprites/a.png");
    common::assert_valid_manifest(&manifest);
    assert_eq!(
        files(dir.path()),
        [
            "flipbooks/spin/0000.png",
            "flipbooks/spin/0001.png",
            "flipbooks/spin/0002.png",
            "flipbooks/spin/0003.png",
            "manifest.json",
            "sprites/a.png"
        ]
    );
}

#[test]
fn single_png_frames_are_still_images_of_the_animation() {
    let ws = spin_sheet();
    let dir = tempfile::tempdir().unwrap();
    let opts = RenderOptions {
        scale: 2,
        ..single_frames(dir.path())
    };
    let (_, manifest) = export::render(&ws, &opts).unwrap();
    let expected = pixel_perfect::animate::Animation::new(&ws, "flipbooks", "spin")
        .unwrap()
        .exact_frames();
    let frames = manifest
        .entry("/flipbooks/spin")
        .unwrap()
        .frames
        .clone()
        .unwrap();
    assert_eq!(frames.len(), expected.len());
    // One color space: the 4 colors of all frames in one palette, in playback
    // order, shared by every frame.
    let shared_plte: Vec<u8> = (1..=4).flat_map(|r| [r, 0, 0]).collect();
    for (frame, (image, _)) in frames.iter().zip(&expected) {
        let png = std::fs::read(dir.path().join(&frame.path)).unwrap();
        assert!(
            !images::chunks(&png).iter().any(|c| c == "acTL"),
            "{}",
            frame.path
        );
        let (_, _, bit_depth, color_type) = images::header(&png);
        assert_eq!(
            (color_type, bit_depth),
            (3, 2),
            "{}: indexed, 2 bits",
            frame.path
        );
        assert_eq!(
            images::chunk_data(&png, "PLTE"),
            Some(shared_plte.as_slice()),
            "{}",
            frame.path
        );
        let (w, h, pixels) = images::decode(&png);
        assert_eq!((w, h), (16, 16));
        assert!(pixels == image.upscale(2).pixels(), "{}", frame.path);
    }
}

#[test]
fn single_webp_frames_are_still_images_of_the_animation() {
    let ws = spin_sheet();
    let dir = tempfile::tempdir().unwrap();
    let opts = RenderOptions {
        single_frames: true,
        ..webp_options(dir.path())
    };
    let (_, manifest) = export::render(&ws, &opts).unwrap();
    let expected = pixel_perfect::animate::Animation::new(&ws, "flipbooks", "spin")
        .unwrap()
        .exact_frames();
    let frames = manifest
        .entry("/flipbooks/spin")
        .unwrap()
        .frames
        .clone()
        .unwrap();
    assert_eq!(frames.len(), expected.len());
    for (frame, (image, _)) in frames.iter().zip(&expected) {
        assert!(frame.path.ends_with(".webp"), "{}", frame.path);
        let webp = std::fs::read(dir.path().join(&frame.path)).unwrap();
        let fourccs: Vec<String> = images::webp_chunks(&webp)
            .into_iter()
            .map(|c| c.fourcc)
            .collect();
        assert!(!fourccs.iter().any(|c| c == "ANIM"), "{}", frame.path);
        let (_, _, pixels) = images::decode_webp(&webp);
        assert!(pixels == image.pixels(), "{}", frame.path);
    }
}

#[test]
fn frame_names_are_zero_padded_to_the_last_index() {
    assert_eq!(export::frame_name(0, 4), "0000");
    assert_eq!(export::frame_name(3, 4), "0003");
    assert_eq!(export::frame_name(9999, 10000), "9999");
    assert_eq!(export::frame_name(0, 10001), "00000");
    assert_eq!(export::frame_name(10000, 10001), "10000");
    assert_eq!(export::frame_name(12344, 12345), "12344");
    assert_eq!(export::frame_name(0, 1), "0000");
}

#[test]
fn single_png_frames_are_all_rgba_when_their_colors_do_not_fit_one_palette() {
    // Two 16x9 frames of 144 colors each, all different: each would fit a
    // palette on its own, but together they have 288 colors.
    let chars: Vec<char> = (0x4E00..0x4E00 + 288).filter_map(char::from_u32).collect();
    let colors: Vec<Value> = chars
        .iter()
        .enumerate()
        .map(|(i, c)| json!({ "char": c.to_string(), "rgba": [i % 256, i / 256, 7] }))
        .collect();
    let lines = |half: usize| -> Vec<String> {
        chars[half * 144..(half + 1) * 144]
            .chunks(16)
            .map(|row| row.iter().collect())
            .collect()
    };
    let ws = common::load(json!({
        "palettes": [{ "id": "p", "colors": colors }],
        "sprites": [
            { "id": "first", "_palette": "p", "lines": lines(0) },
            { "id": "second", "_palette": "p", "lines": lines(1) }
        ],
        "flipbooks": [{ "id": "rich", "width": 16, "height": 9, "fps": 1, "frames": [
            { "_sprite": "first", "coordinates": [0, 0], "duration": 1 },
            { "_sprite": "second", "coordinates": [0, 0], "duration": 1 }
        ] }],
        "exports": ["/flipbooks/rich"]
    }));
    let dir = tempfile::tempdir().unwrap();
    let (_, manifest) = export::render(&ws, &single_frames(dir.path())).unwrap();
    let frames = manifest
        .entry("/flipbooks/rich")
        .unwrap()
        .frames
        .clone()
        .unwrap();
    assert_eq!(frames.len(), 2);
    for frame in &frames {
        let png = std::fs::read(dir.path().join(&frame.path)).unwrap();
        assert_eq!(images::header(&png).3, 6, "{}: RGBA", frame.path);
    }
}

// Unchanged files are not rewritten

/// Modification time of `path`.
fn mtime(path: &Path) -> std::time::SystemTime {
    std::fs::metadata(path).unwrap().modified().unwrap()
}

/// Sets the modification time of `path` one hour in the past, so that a
/// rewrite is visible whatever the file system's time resolution.
fn age(path: &Path) -> std::time::SystemTime {
    let past = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(past)
        .unwrap();
    mtime(path)
}

#[test]
fn identical_render_writes_nothing() {
    let ws = worksheet(json!([sprite("hero"), sprite("coin")]), None);
    let dir = tempfile::tempdir().unwrap();
    let first = export::render_report(&ws, &options(dir.path())).unwrap();
    assert_eq!((first.written, first.unchanged), (3, 0));
    let hero = dir.path().join("sprites/hero.png");
    let manifest = dir.path().join("manifest.json");
    let (hero_time, manifest_time) = (age(&hero), age(&manifest));
    let second = export::render_report(&ws, &options(dir.path())).unwrap();
    assert_eq!((second.written, second.unchanged), (0, 3));
    assert_eq!(mtime(&hero), hero_time);
    assert_eq!(mtime(&manifest), manifest_time);
}

#[test]
fn only_changed_files_are_rewritten() {
    let dir = tempfile::tempdir().unwrap();
    let before = worksheet(json!([sprite("hero"), sprite("coin")]), None);
    export::render(&before, &options(dir.path())).unwrap();
    let (hero, coin) = (
        dir.path().join("sprites/hero.png"),
        dir.path().join("sprites/coin.png"),
    );
    let (hero_time, coin_time) = (age(&hero), age(&coin));
    // Same size, different pixels: the manifest does not change either.
    let after = worksheet(
        json!([sprite("hero"), { "id": "coin", "_palette": "p", "lines": ["rrr", "ggg"] }]),
        None,
    );
    let report = export::render_report(&after, &options(dir.path())).unwrap();
    assert_eq!((report.written, report.unchanged), (1, 2));
    assert_eq!(mtime(&hero), hero_time);
    assert_ne!(mtime(&coin), coin_time);
}

#[test]
fn different_file_is_overwritten_in_place() {
    let ws = worksheet(json!([sprite("hero")]), None);
    let dir = tempfile::tempdir().unwrap();
    let hero = dir.path().join("sprites/hero.png");
    std::fs::create_dir_all(hero.parent().unwrap()).unwrap();
    std::fs::write(&hero, b"not the rendered image").unwrap();
    #[cfg(unix)]
    let inode = {
        use std::os::unix::fs::MetadataExt;
        std::fs::metadata(&hero).unwrap().ino()
    };
    let report = export::render_report(&ws, &options(dir.path())).unwrap();
    assert_eq!(report.written, 2);
    assert_eq!(&std::fs::read(&hero).unwrap()[1..4], b"PNG");
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert_eq!(std::fs::metadata(&hero).unwrap().ino(), inode, "same file");
    }
}
