//! Sprite rasterization, export planning, safe build and manifest.

mod common;
#[path = "common/images.rs"]
mod images;

use std::path::Path;
use std::process::Command;

use common::assert_error;
use pixel_perfect::export::{self, OutputFormat, RenderError, RenderOptions};
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

#[test]
fn implicit_exports_skip_unsupported_with_a_notice() {
    let doc = json!({
        "palettes": [{ "id": "p", "colors": [{ "char": "r", "rgba": [1, 2, 3] }] }],
        "sprites": [{ "id": "a", "_palette": "p", "lines": ["r"] }, { "id": "b", "_palette": "p", "lines": ["r"] }],
        "panels": [{ "id": "stage", "width": 2, "height": 2, "elements": [{ "_sprite": "a", "coordinates": [0, 0] }] }]
    });
    let ws = pixel_perfect::load_value(doc).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let (plan, _) = export::render(&ws, &options(dir.path())).unwrap();
    assert_eq!(
        files(dir.path()),
        ["manifest.json", "sprites/a.png", "sprites/b.png"]
    );
    assert_eq!(plan.notices.len(), 1);
    assert!(
        plan.notices[0].contains("panels are not supported yet"),
        "{:?}",
        plan.notices
    );
}

#[test]
fn unsupported_explicit_export_writes_nothing() {
    let ws = pixel_perfect::load_file(&common::fixture("valid/full.json")).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let err = export::render(&ws, &options(dir.path())).unwrap_err();
    let RenderError::Plan(diagnostics) = err else {
        panic!("expected a plan error")
    };
    assert_error(&diagnostics, "/exports/2", "panels are not supported yet");
    assert!(files(dir.path()).is_empty());
}

#[test]
fn same_id_in_different_collections_does_not_conflict() {
    // Panels are not rendered yet; the plan keys collisions by collection.
    let ws = worksheet(json!([sprite("hero")]), None);
    let plan = export::plan(&ws, &options(Path::new("out"))).unwrap();
    assert_eq!(plan.outputs[0].path, "sprites/hero.png");
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
        manifest.files["/sprites/ui/button"].path,
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
            "format": "png",
            "scale": 2,
            "safeBuild": false,
            "files": { "/sprites/hero": { "path": "sprites/hero.png", "width": 6, "height": 4 } }
        })
    );
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
    assert!(manifest.safe_build);
    assert_eq!(
        manifest.files["/sprites/hero"].path,
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
