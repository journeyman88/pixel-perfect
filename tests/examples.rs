//! The worksheets in `examples/` stay valid and render with the encodings
//! their comments promise.

#[path = "common/images.rs"]
mod images;

use std::path::PathBuf;

use images::header;
use pixel_perfect::export::{self, OutputFormat, RenderOptions};

fn example(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join(name)
}

#[test]
fn sprites_example_covers_every_encoding() {
    let ws = pixel_perfect::load_file(&example("sprites.yaml")).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let options = RenderOptions {
        output_path: dir.path().to_owned(),
        format: OutputFormat::Png,
        scale: 1,
        safe_build: false,
    };
    let (plan, _) = export::render(&ws, &options).unwrap();
    assert!(plan.notices.is_empty());

    // (sprite, width, height, bit depth, color type: 3 indexed, 6 RGBA)
    let expected = [
        ("coin", 8, 8, 1, 3),
        ("heart", 7, 6, 2, 3),
        ("slime", 12, 9, 4, 3),
        ("slime-ice", 12, 9, 4, 3),
        ("sunset", 24, 24, 8, 3),
        ("spectrum", 16, 18, 8, 6),
    ];
    assert_eq!(plan.outputs.len(), expected.len());
    for (id, width, height, depth, color_type) in expected {
        let png = std::fs::read(dir.path().join(format!("sprites/{id}.png"))).unwrap();
        assert_eq!(header(&png), (width, height, depth, color_type), "{id}");
    }
}
