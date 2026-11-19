## 1. Schema and model

- [x] 1.1 Schema: add `tile` to the transform `kind` enum and its description, with a `then` branch requiring `width` and `height` (`$defs/size`), described as repeating the image from its top-left corner
- [x] 1.2 Model: `Transform::Tile { width, height }`
- [x] 1.3 Tests: schema accepts a `tile` transform on a flipbook frame, a panel element and inside `appendTransforms`; rejects missing `width`/`height`, `width: 0` and extra fields (`tests/schema.rs`, fixture `tests/fixtures/valid/full.json` gains a `tile`)

## 2. Rendering

- [x] 2.1 `raster::apply`: `tile` by modulo indexing; `raster::output_size`: `(width, height)`
- [x] 2.2 Tests: every "Tile transform" scenario of `specs/sprite-rendering/spec.md` (`tests/raster.rs`, `tests/animate.rs` for the tiled flipbook element)

## 3. Validation and size guard

- [x] 3.1 `semantic::walk_crops`: the size after `tile` is `(width, height)`
- [x] 3.2 Tests: "Crop after tile" and "Crop outside the tiled area" scenarios, plus an appended `tile` followed by an appended `crop` (`tests/semantic.rs`)
- [x] 3.3 Tests: a `tile` to 30000x30000 inside a 16x16 panel fails the default `--max-image-size` pointing to that transform, both on an element and in `appendTransforms` (`tests/size_limit.rs`)

## 4. Wrap-up

- [x] 4.1 Example: use `tile` for a background in `examples/animation.yaml` (e.g. a tiled ground strip) and update its golden expectations in `tests/examples.rs` if sizes or loops change
- [x] 4.2 README: add `tile` to the list of transforms
- [x] 4.3 `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` pass
