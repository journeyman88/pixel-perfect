// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 The pixel-perfect Authors

//! Typed model and resolution (task 4.3).

mod common;

use common::{descriptions, fixture, full, load, strip_descriptions};
use pixel_perfect::model::{Rgba, Transform, Transition};

#[test]
fn every_valid_fixture_loads() {
    let dir = fixture("valid");
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        pixel_perfect::load_file(&path)
            .unwrap_or_else(|e| panic!("{}: {e}: {:#?}", path.display(), e.diagnostics()));
    }
}

#[test]
fn alpha_conversion() {
    assert_eq!(Rgba::alpha_to_u8(0.0), 0);
    assert_eq!(Rgba::alpha_to_u8(0.5), 128);
    assert_eq!(Rgba::alpha_to_u8(1.0), 255);
    // Decimal halves round up even when binary floating point lands just below.
    assert_eq!(Rgba::alpha_to_u8(0.3), 77);
    assert_eq!(Rgba::alpha_to_u8(0.1), 26);
    assert_eq!(Rgba::alpha_to_u8(0.6), 153);
}

#[test]
fn palette_colors_are_8_bit_with_default_alpha() {
    let ws = load(full());
    let base = &ws.palettes["base"].colors;
    assert_eq!(
        base[&'r'],
        Rgba([255, 0, 0, 255]),
        "missing alpha is opaque"
    );
    assert_eq!(base[&'g'], Rgba([0, 255, 0, 255]));
    assert_eq!(base[&'w'], Rgba([255, 255, 255, 128]));
}

#[test]
fn derived_palette_overrides_and_adds() {
    let ws = load(full());
    let night = &ws.palettes["night"].colors;
    assert_eq!(night[&'r'], Rgba([80, 0, 0, 255]), "overridden");
    assert_eq!(night[&'g'], Rgba([0, 255, 0, 255]), "inherited");
    assert_eq!(night[&'b'], Rgba([0, 0, 255, 255]), "added");
    assert_eq!(ws.palettes["night-deep"], ws.palettes["night"]);
}

#[test]
fn derived_sprites_resolve_lines_and_palette() {
    let ws = load(full());
    let rows = |id: &str| -> Vec<String> {
        ws.sprites[id]
            .rows
            .iter()
            .map(|r| r.iter().collect())
            .collect()
    };
    let base = rows("block");
    let night = rows("block-night");
    assert_eq!(night[0], "rrbg", "`=` copies the base character");
    assert_eq!(night[7], "bbbb");
    assert_eq!(night[1..7], base[1..7]);
    assert_eq!(ws.sprites["block-night"].palette, "night");
    assert_eq!(rows("block-deep"), night);
    assert_eq!(ws.sprites["block-deep"].palette, "night-deep");
    assert_eq!(
        (ws.sprites["block"].width(), ws.sprites["block"].height()),
        (4, 8)
    );
}

#[test]
fn transforms_and_transitions_deserialize() {
    let ws = load(full());
    let frames = &ws.model.flipbooks[0].frames;
    assert!(matches!(
        frames[1].transforms[1],
        Transform::Crop { area: [0, 0, 8, 4] }
    ));
    assert!(matches!(
        frames[3].transforms[0],
        Transform::Scale {
            width: 8,
            height: 16
        }
    ));
    let transitions = &ws.model.timelines[0].transitions;
    assert!(matches!(
        transitions[0],
        Transition::MoveTo {
            start: 0,
            duration: 30,
            ..
        }
    ));
    assert_eq!(transitions[4].element(), "hero");
    let spinner = &ws.model.panels[0].elements[1];
    assert!(!spinner.visible);
    assert_eq!(ws.model.panels[0].elements[0].z_index, 2);
    assert_eq!(ws.model.panels[0].elements[2].z_index, 0, "default zIndex");
}

#[test]
fn descriptions_are_kept_in_the_model() {
    let ws = load(descriptions()).model;
    assert_eq!(
        ws.description.as_deref(),
        Some("Every annotatable object, annotated.")
    );
    assert_eq!(
        ws.palettes[0].colors[0].description.as_deref(),
        Some("Red.")
    );
    assert_eq!(
        ws.flipbooks[0].frames[0].description.as_deref(),
        Some("Upright.")
    );
    assert_eq!(
        ws.scenes[0].layers[0].description.as_deref(),
        Some("Still stage.")
    );
}

#[test]
fn transform_and_transition_descriptions_are_ignored() {
    // `Transform` and `Transition` have no `description` field: the annotated
    // and the stripped document must deserialize to the same values.
    let annotated = load(descriptions()).model;
    let mut doc = descriptions();
    strip_descriptions(&mut doc);
    let stripped = load(doc).model;

    let transforms = |ws: &pixel_perfect::model::Worksheet| -> Vec<Vec<Transform>> {
        let frames = ws
            .flipbooks
            .iter()
            .flat_map(|f| &f.frames)
            .map(|f| f.transforms.clone());
        let elements = ws
            .panels
            .iter()
            .flat_map(|p| &p.elements)
            .map(|e| e.transforms.clone());
        frames.chain(elements).collect()
    };
    let transitions = |ws: &pixel_perfect::model::Worksheet| -> Vec<Transition> {
        ws.timelines
            .iter()
            .flat_map(|t| t.transitions.clone())
            .collect()
    };
    assert!(transforms(&annotated).iter().any(|t| !t.is_empty()));
    assert_eq!(transforms(&annotated), transforms(&stripped));
    assert_eq!(transitions(&annotated).len(), 6);
    assert_eq!(transitions(&annotated), transitions(&stripped));
}

#[test]
fn sequence_fixture_loads_the_same_in_every_format() {
    use pixel_perfect::Format;
    use pixel_perfect::model::Action;
    let yaml = std::fs::read_to_string(common::fixture("valid/sequence.yaml")).unwrap();
    let value = pixel_perfect::input::parse(&yaml, Format::Yaml).unwrap();
    let json = serde_json::to_string_pretty(&value).unwrap();
    let toml = toml::to_string(&value).unwrap();
    let models: Vec<_> = [
        (yaml.as_str(), Format::Yaml),
        (&json, Format::Json),
        (&toml, Format::Toml),
    ]
    .into_iter()
    .map(|(text, format)| pixel_perfect::load_str(text, format).unwrap().model)
    .collect();
    assert_eq!(models[0], models[1]);
    assert_eq!(models[1], models[2]);

    let model = &models[0];
    let seq = &model.sequences[0];
    assert_eq!(
        (seq.id.as_str(), seq.scene.as_str(), seq.fps, seq.duration),
        ("drift", "valley", 4, 8)
    );
    assert_eq!(
        seq.actions[0],
        Action::MoveTo {
            layer: "far".into(),
            coordinates: [0, 1],
            start: 0,
            duration: 8
        }
    );
    assert_eq!(
        seq.actions[3],
        Action::MoveToZ {
            layer: "light".into(),
            value: 5,
            at: 6
        }
    );
    assert_eq!(seq.instant_order(), vec![2, 3]);
    let layers = &model.scenes[0].layers;
    assert_eq!(layers[0].id, None);
    assert!(layers[0].visible, "visible defaults to true");
    assert_eq!(layers[3].id.as_deref(), Some("light"));
    assert!(!layers[3].visible);
}
