// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 The pixel-perfect Authors

//! The `validate`, `render` and `import` commands.

mod common;
#[path = "common/images.rs"]
mod images;

use assert_cmd::Command;
use common::fixture;

fn cli() -> Command {
    Command::cargo_bin("pixel-perfect").unwrap()
}

#[test]
fn all_files_valid() {
    cli()
        .arg("validate")
        .arg(fixture("valid/readme-example.json"))
        .arg(fixture("valid/readme-example.yaml"))
        .assert()
        .code(0)
        .stderr("");
}

#[test]
fn one_invalid_file() {
    let out = cli()
        .arg("validate")
        .arg(fixture("valid/readme-example.json"))
        .arg(fixture("invalid/unknown-palette.toml"))
        .assert()
        .code(1)
        .get_output()
        .clone();
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.contains("unknown-palette.toml: /sprites/0/_palette: unknown palette \"missing\""),
        "{stderr}"
    );
    assert!(!stderr.contains("readme-example"), "{stderr}");
}

#[test]
fn syntax_error_is_reported_with_line_and_column() {
    let out = cli()
        .arg("validate")
        .arg(fixture("invalid/malformed.json"))
        .assert()
        .code(1)
        .get_output()
        .clone();
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("malformed.json: 12:"), "{stderr}");
}

#[test]
fn missing_file() {
    cli()
        .arg("validate")
        .arg(fixture("valid/missing.json"))
        .assert()
        .code(2);
}

#[test]
fn unsupported_extension() {
    cli()
        .arg("validate")
        .arg(fixture("invalid/unsupported.txt"))
        .assert()
        .code(2);
}

#[test]
fn usage_errors() {
    cli().assert().code(2);
    cli().arg("validate").assert().code(2);
    cli().arg("--help").assert().code(0);
}

// render

fn worksheet_file(dir: &std::path::Path, text: &str) -> std::path::PathBuf {
    let path = dir.join("sheet.json");
    std::fs::write(&path, text).unwrap();
    path
}

const SHEET: &str = r#"{
    "palettes": [{ "id": "p", "colors": [{ "char": "r", "rgba": [255, 0, 0] }] }],
    "sprites": [{ "id": "hero", "_palette": "p", "lines": ["r.", ".r"] }]
}"#;

#[test]
fn render_writes_files_and_manifest() {
    let dir = tempfile::tempdir().unwrap();
    let sheet = worksheet_file(dir.path(), SHEET);
    let out = dir.path().join("out");
    cli()
        .arg("render")
        .arg(&sheet)
        .arg("--output-path")
        .arg(&out)
        .assert()
        .code(0);
    assert!(out.join("sprites/hero.png").is_file());
    assert!(out.join("manifest.json").is_file());
}

#[test]
fn render_short_output_flag_scale_and_safe_build() {
    let dir = tempfile::tempdir().unwrap();
    let sheet = worksheet_file(dir.path(), SHEET);
    let out = dir.path().join("out");
    cli()
        .arg("render")
        .arg(&sheet)
        .arg("-o")
        .arg(&out)
        .args(["--scale", "3", "--safe-build"])
        .assert()
        .code(0);
    let png = std::fs::read(out.join("sprites/8737b11b474b218b.png")).unwrap();
    let ihdr = &png[16..24];
    assert_eq!(ihdr, [0, 0, 0, 6, 0, 0, 0, 6], "2x2 sprite scaled by 3");
}

#[test]
fn render_default_output_path_is_the_current_directory() {
    let dir = tempfile::tempdir().unwrap();
    let sheet = worksheet_file(dir.path(), SHEET);
    cli()
        .current_dir(dir.path())
        .arg("render")
        .arg(&sheet)
        .assert()
        .code(0);
    assert!(dir.path().join("sprites/hero.png").is_file());
}

#[test]
fn render_usage_errors() {
    let dir = tempfile::tempdir().unwrap();
    let sheet = worksheet_file(dir.path(), SHEET);
    for args in [["--format", "gif"], ["--scale", "0"], ["--scale", "65"]] {
        cli()
            .arg("render")
            .arg(&sheet)
            .args(args)
            .current_dir(dir.path())
            .assert()
            .code(2);
    }
    assert!(!dir.path().join("sprites").exists());
}

