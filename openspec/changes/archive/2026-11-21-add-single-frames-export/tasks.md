## 1. Options and CLI

- [x] 1.1 Add `single_frames: bool` to `RenderOptions`. Update every literal construction in `src/` and `tests/` so that it sets `single_frames: false`.
- [x] 1.2 Add `--single-frames` to the `render` subcommand in `src/main.rs`, with a one-line help text, and pass it to `RenderOptions`.
- [x] 1.3 Test: `pixel-perfect render … --single-frames` exits with 0, writes frames under `flipbooks/<id>/`, and the manifest has `options.singleFrames` true (`tests/cli.rs`).

## 2. Frames with exact durations

- [x] 2.1 Add `Animation::exact_frames()` in `src/animate.rs`. It is `merged` over the segment lengths, with an unbounded merge.
- [x] 2.2 Test: for flipbook `spin` (`fps` 12, durations 2, 2, 1, 1) the durations are 2, 2, 1, 1 ticks on a clock of 12. For a flipbook showing `a`, `a`, `b` (1, 2 and 1 units at `fps` 10) there are 2 frames, of 3 and 1 units.
- [x] 2.3 Test: a hold longer than one APNG delay stays a single frame in `exact_frames()`, and the durations add up to `loop_len`.

## 3. Planning

- [x] 3.1 Add `FrameCollection` to `EntryKind`. Add `kind: EntryKind` to `PlannedOutput` and set it from `is_animated` and `options.single_frames`. For a frame collection, `path` is the directory `<collection>/<name>`.
- [x] 3.2 Skip the WebP millisecond bound and `delay_overflow` in `plan_animation` when `single_frames` is set. Keep the size bound and the clock and loop checks.
- [x] 3.3 Tests:
  - a frame-collection output has the directory path, also with `--safe-build`;
  - static outputs do not change;
  - an animation that fails `delay_overflow` without the flag plans successfully with it;
  - the size limit still rejects a too-long animation with the flag.

## 4. Writing and manifest

- [x] 4.1 Manifest types:
  - `ManifestEntry.path` becomes `Option<String>`;
  - add `frames: Option<Vec<ManifestFrame>>`, with `ManifestFrame { index, path, duration }`;
  - field order is `id, kind, path, width, height, frameCount, frames, loop`;
  - add `single_frames` to `ManifestOptions`, always serialized;
  - `ManifestEntry::new` takes `kind` from the output.
- [x] 4.2 In `write()`, for `FrameCollection`:
  - upscale each frame of `exact_frames()`;
  - encode it with `encode::png` or `webp::still`;
  - write it to `<dir>/<i>.<ext>`, with `i` zero-padded to `max(4, digits of the last index)`;
  - fill `frames` with the exact durations, as `time::reduced`.
- [x] 4.3 Test: the "Frame collection entry" scenario. Flipbook `spin` gives the exact entry, with 4 frames, durations 1/6, 1/6, 1/12, 1/12 and `loop` 1/2, and the manifest is valid against the schema.
- [x] 4.4 Tests:
  - PNG frames have no `acTL` chunk and are indexed;
  - WebP frames have no `ANIM` chunk;
  - each decoded frame equals the corresponding `exact_frames()` image after `--scale`.
- [x] 4.5 Put the frame name in a helper, `frame_name(index, count)`, and test it:
  - with 4 frames, the names are `0000` to `0003`;
  - with 10000 frames, the last name is `9999`;
  - with 10001 frames, the names are `00000` to `10000`;
  - with 12345 frames, the last name is `12344`.

  It is a unit test, so no animation with 12345 frames has to be rendered.
- [x] 4.6 Update the "Manifest content" test and every assertion on the whole `options` object so they include `"singleFrames": false`.
- [x] 4.7 Shared color space for PNG frames:
  - add `encode::png_with(image, encoding)`, with `encode::png` calling it;
  - in `write_frames`, choose the encoding once with `choose_all` over all the frames.
- [x] 4.8 Tests:
  - the 4 frames of `spin` are indexed, with identical `PLTE` (4 colors) and bit depth 2;
  - two frames of 144 different colors each (288 in total) are both RGBA.
- [x] 4.9 Regenerate the golden files and check that only the PNG frames in `animation-frames` change.

## 5. Golden files and examples

- [x] 5.1 Add the golden directories `animation-frames` (PNG) and `animation-frames-webp`, rendered from `tests/fixtures/valid/animation.json` with `--single-frames`, to `tests/golden.rs`.
- [x] 5.2 Regenerate with `UPDATE_GOLDEN=1 cargo test --test golden`. Check that only `manifest.json` changes in the existing golden directories, and update the expected count in `golden_manifests_match_the_schema`.
- [x] 5.3 Add a single-frames render of an animated example to `tests/examples.rs`, through the schema-checking `render` helper.

## 6. Documentation

- [x] 6.1 README:
  - add `--single-frames` to the options table;
  - add a short section on the frame layout and the `frameCollection` manifest entry, with an example;
  - say that frames must be read from the manifest, because stale frames from earlier renders are not deleted.
- [x] 6.2 Run `cargo test`, `cargo clippy --all-targets` and `cargo fmt --check`. Render an example with `--single-frames` by hand and check the manifest against the schema.
