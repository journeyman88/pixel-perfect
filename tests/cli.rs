//! The `validate` and `render` commands.

mod common;

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
fn render_unsupported_explicit_export_is_an_error() {
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
    let output = cli()
        .arg("render")
        .arg(&sheet)
        .arg("-o")
        .arg(&out)
        .assert()
        .code(1)
        .get_output()
        .clone();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("/exports/2"), "{stderr}");
    assert!(!out.exists());
}

#[test]
fn render_notice_for_implicitly_skipped_resources() {
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
    assert!(
        stderr.contains("note: skipped 1 resource(s) in flipbooks"),
        "{stderr}"
    );
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