#[test]
fn render_invalid_worksheet_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out");
    cli()
        .arg("render")
        .arg(fixture("invalid/unknown-palette.toml"))
        .arg("-o")
        .arg(&out)
        .assert()
        .code(1);
    assert!(!out.exists());
}

#[test]
fn render_readme_example() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out");
    cli()
        .arg("render")
        .arg(fixture("valid/readme-example.json"))
        .arg("-o")
        .arg(&out)
        .args(["--scale", "8"])
        .assert()
        .code(0)
        .stderr("");
    for name in ["heart", "heart-dark"] {
        let png = std::fs::read(out.join(format!("sprites/{name}.png"))).unwrap();
        assert_eq!(
            &png[16..24],
            [0, 0, 0, 56, 0, 0, 0, 48],
            "{name}: 7x6 scaled by 8"
        );
    }
}

#[test]
fn render_writes_animations() {
    let dir = tempfile::tempdir().unwrap();
    let mut doc: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(fixture("valid/readme-example.json")).unwrap(),
    )
    .unwrap();
    doc["exports"]
        .as_array_mut()
        .unwrap()
        .push("/flipbooks/beat".into());
    let sheet = worksheet_file(dir.path(), &doc.to_string());
    let out = dir.path().join("out");
    cli()
        .arg("render")
        .arg(&sheet)
        .arg("-o")
        .arg(&out)
        .assert()
        .code(0);
    let apng = std::fs::read(out.join("flipbooks/beat.png")).unwrap();
    assert!(apng.windows(4).any(|w| w == b"acTL"), "beat is an APNG");
}

#[test]
fn render_without_exports_prints_no_notice() {
    let dir = tempfile::tempdir().unwrap();
    let mut doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(fixture("valid/full.json")).unwrap())
            .unwrap();
    doc.as_object_mut().unwrap().remove("exports");
    let sheet = worksheet_file(dir.path(), &doc.to_string());
    let output = cli()
        .arg("render")
        .arg(&sheet)
        .arg("-o")
        .arg(dir.path().join("out"))
        .assert()
        .code(0)
        .get_output()
        .clone();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(!stderr.contains("skipped"), "{stderr}");
    assert!(dir.path().join("out/timelines/walk.png").exists());
    assert!(dir.path().join("out/scenes/night.png").exists());
}

#[test]
fn render_max_image_size_values() {
    let dir = tempfile::tempdir().unwrap();
    let sheet = worksheet_file(dir.path(), SHEET);
    for value in ["1048576", "512MiB", "2gib", "none"] {
        cli()
            .arg("render")
            .arg(&sheet)
            .arg("-o")
            .arg(dir.path().join("ok"))
            .args(["--max-image-size", value])
            .assert()
            .code(0);
    }
    for value in ["0", "1GB", "1.5GiB", "big", "-1"] {
        cli()
            .arg("render")
            .arg(&sheet)
            .arg("-o")
            .arg(dir.path().join("bad"))
            .args(["--max-image-size", value])
            .assert()
            .code(2);
    }
    assert!(!dir.path().join("bad").exists());
}

#[test]
fn render_over_the_size_limit_is_an_invalid_plan() {
    let dir = tempfile::tempdir().unwrap();
    let sheet = worksheet_file(dir.path(), SHEET);
    let out = dir.path().join("out");
    let output = cli()
        .arg("render")
        .arg(&sheet)
        .arg("-o")
        .arg(&out)
        .args(["--max-image-size", "15"])
        .assert()
        .code(1)
        .get_output()
        .clone();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr
            .contains("/sprites/0: sprite \"hero\" is 2x2 (16 B), over the 15 B image size limit"),
        "{stderr}"
    );
    assert!(!out.exists());
}

