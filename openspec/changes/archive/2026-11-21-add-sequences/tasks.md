## 1. Schema and model

- [x] 1.1 Review the drafted schema blocks (`sequences`, `sequence`, `action`, layer `id`/`visible`, `exports` pattern) against the specs; add `$comment`s for every engine rule (layer id uniqueness, `_layer` resolution, action timing)
- [x] 1.2 Schema tests in `tests/schema.rs`: a valid sequence passes; `appendTransforms`/`seekFlipbook` kinds fail pointing to `kind`; missing `_layer`, `at` or `coordinates` fail; `/sequences/x` is an accepted export pattern; fixture `tests/fixtures/valid/sequence.yaml`
- [x] 1.3 Add `Sequence`, `Action`, `Layer.id`, `Layer.visible` (default true) and `Worksheet.sequences` to `model.rs`
- [x] 1.4 Model tests: the fixture deserializes in JSON, YAML and TOML to the same model; `visible` defaults to true

## 2. Shared state rules (refactor first)

- [x] 2.1 Extract the linear movement (`moved`) and instant ordering (`instant_order`) helpers from `timeline_state`/`position`; make timelines use them
- [x] 2.2 Verify no output changed: all existing tests, APNG/WebP golden files and example renders pass unmodified

## 3. Validation

- [x] 3.1 In `semantic.rs`: unique layer ids per scene; `_scene` resolves; `_layer` names a layer id of the scene; `exports` resolves `/sequences/<id>`
- [x] 3.2 Share the timing checks with timelines (`at`/`start` < `duration`, `moveTo` within the loop, no overlapping `moveTo` per target) through a function parameterized by pointer prefix and target name
- [x] 3.3 Tests in `tests/semantic.rs` (fixtures under `tests/fixtures/invalid/`): duplicate layer id; same layer id in two scenes is fine; unknown scene; unknown layer; `at` = `duration`; movement ending at the boundary is fine; movement past the end; overlapping movements; existing timeline checks still report the same pointers

## 4. Layer visibility

- [x] 4.1 Static scenes skip layers with `visible: false`; `layer_kinds` ignores hidden layers, so a scene whose only animated layer is hidden is static
- [x] 4.2 Tests: hidden layer over a background renders as the background alone; hidden timeline layer makes the scene static (written as PNG); hidden layer does not lengthen the scene loop

## 5. Sequence animation

- [x] 5.1 Implement `sequence_state(sequence, scene, t) -> Vec<LayerState>` on the shared helpers
- [x] 5.2 Unit tests: loop restarts from the scene; `setVisible` from `at`; same-`at` array order for `moveToZ`; moving in front of another layer; parallax positions at frame-unit 8; chained movements
- [x] 5.3 Add `Kind::Sequence`: rates and loop over drawable animated layers (visible, or targeted by `setVisible` true), size of the scene, `sequence_periodic` change ticks plus layer periodics, rendering at the global tick
- [x] 5.4 Tests in `tests/animate.rs`: clock with a 6 fps timeline layer and a 4 fps sequence is 12; loop with a 1.5 s layer is 12 s; never-drawn layer ignored; flicker keeps its rhythm at 4.5 s; shown-after-hidden layer shows its global time; two frames of 1 s for a single switch; frame durations sum to the loop

## 6. Export

- [x] 6.1 Add `sequences` to the export collections, default selection and `is_animated`; size guard uses the scene canvas; `Animation::new` accepts `sequences`
- [x] 6.2 Push the loop-length notice when the loop exceeds the sequence's own length, with exact `n` or `n/d` seconds
- [x] 6.3 Tests in `tests/export.rs` and `tests/cli.rs`: `sequences/<id>.png` is APNG with the scene size and a manifest entry with `frames` and `loop`; same as animated WebP; implicit export includes sequences; notice text and exit code 0; no notice when the loop matches
- [x] 6.4 Golden files in `tests/golden.rs` for a sequence (APNG and WebP), regenerated with `UPDATE_GOLDEN=1`; a second render is byte-identical

## 7. Example and docs

- [x] 7.1 Write `examples/parallax.yaml`: static sky, three tiled planes moving at different speeds for a seamless loop, and a light layer toggled with `setVisible` and lifted with `moveToZ`; header comment listing outputs
- [x] 7.2 Extend `tests/examples.rs` to render it and check it is an APNG of the scene size with the expected loop
- [x] 7.3 README: sequences in Concepts and the Animation section (global time, loop notice, layer `visible`); `examples/README.md` entry; note the layer `visible` behavior change
