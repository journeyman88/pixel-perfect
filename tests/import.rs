// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 The pixel-perfect Authors

//! `import`: PNG decoding, palettes, format-preserving edits, checks and
//! writing (scenarios of the png-import spec).

mod common;
#[path = "common/images.rs"]
mod images;

use std::fs;
use std::path::{Path, PathBuf};

use common::fixture;
use images::{PngSpec, rgba_png, rgba_png_scaled, write_png};
use pixel_perfect::edit::{self, Addition, Node};
use pixel_perfect::import::{self, ImportError, ImportOptions, image};
use pixel_perfect::input::{self, Format};
use pixel_perfect::model::Rgba;
use pixel_perfect::raster::{Image, TRANSPARENT};
use png::{BitDepth, ColorType};
use serde_json::Value;
use tempfile::TempDir;

const RED: Rgba = Rgba([255, 0, 0, 255]);
const BLUE: Rgba = Rgba([0, 0, 255, 255]);
const GREEN: Rgba = Rgba([0, 255, 0, 255]);
const GHOST: Rgba = Rgba([255, 255, 255, 128]);

/// A 3x2 image: `red red blue` / `green ghost .`.
fn small() -> Image {
    Image::new(3, 2, vec![RED, RED, BLUE, GREEN, GHOST, TRANSPARENT])
}

/// A worksheet file and a PNG in a temporary directory.
struct Case {
    _dir: TempDir,
    ws: PathBuf,
    png: PathBuf,
}

impl Case {
    fn new(ws_name: &str, ws_text: &str, png: &[u8]) -> Case {
        let dir = TempDir::new().unwrap();
        let ws = dir.path().join(ws_name);
        let png_path = dir.path().join("art.png");
        fs::write(&ws, ws_text).unwrap();
        fs::write(&png_path, png).unwrap();
        Case {
            _dir: dir,
            ws,
            png: png_path,
        }
    }

    fn from_fixture(path: &Path, png: &[u8]) -> Case {
        let name = path.file_name().unwrap().to_str().unwrap();
        Case::new(name, &fs::read_to_string(path).unwrap(), png)
    }

    fn options(&self, id: &str) -> ImportOptions {
        ImportOptions::new(&self.ws, &self.png, id)
    }

    fn text(&self) -> String {
        fs::read_to_string(&self.ws).unwrap()
    }