#[test]
fn render_webp() {
    let dir = tempfile::tempdir().unwrap();
    let sheet = worksheet_file(dir.path(), SHEET);
    let out = dir.path().join("out");
    cli()
        .arg("render")
        .arg(&sheet)
        .arg("-o")
        .arg(&out)
        .args(["--format", "webp"])
        .assert()
        .code(0);
    let webp = std::fs::read(out.join("sprites/hero.webp")).unwrap();
    assert_eq!(&webp[0..4], b"RIFF");
    assert_eq!(&webp[8..16], b"WEBPVP8L");
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["options"]["format"], "webp", "{manifest}");
}

#[test]
fn render_webp_side_limit_is_an_invalid_request() {
    let dir = tempfile::tempdir().unwrap();
    let lines = vec![format!("\"{}\"", "r".repeat(300)); 300].join(", ");
    let sheet = worksheet_file(
        dir.path(),
        &format!(
            r#"{{
    "palettes": [{{ "id": "p", "colors": [{{ "char": "r", "rgba": [255, 0, 0] }}] }}],
    "sprites": [{{ "id": "big", "_palette": "p", "lines": [{lines}] }}]
}}"#
        ),
    );
    let out = dir.path().join("out");
    let output = cli()
        .arg("render")
        .arg(&sheet)
        .arg("-o")
        .arg(&out)
        .args([
            "--format",
            "webp",
            "--scale",
            "64",
            "--max-image-size",
            "none",
        ])
        .assert()
        .code(1)
        .get_output()
        .clone();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("too large for WebP"), "{stderr}");
    assert!(!out.exists());
}

// ------------------------------------------------------------------ import

/// A copy of `worksheet` and a PNG of a small image in a temporary directory.
fn import_case(
    worksheet: &str,
    png: &[u8],
) -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
    let dir = tempfile::TempDir::new().unwrap();
    let src = fixture(worksheet);
    let ws = dir.path().join(src.file_name().unwrap());
    std::fs::copy(&src, &ws).unwrap();
    let png_path = dir.path().join("art.png");
    std::fs::write(&png_path, png).unwrap();
    (dir, ws, png_path)
}

fn coin() -> pixel_perfect::raster::Image {
    use pixel_perfect::model::Rgba;
    pixel_perfect::raster::Image::from_fn(8, 8, |x, y| {
        if (x + y) % 3 == 0 {
            Rgba([240, 190, 40, 255])
        } else {
            Rgba([0, 0, 0, 0])
        }
    })
}

#[test]
fn import_prints_the_summary() {
    let (_dir, ws, png) = import_case("valid/readme-example.yaml", &images::rgba_png(&coin()));
    cli()
        .arg("import")
        .arg(&ws)
        .arg(&png)
        .args(["--id", "coin"])
        .assert()
        .code(0)
        .stdout(format!(
            "{}: added /palettes/coin, /sprites/coin (8\u{d7}8, 1 color)\n",
            ws.display()
        ))
        .stderr("");
}

#[test]
fn import_dry_run_prints_the_worksheet() {
    let (_dir, ws, png) = import_case("valid/readme-example.yaml", &images::rgba_png(&coin()));
    let before = std::fs::read_to_string(&ws).unwrap();
    let out = cli()
        .arg("import")
        .arg(&ws)
        .arg(&png)
        .args(["--id", "coin", "--dry-run"])
        .assert()
        .code(0)
        .get_output()
        .clone();
    assert_eq!(std::fs::read_to_string(&ws).unwrap(), before);
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.starts_with(&before[..before.find("palettes:").unwrap()]));
    assert!(stdout.contains("  - id: coin\n"));
    assert!(
        String::from_utf8(out.stderr)
            .unwrap()
            .contains("added /palettes/coin")
    );
}

#[test]
fn import_rejection_exits_1() {
    let (_dir, ws, png) = import_case("valid/readme-example.yaml", &images::rgba_png(&coin()));
    let out = cli()
        .arg("import")
        .arg(&ws)
        .arg(&png)
        .args(["--id", "heart"])
        .assert()
        .code(1)
        .get_output()
        .clone();
    assert!(
        String::from_utf8(out.stderr)
            .unwrap()
            .contains("sprite \"heart\" already exists")
    );
    // A JPEG is rejected too.
    std::fs::write(&png, [0xFF, 0xD8, 0xFF, 0xE0]).unwrap();
    let out = cli()
        .arg("import")
        .arg(&ws)
        .arg(&png)
        .args(["--id", "x"])
        .assert()
        .code(1)
        .get_output()
        .clone();
    assert!(
        String::from_utf8(out.stderr)
            .unwrap()
            .contains("art.png: not a PNG file")
    );
}

