## 1. Compositing

- [x] 1.1 Add `compose::blend` with the integer source-over formula and `round(n, d) = (2n + d) div (2d)`
- [x] 1.2 Tests: every scenario of "Alpha compositing" in `specs/composition/spec.md`; result alpha 0 only when both alphas are 0
- [x] 1.3 Test: shortcuts for `sa = 255`, `sa = 0`, `da = 0` agree with the full formula on all 256 × 256 alpha pairs (sampled colors)
- [x] 1.4 Add `compose::draw` (placement at `i64` coordinates, clipping, blending only the overlap)
- [x] 1.5 Tests: placement, negative coordinates, partly and completely outside, image larger than the canvas

## 2. Panels and scenes

- [x] 2.1 Render panels: transparent canvas, visible elements sorted by `zIndex` (stable), element `transforms` before placement
- [x] 2.2 Tests: every "Panel rendering" and "Element placement and clipping" scenario, with fixtures under `tests/fixtures/valid/`
- [x] 2.3 Render scenes: each `_panel` layer flattened on its own canvas, then drawn by `zIndex` with clipping
- [x] 2.4 Tests: layer order, layer clipped to the scene, group rule (expected pixels computed by hand with the formula)
- [x] 2.5 Add `is_animated_panel` and `is_animated_scene`
- [x] 2.6 Tests: visible flipbook element, hidden flipbook element, `_timeline` layer, layer using an animated panel

## 3. Export

- [x] 3.1 Export selection: sprites, static panels and static scenes; animated ones and flipbooks/timelines rejected when explicit, skipped with a notice naming the reason when implicit
- [x] 3.2 Tests: every "Export selection" scenario, including the animated scene listed in `exports`; same id in `sprites` and `panels` written without conflict (completes the scenario left partial in `add-sprite-rendering`)
- [x] 3.3 `render::resource` dispatch so the encoder, `--scale`, manifest and safe build work unchanged for panels and scenes
- [x] 3.4 Tests: panel with semi-transparent elements on an opaque element gives the expected indexed palette; a composition with more than 256 blended colors gives RGBA

## 4. Image size limit

- [x] 4.1 Add `--max-image-size <SIZE>` (custom `clap` parser: integer bytes, `KiB`/`MiB`/`GiB`/`TiB` case-insensitive, `none`; default `1GiB`) and `RenderOptions::max_image_size`
- [x] 4.2 Tests: parser accepts `1048576`, `512MiB`, `2gib`, `none`; rejects `0`, `1GB`, `1.5GiB`, `big`, `-1` with exit code 2
- [x] 4.3 Plan-time size walk for every selected export: sprite raster, each transform step of every visible element, panel and scene canvases, final image × `scale²`; errors pointing to the resource or transform, with sizes and the `--max-image-size` hint
- [x] 4.4 Tests: every "Image size limit" scenario (planning only, no large allocation); limit also applies to sprites; nothing written on error

## 5. Wrap-up

- [x] 5.1 Golden files: regenerate `full` and `full-x4-safe` (now including `panels/stage`) and add a fixture with a static scene of semi-transparent layers
- [x] 5.2 `examples/composition.yaml`: a static scene (background panel, characters, a semi-transparent overlay) plus an entry in `examples/README.md`; covered by `tests/examples.rs`
- [x] 5.3 README: panels and scenes in Status and Render, the compositing rule in one paragraph, `--max-image-size` in the options table (noting it is a guard, not a memory budget)
- [x] 5.4 `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` pass
