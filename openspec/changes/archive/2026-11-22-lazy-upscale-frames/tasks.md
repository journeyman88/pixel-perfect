## 1. Encoders

- [x] 1.1 In `src/encode.rs`, add `apng_scaled(frames, scale)`:
  - the palette scan runs on the frames before scaling;
  - each frame's parallel job upscales its previous and current frame before `encoded_part`;
  - `apng(frames)` calls `apng_scaled(frames, 1)`.

  At scale 1 nothing is upscaled and nothing is copied.
- [x] 1.2 In `src/webp.rs`, the same with `animated_scaled(frames, scale)` and `animated(frames)`. The alpha check runs on the frames before scaling.

## 2. Export and plan

- [x] 2.1 `animation_bytes` in `src/export.rs` passes the merged frames before scaling, and `options.scale`, to `apng_scaled` or `animated_scaled`, with no upscaled vector in between.
- [x] 2.2 `plan_animation` bounds the change ticks with the frame size before scaling, and the error message names that size. The check of one final frame stays as it is.

## 3. Tests

- [x] 3.1 `tests/size_limit.rs`:
  - update `animation_limit_applies_after_scale` to the new rule (1000 frames of 32x32 at scale 16 are 3.9 MiB);
  - add a test for an animation accepted only by the new rule: a 256x256 timeline with about 380 ticks at scale 4, under 1 GiB.
- [x] 3.2 Add a CLI test: `examples/train.yaml` at `--scale 4` plans and renders under the default limit.
- [x] 3.3 Run `cargo test`. The golden files must compare byte-identical, with no `UPDATE_GOLDEN`.
- [x] 3.4 Render every example and valid fixture at scales 1, 3 and 4, as PNG and WebP, with the previous build and the new one, and compare every file byte for byte.

## 4. Measurements and checks

- [x] 4.1 Measure peak memory and wall time before and after, for `train` and `campfire` at scales 1 and 4, as PNG and WebP. Record the results in `design.md`.
- [x] 4.2 Run `cargo clippy --all-targets` and `cargo fmt --check`.