#[test]
fn import_usage_errors_exit_2() {
    let (dir, ws, png) = import_case("valid/readme-example.yaml", &images::rgba_png(&coin()));
    let before = std::fs::read_to_string(&ws).unwrap();
    let run = |args: &[&str]| {
        cli()
            .arg("import")
            .arg(&ws)
            .arg(&png)
            .args(args)
            .assert()
            .code(2);
    };
    run(&["--id", "a", "--palette", "warm", "--palette-id", "b"]);
    run(&["--id", "a", "--max-colors", "0"]);
    run(&["--id", "a", "--max-colors", "2000000"]);
    run(&["--id", "a", "--scale", "65"]);
    assert_eq!(std::fs::read_to_string(&ws).unwrap(), before);
    let xml = dir.path().join("art.xml");
    std::fs::write(&xml, "<worksheet/>").unwrap();
    cli()
        .arg("import")
        .arg(&xml)
        .arg(&png)
        .args(["--id", "a"])
        .assert()
        .code(2);
    assert_eq!(std::fs::read_to_string(&xml).unwrap(), "<worksheet/>");
    cli()
        .arg("import")
        .arg(&ws)
        .arg(dir.path().join("missing.png"))
        .args(["--id", "a"])
        .assert()
        .code(2);
}

#[test]
fn import_then_render_gives_back_the_png() {
    for scale in [1u32, 4] {
        let png = images::rgba_png_scaled(&coin(), scale);
        let (dir, ws, png_path) = import_case("../../examples/sprites.yaml", &png);
        cli()
            .arg("import")
            .arg(&ws)
            .arg(&png_path)
            .args(["--id", "imported", "--scale", &scale.to_string()])
            .assert()
            .code(0);
        let out = dir.path().join("out");
        cli()
            .arg("render")
            .arg(&ws)
            .arg("-o")
            .arg(&out)
            .assert()
            .code(0);
        let rendered = std::fs::read(out.join("sprites/imported.png")).unwrap();
        assert_eq!(
            images::decode(&rendered).2,
            coin().pixels(),
            "scale {scale}"
        );
    }
}

#[test]
fn render_prints_the_sequence_loop_notice() {
    let dir = tempfile::TempDir::new().unwrap();
    let text = std::fs::read_to_string(fixture("valid/sequence.yaml"))
        .unwrap()
        // The flicker timeline now loops every 1.5 s; the sequence lasts 2 s.
        .replace(
            "    fps: 6\n    duration: 3\n",
            "    fps: 6\n    duration: 9\n",
        );
    let ws = dir.path().join("seq.yaml");
    std::fs::write(&ws, text).unwrap();
    let out = cli()
        .arg("render")
        .arg(&ws)
        .arg("-o")
        .arg(dir.path().join("out"))
        .assert()
        .code(0)
        .get_output()
        .clone();
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert_eq!(
        stderr,
        format!(
            "{}: note: /sequences/drift loops every 6 s (its own duration is 2 s) because of its animated layers\n",
            ws.display()
        )
    );
    // Without the longer layer, nothing is printed.
    cli()
        .arg("render")
        .arg(fixture("valid/sequence.yaml"))
        .arg("-o")
        .arg(dir.path().join("out2"))
        .assert()
        .code(0)
        .stderr("");
}

