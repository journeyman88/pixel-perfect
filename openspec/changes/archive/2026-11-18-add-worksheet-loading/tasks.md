## 1. Crate setup

- [x] 1.1 Create `Cargo.toml` (`name = "pixel-perfect"`, edition 2024, `license = "Apache-2.0"`, lib + bin) and `src/lib.rs`, `src/main.rs`
- [x] 1.2 Add module skeletons: `input`, `schema`, `model`, `resolve`, `semantic`, `error`
- [x] 1.3 Define the diagnostic type (file, location as JSON Pointer or line/column, message) with `thiserror`
- [x] 1.4 Add `tests/fixtures/valid/` and `tests/fixtures/invalid/`; copy the README example in as `valid/readme-example.json`

## 2. Multi-format input

- [x] 2.1 Detect the format from the extension (case-insensitive); reject unknown extensions with the list of supported ones
- [x] 2.2 Parse JSON into `serde_json::Value`, reporting line/column on syntax errors
- [x] 2.3 Choose a maintained YAML crate (`serde_norway` or `serde_yaml_ng`), parse into `Value`, convert integer keys to strings and reject other non-string keys
- [x] 2.4 Parse TOML into `Value`; reject date-times with their location
- [x] 2.5 Test: the README example written in JSON, YAML and TOML loads into equal `Value`s
- [x] 2.6 Test: YAML integer `overrideLines` key, TOML date-time, malformed JSON with its line number, unknown extension

## 3. Schema validation

- [x] 3.1 Embed `schema/worksheet.schema.json` with `include_str!` and build the validator once
- [x] 3.2 Collect every validation error with its instance path
- [x] 3.3 Test: the `\p{Cc}` pattern compiles and rejects a control character as `char`
- [x] 3.4 Test: one valid and one invalid fixture per `transform.kind` and per `transition.kind`, so the `unevaluatedProperties` behavior is covered
- [x] 3.5 Test: missing `id`, reserved `char` `.`, `lines` together with `_sprite`, element with both `_sprite` and `_flipbook`, several violations reported together

## 4. Typed model

- [x] 4.1 Define serde types for palette, color, sprite, transform, flipbook, element, panel, transition, timeline, layer, scene and exports
- [x] 4.2 Convert `rgba` to 8-bit RGBA with alpha `round_half_up(a * 255)`, default 255
- [x] 4.3 Test: every valid fixture deserializes; alpha 0.5 → 128, 0.0 → 0, 1.0 and missing → 255

## 5. Semantic validation

- [x] 5.1 Unique ids per collection and unique element ids per panel
- [x] 5.2 Reference resolution for `_palette`, `_sprite`, `_flipbook`, `_panel`, `_timeline` and transition `_element`
- [x] 5.3 Palette resolution in topological order with cycle detection; duplicate `char` within one `colors` array
- [x] 5.4 Sprite resolution in topological order with cycle detection; `_palette` replacement; `overrideLines` index/width checks and `=` pass-through
- [x] 5.5 Resolved grid checks: equal row widths (in `char`s), `width`/`height` match, every non-`.` character in the resolved palette
- [x] 5.6 Crop bounds along the declared `transforms` chains of flipbook frames and panel elements, tracking size through `rotate`/`scale`/`crop`
- [x] 5.7 `exports` pointers resolve
- [x] 5.8 Tests: one invalid fixture per scenario in `specs/worksheet-loading/spec.md`, each asserting the exact pointer reported; plus the pass-through and palette-swap scenarios

## 6. CLI

- [x] 6.1 `pixel-perfect validate <file>...` with `clap`; validate each file independently
- [x] 6.2 Print diagnostics to stderr as `<file>: <location>: <message>`; exit codes 0 / 1 / 2
- [x] 6.3 Integration tests (`assert_cmd` or `std::process::Command`): all valid → 0, one invalid → 1 with its errors, missing file → 2

## 7. Wrap-up

- [x] 7.1 `cargo clippy -- -D warnings` and `cargo fmt --check` pass
- [x] 7.2 Update the README status and add a short "Usage" section for `validate`