    fn files(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(self.ws.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
}

fn format_of(path: &Path) -> Format {
    Format::from_path(path).unwrap()
}

/// Imports and checks the invariants every successful import must keep.
fn import_ok(case: &Case, options: &ImportOptions) -> (String, String) {
    let before = case.text();
    let imported = import::run(options).unwrap_or_else(|e| panic!("{e}"));
    let after = case.text();
    if options.dry_run {
        assert_eq!(after, before);
    } else {
        assert_eq!(after, imported.text);
    }
    assert!(edit::is_subsequence(&before, &imported.text));
    pixel_perfect::load_str(&imported.text, format_of(&case.ws)).unwrap();
    (before, imported.text)
}

fn rejected(case: &Case, options: &ImportOptions) -> String {
    let before = case.text();
    let files = case.files();
    let err = import::run(options).unwrap_err();
    assert_eq!(case.text(), before, "the worksheet must be unchanged");
    assert_eq!(case.files(), files, "no file may be left behind");
    match err {
        ImportError::Rejected(m) => m,
        other => panic!("expected a rejection, got {other:?}"),
    }
}

/// The text inserted by an import (the result minus the original, assuming
/// a single insertion point).
fn inserted(before: &str, after: &str) -> String {
    let prefix = before
        .bytes()
        .zip(after.bytes())
        .take_while(|(a, b)| a == b)
        .count();
    let suffix = before[prefix..]
        .bytes()
        .rev()
        .zip(after[prefix..].bytes().rev())
        .take_while(|(a, b)| a == b)
        .count();
    after[prefix..after.len() - suffix].to_owned()
}

// ---------------------------------------------------------------- decoding

fn decode(spec: &PngSpec) -> Result<Image, String> {
    image::decode(&write_png(spec), 1 << 30)
}

#[test]
fn decodes_every_color_type() {
    let gray = decode(&PngSpec::new(
        2,
        1,
        ColorType::Grayscale,
        BitDepth::Eight,
        &[0, 128],
    ))
    .unwrap();
    assert_eq!(
        gray.pixels(),
        &[Rgba([0, 0, 0, 255]), Rgba([128, 128, 128, 255])]
    );

    let ga = decode(&PngSpec::new(
        2,
        1,
        ColorType::GrayscaleAlpha,
        BitDepth::Eight,
        &[10, 255, 20, 0],
    ))
    .unwrap();
    assert_eq!(ga.pixels(), &[Rgba([10, 10, 10, 255]), TRANSPARENT]);

    let rgb = decode(&PngSpec::new(
        1,
        1,
        ColorType::Rgb,
        BitDepth::Eight,
        &[1, 2, 3],
    ))
    .unwrap();
    assert_eq!(rgb.pixels(), &[Rgba([1, 2, 3, 255])]);

    let rgba = decode(&PngSpec::new(
        1,
        1,
        ColorType::Rgba,
        BitDepth::Eight,
        &[1, 2, 3, 4],
    ))
    .unwrap();
    assert_eq!(rgba.pixels(), &[Rgba([1, 2, 3, 4])]);
}

#[test]
fn decodes_indexed_images_with_transparency() {
    let palette = [9, 9, 9, 255, 0, 0, 0, 0, 255, 0, 0, 0];
    let trns = [0];
    let expected = [TRANSPARENT, RED, BLUE, RED];
    // Indices 0 1 2 1 at 1, 2, 4 and 8 bits per pixel.
    let cases: [(BitDepth, &[u8]); 4] = [
        (BitDepth::One, &[]),
        (BitDepth::Two, &[0b0001_1001]),
        (BitDepth::Four, &[0x01, 0x21]),
        (BitDepth::Eight, &[0, 1, 2, 1]),
    ];
    for (depth, data) in cases {
        if depth == BitDepth::One {
            continue; // 3 entries do not fit in 1 bit; covered below.
        }
        let mut spec = PngSpec::new(4, 1, ColorType::Indexed, depth, data);
        spec.palette = Some(&palette);
        spec.trns = Some(&trns);
        assert_eq!(decode(&spec).unwrap().pixels(), &expected, "{depth:?}");
    }
    let mut one = PngSpec::new(4, 1, ColorType::Indexed, BitDepth::One, &[0b0101_0000]);
    one.palette = Some(&palette[..6]);
    one.trns = Some(&trns);
    assert_eq!(
        decode(&one).unwrap().pixels(),
        &[
            TRANSPARENT,
            Rgba([255, 0, 0, 255]),
            TRANSPARENT,
            Rgba([255, 0, 0, 255])
        ]
    );
}

#[test]
fn exact_16_bit_image_is_accepted() {
    let data: Vec<u8> = [257u16, 2 * 257, 3 * 257, 255 * 257]
        .iter()
        .flat_map(|v| v.to_be_bytes())
        .collect();
    let img = decode(&PngSpec::new(
        1,
        1,
        ColorType::Rgba,
        BitDepth::Sixteen,
        &data,
    ))
    .unwrap();
    assert_eq!(img.pixels(), &[Rgba([1, 2, 3, 255])]);
}

#[test]
fn lossy_16_bit_sample_names_the_pixel() {
    let mut samples = [0u16; 4 * 6 * 4];
    samples.iter_mut().for_each(|s| *s = 65535);
    samples[(5 * 4 + 3) * 4] = 1000; // red of pixel (3, 5)
    let data: Vec<u8> = samples.iter().flat_map(|v| v.to_be_bytes()).collect();
    let err = decode(&PngSpec::new(
        4,
        6,
        ColorType::Rgba,
        BitDepth::Sixteen,
        &data,
    ))
    .unwrap_err();
    assert!(err.contains("pixel (3, 5)"), "{err}");
}

#[test]
fn transparent_pixels_are_equal_whatever_their_color() {
    let img = decode(&PngSpec::new(
        2,
        1,
        ColorType::Rgba,
        BitDepth::Eight,
        &[255, 0, 0, 0, 0, 0, 255, 0],
    ))
    .unwrap();
    assert_eq!(img.pixels(), &[TRANSPARENT, TRANSPARENT]);
}

#[test]
fn animated_png_and_jpeg_are_rejected() {
    let mut spec = PngSpec::new(1, 1, ColorType::Rgba, BitDepth::Eight, &[1, 2, 3, 255]);
    spec.animated = true;
    assert!(decode(&spec).unwrap_err().contains("animated PNG"));
    let jpeg = [0xFF, 0xD8, 0xFF, 0xE0, 0, 16, b'J', b'F', b'I', b'F'];
    assert_eq!(image::decode(&jpeg, 1 << 30).unwrap_err(), "not a PNG file");
}

// ------------------------------------------------------------- downscaling

#[test]
fn downscale_enlarged_image() {
    let small = Image::from_fn(16, 16, |x, y| Rgba([x as u8, y as u8, 0, 255]));
    let big = image::decode(&rgba_png_scaled(&small, 4), 1 << 30).unwrap();
    assert_eq!((big.width(), big.height()), (64, 64));
    assert_eq!(image::downscale(&big, 4).unwrap(), small);
    assert_eq!(image::downscale(&small, 1).unwrap(), small);
}

#[test]
fn downscale_rejects_bad_sizes_and_blocks() {
    let err = image::downscale(&Image::transparent(30, 32), 4).unwrap_err();
    assert!(
        err.contains("width 30 is not a multiple of --scale 4"),
        "{err}"
    );
    let img = Image::from_fn(16, 16, |x, y| if (x, y) == (10, 13) { RED } else { BLUE });
    let err = image::downscale(&img, 4).unwrap_err();
    assert!(err.contains("block at (8, 12)"), "{err}");
}

#[test]
fn downscale_treats_all_transparent_pixels_as_one_color() {
    let img = Image::new(2, 1, vec![Rgba([1, 0, 0, 0]), Rgba([0, 0, 9, 0])]);
    assert_eq!(
        image::downscale(&img, 1).unwrap().pixels(),
        &[TRANSPARENT, TRANSPARENT]
    );
    let wide = Image::new(2, 2, vec![TRANSPARENT; 4]);
    assert_eq!(
        image::downscale(&wide, 2).unwrap(),
        Image::transparent(1, 1)
    );
}

// ------------------------------------------------------- import, palettes

#[test]
fn basic_import_renders_the_png() {
    let case = Case::from_fixture(&fixture("valid/readme-example.yaml"), &rgba_png(&small()));
    let mut options = case.options("coin");
    options.description = Some("Spinning coin, frame 1".into());
    let imported = import::run(&options).unwrap();
    assert_eq!(
        imported.summary,
        format!(
            "{}: added /palettes/coin, /sprites/coin (3\u{d7}2, 4 colors)",
            case.ws.display()
        )
    );
    let ws = pixel_perfect::load_file(&case.ws).unwrap();
    assert_eq!(pixel_perfect::render::sprite(&ws, "coin"), small());
    let sprite = ws.model.sprites.iter().find(|s| s.id == "coin").unwrap();
    assert_eq!(
        sprite.description.as_deref(),
        Some("Spinning coin, frame 1")
    );
    assert_eq!(
        sprite.lines.as_deref(),
        Some(&["aab".to_owned(), "cd.".to_owned()][..])
    );
    assert_eq!((sprite.width, sprite.height), (None, None));
    let palette = ws.model.palettes.iter().find(|p| p.id == "coin").unwrap();
    assert_eq!(palette.description, None);
    assert!(
        case.text()
            .contains("{ char: d, rgba: [255, 255, 255, 0.5] }")
    );
}

#[test]
fn every_alpha_round_trips() {
    let img = Image::from_fn(254, 1, |x, _| Rgba([7, 7, 7, x as u8 + 1]));
    let case = Case::from_fixture(&fixture("valid/readme-example.json"), &rgba_png(&img));
    import_ok(&case, &case.options("alpha"));
    let ws = pixel_perfect::load_file(&case.ws).unwrap();
    assert_eq!(pixel_perfect::render::sprite(&ws, "alpha"), img);
}

#[test]
fn large_palette_with_explicit_limit() {
    // 50 000 colors: past tier 1 (27 064 characters) into tier 2.
    let img = Image::from_fn(250, 200, |x, y| {
        let i = y * 250 + x;
        Rgba([i as u8, (i >> 8) as u8, 1, 255])
    });
    let case = Case::from_fixture(&fixture("valid/readme-example.yaml"), &rgba_png(&img));
    let err = rejected(&case, &case.options("big"));
    assert!(
        err.contains("50000 colors, more than --max-colors 256"),
        "{err}"
    );
    let mut options = case.options("big");
    options.max_colors = 50_000;
    import_ok(&case, &options);
    let ws = pixel_perfect::load_file(&case.ws).unwrap();
    assert_eq!(pixel_perfect::render::sprite(&ws, "big"), img);
}

#[test]
fn reuse_an_existing_palette() {
    let ws = fixture("../../examples/sprites.yaml");
    let original = pixel_perfect::load_file(&ws).unwrap();
    let slime = pixel_perfect::render::sprite(&original, "slime");
    let case = Case::from_fixture(&ws, &rgba_png(&slime));
    let mut options = case.options("slime-copy");
    options.palette = Some("slime".into());
    let (before, after) = import_ok(&case, &options);
    let added = inserted(&before, &after);
    assert!(!added.contains("colors:"), "no palette is added:\n{added}");
    assert!(added.contains("_palette: slime\n"), "{added}");
    let ws = pixel_perfect::load_file(&case.ws).unwrap();
    let copy = ws
        .model
        .sprites
        .iter()
        .find(|s| s.id == "slime-copy")
        .unwrap();
    let orig = ws.model.sprites.iter().find(|s| s.id == "slime").unwrap();
    assert_eq!(
        copy.lines, orig.lines,
        "same characters as the hand-written sprite"
    );
}

#[test]
fn reuse_a_derived_palette_with_inherited_colors() {
    let ws = fixture("../../examples/sprites.yaml");
    let original = pixel_perfect::load_file(&ws).unwrap();
    let ice = pixel_perfect::render::sprite(&original, "slime-ice");
    let case = Case::from_fixture(&ws, &rgba_png(&ice));
    let mut options = case.options("ice-copy");
    options.palette = Some("ice".into());
    import_ok(&case, &options);
    let ws = pixel_perfect::load_file(&case.ws).unwrap();
    let lines = |id: &str| ws.sprites[id].rows.clone();
    // Eyes (`w`, `k`) and shadow (`s`) are inherited from `slime`.
    assert_eq!(lines("ice-copy"), lines("slime"));
}

#[test]
fn reuse_reports_missing_colors_and_unknown_palettes() {
    let img = Image::from_fn(3, 8, |x, y| {
        if (x, y) == (2, 7) {
            Rgba([255, 0, 255, 255])
        } else {
            Rgba([20, 70, 30, 255])
        }
    });
    let case = Case::from_fixture(&fixture("../../examples/sprites.yaml"), &rgba_png(&img));
    let mut options = case.options("odd");
    options.palette = Some("slime".into());
    let err = rejected(&case, &options);
    assert!(err.contains("(255, 0, 255) at (2, 7)"), "{err}");
    options.palette = Some("nope".into());
    let err = rejected(&case, &options);
    assert!(err.contains("no palette \"nope\""), "{err}");
}

#[test]
fn existing_ids_are_never_overwritten() {
    let case = Case::from_fixture(&fixture("valid/readme-example.yaml"), &rgba_png(&small()));
    let err = rejected(&case, &case.options("heart"));
    assert!(
        err.contains("/sprites/0: sprite \"heart\" already exists"),
        "{err}"
    );
    let err = rejected(&case, &case.options("warm"));
    assert!(
        err.contains("/palettes/0: palette \"warm\" already exists"),
        "{err}"
    );
    assert!(
        err.contains("--palette warm") && err.contains("--palette-id"),
        "{err}"
    );
    let mut options = case.options("warm");
    options.palette_id = Some("warm-2".into());
    import_ok(&case, &options);
}

#[test]
fn empty_id_is_a_usage_error() {
    let case = Case::from_fixture(&fixture("valid/readme-example.yaml"), &rgba_png(&small()));
    assert!(matches!(
        import::run(&case.options("")),
        Err(ImportError::Usage(_))
    ));
}

#[test]
fn export_appends_only_to_an_existing_list() {
    let case = Case::from_fixture(&fixture("valid/readme-example.yaml"), &rgba_png(&small()));
    let mut options = case.options("coin");
    options.export = true;
    import_ok(&case, &options);
    let ws = pixel_perfect::load_file(&case.ws).unwrap();
    assert_eq!(
        ws.model.exports.as_deref(),
        Some(
            &[
                "/sprites/heart".to_owned(),
                "/sprites/heart-dark".to_owned(),
                "/sprites/coin".to_owned()
            ][..]
        )
    );
    assert!(
        case.text()
            .contains("exports: [/sprites/heart, /sprites/heart-dark, \"/sprites/coin\"]\n")
    );

    let case = Case::from_fixture(
        &fixture("import/yaml/non-indented.yaml"),
        &rgba_png(&small()),
    );
    let mut options = case.options("coin");
    options.export = true;
    let (_, after) = import_ok(&case, &options);
    assert!(!after.contains("exports"));

    let case = Case::from_fixture(&fixture("valid/readme-example.yaml"), &rgba_png(&small()));
    import_ok(&case, &case.options("coin"));
    let ws = pixel_perfect::load_file(&case.ws).unwrap();
    assert_eq!(
        ws.model.exports.unwrap().len(),
        2,
        "exports untouched without --export"
    );
}

#[test]
fn scale_option_downscales_before_import() {
    let case = Case::from_fixture(
        &fixture("valid/readme-example.toml"),
        &rgba_png_scaled(&small(), 4),
    );
    let mut options = case.options("coin");
    options.scale = 4;
    import_ok(&case, &options);
    let ws = pixel_perfect::load_file(&case.ws).unwrap();
    assert_eq!(pixel_perfect::render::sprite(&ws, "coin"), small());
}

// -------------------------------------------------------------- editors

fn import_fixture(rel: &str) -> (String, String) {
    let case = Case::from_fixture(&fixture(rel), &rgba_png(&small()));
    let mut options = case.options("coin");
    options.export = true;
    import_ok(&case, &options)
}

#[test]
fn json_layouts() {
    let (_, after) = import_fixture("import/json/four-spaces.json");
    assert!(after.contains("\n        {\n            \"id\": \"coin\",\n            \"colors\": [\n                { \"char\": \"a\", \"rgba\": [255, 0, 0] },"), "{after}");
    assert!(after.contains("Says \\\"hi\\\" with a \\\\ backslash"));

    let (_, after) = import_fixture("import/json/tabs.json");
    assert!(
        after.contains("\n\t\t{\n\t\t\t\"id\": \"coin\",\n\t\t\t\"colors\": [\n"),
        "{after}"
    );

    let (before, after) = import_fixture("import/json/minified.json");
    assert!(!after.contains('\n'), "{after}");
    assert!(after.len() > before.len());
    assert!(after.ends_with(",\"/sprites/coin\"]}"), "{after}");

    let (_, after) = import_fixture("import/json/no-sprites.json");
    assert!(
        after.contains("\n  \"sprites\": [\n    {\n      \"id\": \"coin\",\n"),
        "{after}"
    );
}

#[test]
fn json_empty_arrays_and_objects() {
    let adds = |collection: &str| {
        vec![Addition {
            collection: collection.into(),
            items: vec![Node::Quoted("x".into())],
        }]
    };
    let out = edit::add("{\n  \"exports\": []\n}\n", Format::Json, &adds("exports")).unwrap();
    assert_eq!(out, "{\n  \"exports\": [\n    \"x\"\n  ]\n}\n");
    let out = edit::add("{}", Format::Json, &adds("exports")).unwrap();
    assert_eq!(out, "{\"exports\":[\"x\"]}");
    let two = vec![
        Addition {
            collection: "a".into(),
            items: vec![Node::Num("1".into())],
        },
        Addition {
            collection: "b".into(),
            items: vec![Node::Num("2".into())],
        },
    ];
    let out = edit::add("{\n}\n", Format::Json, &two).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&out).unwrap(),
        serde_json::json!({ "a": [1], "b": [2] })
    );
}