#[test]
fn render_single_frames() {
    let dir = tempfile::tempdir().unwrap();
    let sheet = worksheet_file(
        dir.path(),
        r#"{
    "palettes": [{ "id": "p", "colors": [{ "char": "r", "rgba": [255, 0, 0] }, { "char": "g", "rgba": [0, 255, 0] }] }],
    "sprites": [{ "id": "a", "_palette": "p", "lines": ["r"] }, { "id": "b", "_palette": "p", "lines": ["g"] }],
    "flipbooks": [{ "id": "blink", "width": 1, "height": 1, "fps": 2, "frames": [
        { "_sprite": "a", "coordinates": [0, 0], "duration": 1 },
        { "_sprite": "b", "coordinates": [0, 0], "duration": 1 }
    ] }]
}"#,
    );
    let out = dir.path().join("out");
    cli()
        .arg("render")
        .arg(&sheet)
        .arg("-o")
        .arg(&out)
        .arg("--single-frames")
        .assert()
        .code(0);
    assert!(out.join("flipbooks/blink/0000.png").is_file());
    assert!(out.join("flipbooks/blink/0001.png").is_file());
    assert!(!out.join("flipbooks/blink.png").exists());
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["options"]["singleFrames"], true, "{manifest}");
}

#[test]
fn render_and_watch_accept_the_same_options() {
    let options = |command: &str| -> Vec<String> {
        let output = cli().args([command, "--help"]).output().unwrap();
        let help = String::from_utf8(output.stdout).unwrap();
        let mut flags: Vec<String> = help
            .split_whitespace()
            .filter(|w| w.starts_with("--") && w.len() > 2)
            .map(|w| w.trim_end_matches(',').to_owned())
            .collect();
        flags.sort();
        flags.dedup();
        flags
    };
    let render = options("render");
    for flag in [
        "--output-path",
        "--format",
        "--scale",
        "--safe-build",
        "--single-frames",
        "--max-image-size",
    ] {
        assert!(render.iter().any(|f| f == flag), "render lacks {flag}");
    }
    assert_eq!(options("watch"), render);
}

/// Every file under `dir` with its bytes, by relative path.
fn tree_bytes(dir: &std::path::Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    let mut out = std::collections::BTreeMap::new();
    let mut stack = vec![dir.to_owned()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let relative = path
                    .strip_prefix(dir)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                out.insert(relative, std::fs::read(&path).unwrap());
            }
        }
    }
    out
}

#[test]
fn output_does_not_depend_on_the_thread_count() {
    let dir = tempfile::tempdir().unwrap();
    let sheet = common::fixture("valid/animation.json");
    for format in ["png", "webp"] {
        for extra in [&[][..], &["--single-frames"][..]] {
            let outputs: Vec<_> = ["1", "4"]
                .iter()
                .map(|threads| {
                    let out = dir
                        .path()
                        .join(format!("{format}-{}-{threads}", extra.len()));
                    cli()
                        .arg("render")
                        .arg(&sheet)
                        .arg("-o")
                        .arg(&out)
                        .args(["--format", format, "--threads", threads])
                        .args(extra)
                        .assert()
                        .code(0);
                    tree_bytes(&out)
                })
                .collect();
            assert!(
                outputs[0].len() > 1,
                "{format} {extra:?}: something was written"
            );
            assert!(
                outputs[0] == outputs[1],
                "{format} {extra:?}: 1 and 4 threads differ"
            );
        }
    }
}

#[test]
fn zero_threads_is_a_usage_error() {
    cli()
        .args(["render", "hero.json", "--threads", "0"])
        .assert()
        .code(2);
}

#[test]
fn train_example_renders_at_scale_4_under_the_default_limit() {
    // 382 frames before merging, 128 in the file: counted before --scale
    // (256 KiB each) they fit the default 1 GiB, as final 1024x1024 frames
    // (4 MiB each) they would not.
    let dir = tempfile::tempdir().unwrap();
    let sheet = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/train.yaml");
    cli()
        .arg("render")
        .arg(&sheet)
        .arg("-o")
        .arg(dir.path())
        .args(["--scale", "4"])
        .assert()
        .code(0);
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.path().join("manifest.json")).unwrap())
            .unwrap();
    let ride = &manifest["files"][0];
    assert_eq!(ride["id"], "/sequences/ride");
    assert_eq!(
        (ride["width"].as_u64(), ride["height"].as_u64()),
        (Some(1024), Some(1024))
    );
    assert_eq!(ride["frameCount"], 128);
}
