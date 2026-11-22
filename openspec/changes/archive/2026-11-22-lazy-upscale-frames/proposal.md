## Why

`train` at `--scale 4` is rejected with "the animation needs more than 256
frames of 1024x1024 (over 1.00 GiB)", although the file it would write has
128 frames. The check follows the spec. An animation's size is "the number
of frames before merging times the size of one final frame", and `train` has
382 change ticks before identical frames are merged. 382 × 4 MiB is 1.49 GiB.

That rule estimates more memory than the engine uses. Frames before merging
are rendered in small batches, at their size before `--scale`, and merged at
once: they are never all held. What the engine holds is the merged frames,
all upscaled before encoding. For `train` at scale 4 that is 128 × 4 MiB, and
the measured peak is 553 MB, not 1.49 GiB.

## What Changes

- The merged frames of an animation stay at their size before `--scale`.
  Each frame is upscaled inside the parallel encoding, when it is encoded,
  for APNG and for animated WebP. `--single-frames` already works this way.
- Memory for animations drops accordingly. For `train` at scale 4 it is
  expected to go from about 550 MB to under 200 MB.
- The size limit for an animated resource checks two sizes:
  - one final frame (after `--scale`), as today;
  - the number of frames before merging times the size of one frame
    *before* `--scale`.

  At `--scale 1` this is the same rule as today. `train` at `--scale 4` then
  fits the default 1 GiB limit: 382 × 256 KiB is about 96 MiB.
- The output stays byte-identical: the encoders receive exactly the same
  upscaled images, only later.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `png-export`: *Image size limit* changes how an animated resource's size
  is computed, with a scenario for an animation that only fits because of
  the new rule.

## Impact

- `src/export.rs`:
  - `plan_animation` computes the frame bound from the size before scaling;
  - `animation_bytes` no longer upscales every frame, and passes the scale to
    the encoders.
- `src/encode.rs`: `apng_scaled(frames, scale)` upscales the previous and
  current frame inside each frame's parallel job. `apng(frames)` stays as
  `apng_scaled(frames, 1)`.
- `src/webp.rs`: the same, with `animated_scaled(frames, scale)` and
  `animated(frames)`.
- Tests:
  - `tests/size_limit.rs`: the scaled-animation test follows the new rule,
    and a new test covers an animation accepted only by it;
  - golden files and byte comparisons are unchanged.