#[test]
fn yaml_keeps_comments_of_the_examples() {
    let case = Case::from_fixture(&fixture("../../examples/sprites.yaml"), &rgba_png(&small()));
    let (before, after) = import_ok(&case, &case.options("imported"));
    assert_eq!(after.matches('#').count() - before.matches('#').count(), 0);
}

#[test]
fn yaml_comments_before_the_next_key_stay_with_it() {
    let (_, after) = import_fixture("import/yaml/comments.yaml");
    let new_palette = after.find("  - id: coin\n    colors:").unwrap();
    assert!(
        new_palette < after.find("# Sprites below.").unwrap(),
        "{after}"
    );
    let new_sprite = after.find("  - id: coin\n    _palette: coin").unwrap();
    assert!(
        new_sprite < after.find("# About exports").unwrap(),
        "{after}"
    );
    assert!(
        after.contains(
            "# About exports\nexports:\n  - \"/sprites/s\"\n  - \"/sprites/coin\"\n# The end.\n"
        ),
        "{after}"
    );
}

#[test]
fn yaml_non_indented_sequences_and_block_scalars() {
    let (_, after) = import_fixture("import/yaml/non-indented.yaml");
    assert!(after.contains("\n- id: coin\n  colors:\n"), "{after}");
    let (before, after) = import_fixture("import/yaml/block-scalar.yaml");
    assert!(after.starts_with(&before[..before.find("palettes:").unwrap()]));
    let ws = pixel_perfect::load_str(&after, Format::Yaml).unwrap();
    assert!(ws.model.sprites.iter().any(|s| s.id == "coin"));
    assert!(!ws.model.sprites.iter().any(|s| s.id == "fake"));
}

