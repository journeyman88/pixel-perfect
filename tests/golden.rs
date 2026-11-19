//! Golden files: rendering `valid/full.json` (exports removed, so every sprite
//! is exported) must reproduce the committed files byte for byte.
//!
//! Regenerate after an intended change with:
//! `UPDATE_GOLDEN=1 cargo test --test golden`

mod common;

use std::path::Path;

use pixel_perfect::export::{self, OutputFormat, RenderOptions};

fn render_to(dir: &Path, scale: u32, safe_build: bool) {
    let mut doc = common::full();
    doc.as_object_mut().unwrap().remove("exports");
    let ws = common::load(doc);
    let options = RenderOptions {
        output_path: dir.to_owned(),
        format: OutputFormat::Png,
        scale,
        safe_build,
    };
    export::render(&ws, &options).unwrap();
}

fn check(name: &str, scale: u32, safe_build: bool) {
    let golden = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(name);
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        let _ = std::fs::remove_dir_all(&golden);
        render_to(&golden, scale, safe_build);
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    render_to(dir.path(), scale, safe_build);
    let mut compared = 0;
    for sub in ["", "sprites"] {
        for entry in std::fs::read_dir(golden.join(sub)).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                continue;
            }
            let relative = path.strip_prefix(&golden).unwrap();
            let actual = std::fs::read(dir.path().join(relative))
                .unwrap_or_else(|_| panic!("{} was not produced", relative.display()));
            assert!(
                actual == std::fs::read(&path).unwrap(),
                "{} differs from golden",
                relative.display()
            );
            compared += 1;
        }
    }
    assert_eq!(compared, 4, "manifest + 3 sprites");
}

#[test]
fn full_fixture() {
    check("full", 1, false);
}

#[test]
fn full_fixture_scaled_safe_build() {
    check("full-x4-safe", 4, true);
}