#[test]
fn yaml_missing_collection_is_appended() {
    let (_, after) = import_fixture("import/yaml/no-sprites.yaml");
    assert!(
        after.contains("# Sprites will come later.\n\nsprites:\n  - id: coin\n"),
        "{after}"
    );
}

#[test]
fn yaml_without_final_newline() {
    let text = fs::read_to_string(fixture("import/yaml/non-indented.yaml")).unwrap();
    let case = Case::new("w.yaml", text.trim_end(), &rgba_png(&small()));
    import_ok(&case, &case.options("coin"));
}

#[test]
fn yaml_refused_constructs() {
    let case = Case::from_fixture(&fixture("import/yaml/flow.yaml"), &rgba_png(&small()));
    let err = rejected(&case, &case.options("coin"));
    assert!(err.contains("flow collection"), "{err}");
    let case = Case::from_fixture(&fixture("import/yaml/anchor.yaml"), &rgba_png(&small()));
    let err = rejected(&case, &case.options("coin"));
    assert!(err.contains("anchor"), "{err}");

    let sprite = vec![Addition {
        collection: "sprites".into(),
        items: vec![Node::Quoted("x".into())],
    }];
    for (text, what) in [
        ("x: &a 1\nsprites: *a\n", "alias"),
        ("sprites: !!seq\n  - a\n", "tag"),
        (
            "sprites:\n  - a\n---\nsprites:\n  - b\n",
            "several YAML documents",
        ),
        ("sprites:\n  - a\n...\n", "document end"),
        ("{ sprites: [a] }\n", "flow-style document"),
        ("- a\n", "not a mapping"),
        ("sprites:\n\t- a\n", "tab indentation"),
    ] {
        let err = edit::add(text, Format::Yaml, &sprite).unwrap_err();
        assert!(err.0.contains(what), "{text:?}: {err}");
    }
}

#[test]
fn yaml_characters_needing_quotes_round_trip() {
    let chars = [
        '#',
        ':',
        '&',
        '-',
        '?',
        'y',
        'n',
        '"',
        '\\',
        '\'',
        '`',
        '|',
        '>',
        '!',
        '%',
        '@',
        '*',
        '\u{FFFE}',
        '\u{10FFFF}',
    ];
    let colors = chars
        .iter()
        .enumerate()
        .map(|(i, c)| {
            Node::flow_map(vec![
                ("char".into(), Node::Str(c.to_string())),
                (
                    "rgba".into(),
                    Node::flow_seq(vec![
                        Node::Num(i.to_string()),
                        Node::Num("0".into()),
                        Node::Num("0".into()),
                    ]),
                ),
            ])
        })
        .collect();
    let line: String = chars.iter().collect();
    let palette = Node::map(vec![
        ("id".into(), Node::Str("q".into())),
        ("colors".into(), Node::seq(colors)),
    ]);
    let sprite = Node::map(vec![
        ("id".into(), Node::Str("q".into())),
        ("_palette".into(), Node::Str("q".into())),
        ("lines".into(), Node::seq(vec![Node::Quoted(line)])),
    ]);
    let adds = vec![
        Addition {
            collection: "palettes".into(),
            items: vec![palette.clone()],
        },
        Addition {
            collection: "sprites".into(),
            items: vec![sprite.clone()],
        },
    ];
    for (name, format) in [
        ("readme-example.yaml", Format::Yaml),
        ("readme-example.json", Format::Json),
        ("readme-example.toml", Format::Toml),
    ] {
        let text = fs::read_to_string(fixture(&format!("valid/{name}"))).unwrap();
        let out = edit::add(&text, format, &adds).unwrap();
        let value = input::parse(&out, format).unwrap_or_else(|e| panic!("{name}: {e:?}\n{out}"));
        assert_eq!(value["palettes"][2], palette.to_json(), "{name}");
        assert_eq!(value["sprites"][2], sprite.to_json(), "{name}");
        pixel_perfect::load_str(&out, format)
            .unwrap_or_else(|e| panic!("{name}: {:?}", e.diagnostics()));
        if format == Format::Yaml {
            assert!(out.contains("char: \"\\uFFFE\""), "{out}");
        }
    }
}

#[test]
fn crlf_files_get_crlf_insertions() {
    for name in [
        "readme-example.yaml",
        "readme-example.json",
        "readme-example.toml",
    ] {
        let text = fs::read_to_string(fixture(&format!("valid/{name}")))
            .unwrap()
            .replace('\n', "\r\n");
        let case = Case::new(name, &text, &rgba_png(&small()));
        let mut options = case.options("coin");
        options.export = true;
        let (_, after) = import_ok(&case, &options);
        assert_eq!(
            after.matches('\n').count(),
            after.matches("\r\n").count(),
            "{name}:\n{after}"
        );
    }
}

#[test]
fn toml_layouts() {
    let (_, after) = import_fixture("valid/readme-example.toml");
    assert!(
        after
            .contains("exports = [\"/sprites/heart\", \"/sprites/heart-dark\", \"/sprites/coin\"]"),
        "{after}"
    );
    assert!(
        after.contains(
            "[[palettes]]\nid = \"coin\"\ncolors = [\n  { char = \"a\", rgba = [255, 0, 0] },\n"
        ),
        "{after}"
    );
    let (before, after) = import_fixture("import/toml/split.toml");
    assert!(edit::is_subsequence(&before, &after));
    assert!(after.contains("# one color"));
    let ws = pixel_perfect::load_str(&after, Format::Toml).unwrap();
    let ids: Vec<&str> = ws.model.sprites.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["s", "t", "coin"]);
}

#[test]
fn same_result_in_every_format() {
    let results: Vec<Value> = ["yaml", "json", "toml"]
        .iter()
        .map(|ext| {
            let path = fixture(&format!("valid/readme-example.{ext}"));
            let case = Case::from_fixture(&path, &rgba_png(&small()));
            let mut options = case.options("coin");
            options.export = true;
            options.description = Some("A \"coin\"".into());
            let (_, after) = import_ok(&case, &options);
            let mut value = input::parse(&after, format_of(&path)).unwrap();
            value.as_object_mut().unwrap().remove("$schema");
            value
        })
        .collect();
    assert_eq!(results[0], results[1]);
    assert_eq!(results[1], results[2]);
}

// ------------------------------------------------------- checks, writing

#[test]
fn invalid_original_is_reported_like_validate() {
    let case = Case::from_fixture(
        &fixture("invalid/unknown-palette.toml"),
        &rgba_png(&small()),
    );
    match import::run(&case.options("coin")) {
        Err(ImportError::InvalidWorksheet { diagnostics }) => {
            assert!(
                diagnostics
                    .iter()
                    .any(|d| d.to_string().contains("/sprites/0/_palette")),
                "{diagnostics:?}"
            );
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn wrong_insertion_is_caught_by_the_model_check() {
    fn sloppy(text: &str, format: Format, adds: &[Addition]) -> Result<String, edit::EditError> {
        // Adds a description nobody asked for: valid, but a different model.
        let mut adds = adds.to_vec();
        for add in &mut adds {
            for item in &mut add.items {
                if let Node::Map { entries, .. } = item {
                    entries.push(("description".into(), Node::Str("surprise".into())));
                }
            }
        }
        edit::add(text, format, &adds)
    }
    let case = Case::from_fixture(&fixture("valid/readme-example.yaml"), &rgba_png(&small()));
    let before = case.text();
    let err = import::run_with(&case.options("coin"), sloppy)
        .unwrap_err()
        .to_string();
    assert!(err.contains("check 2"), "{err}");
    assert_eq!(case.text(), before);
}

#[test]
fn rewritten_text_is_caught_by_the_subsequence_check() {
    fn rewriter(text: &str, format: Format, adds: &[Addition]) -> Result<String, edit::EditError> {
        Ok(edit::add(text, format, adds)?.replace("# The README example", "# Rewritten"))
    }
    let case = Case::from_fixture(&fixture("valid/readme-example.yaml"), &rgba_png(&small()));
    let err = import::run_with(&case.options("coin"), rewriter)
        .unwrap_err()
        .to_string();
    assert!(err.contains("check 4"), "{err}");
}

#[test]
fn dry_run_prints_what_a_real_run_writes() {
    let case = Case::from_fixture(&fixture("valid/readme-example.yaml"), &rgba_png(&small()));
    let mut options = case.options("coin");
    options.dry_run = true;
    let (_, dry) = import_ok(&case, &options);
    options.dry_run = false;
    let (_, real) = import_ok(&case, &options);
    assert_eq!(dry, real);
}

#[cfg(unix)]
#[test]
fn writing_keeps_permissions_and_symlinks() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let case = Case::from_fixture(&fixture("valid/readme-example.yaml"), &rgba_png(&small()));
    fs::set_permissions(&case.ws, fs::Permissions::from_mode(0o640)).unwrap();
    let link = case.ws.with_file_name("link.yaml");
    symlink(&case.ws, &link).unwrap();
    import::run(&ImportOptions::new(&link, &case.png, "coin")).unwrap();
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(case.text().contains("id: coin"));
    assert_eq!(
        fs::metadata(&case.ws).unwrap().permissions().mode() & 0o777,
        0o640
    );
    assert_eq!(
        case.files(),
        ["art.png", "link.yaml", "readme-example.yaml"]
    );
}

#[test]
fn failed_import_leaves_content_and_mtime() {
    let case = Case::from_fixture(&fixture("valid/readme-example.yaml"), &rgba_png(&small()));
    let mtime = fs::metadata(&case.ws).unwrap().modified().unwrap();
    rejected(&case, &case.options("heart"));
    assert_eq!(fs::metadata(&case.ws).unwrap().modified().unwrap(), mtime);
}
