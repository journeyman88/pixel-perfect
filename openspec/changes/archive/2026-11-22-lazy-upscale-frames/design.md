## Context

For an animated resource, `export::animation_bytes` does three things:
- it gets the merged frames from `Animation::frames()` (APNG) or
  `Animation::ms_frames()` (WebP), at their size before `--scale`;
- it upscales all of them in parallel into a new vector;
- it hands that vector to `encode::apng` or `webp::animated`, whose parallel
  per-frame jobs read the previous and the current frame.

Rendering merges in batches of one segment per thread, so the frames before
merging are never all held. Peak memory is therefore about the merged
frames, at their final size.

`plan_animation` bounds the change ticks by `max_image_size /
(final_width × final_height × 4)`, so the bound shrinks by scale². That is why
`train` (382 ticks, 128 merged frames, 256 × 256) is rejected at scale 4 under
1 GiB. Measured with `--max-image-size none`, it peaks at 553 MB.

`--single-frames` (`write_frames`) already upscales each frame inside its
encoding job.

## Goals / Non-Goals

**Goals:**
- Animations hold their frames at their size before `--scale`, and upscale
  each frame only inside its encoding job.
- The size check for animations matches that: one final frame, and the
  frames before merging at their size before `--scale`.
- Byte-identical output.

**Non-Goals:**
- Computing frame differences on the images before scaling, and upscaling
  only the changed area. It would save more work, but the 1x1 unchanged
  rectangle and WebP's even alignment do not scale exactly, so byte identity
  would need care. It can come later.
- Changing static images: one image, already checked after scaling.

## Decisions

### 1. Encoders take the frames before scaling, and a scale

`encode::apng_scaled(frames, scale)` and `webp::animated_scaled(frames,
scale)` take frames before scaling. In each frame's parallel job they upscale
the previous and the current frame. The rest of the job is unchanged: it
compares the two upscaled images and crops, masks and compresses them. The
images are the same as before, so the bytes are the same.

The palette scan (`choose_all`) and the alpha check (`has_alpha`) run on the
frames before scaling, since upscaling adds no colors. `apng(frames)` and
`animated(frames)` stay, as the same calls with scale 1, so the existing
callers and tests keep working.

*Alternative considered:* upscaling each frame once, in order, and sharing
it between its own job and the next one. This saves one upscale per frame,
but it brings back ordering between jobs and keeps more frames alive.
Upscaling is a cheap copy next to compression. Rejected.

### 2. Two checks in `plan_animation`

- **One final frame**, `final_width × final_height × 4`, against the limit.
  The size walk already reports this.
- **The frames before merging, at their size before scaling.** The change
  ticks are counted with the bound `max_image_size / (width × height × 4)`,
  where width and height are taken before scaling. The error message says
  "frames of W x H before scaling", so the numbers read correctly.

The few upscaled frames in flight, about two per thread plus a crop, are not
counted. `--max-image-size` is documented as "a guard against mistakes, not
a memory budget", and those frames are already bounded by the per-frame
check.

### 3. `--single-frames` unchanged

It already upscales each frame inside its job. It shares the new plan check.

## Risks / Trade-offs

- [Each frame is upscaled twice, as the current frame of its job and as the
  previous frame of the next] → It is a pixel copy. The benchmark must show
  no slowdown at scale 1 (no upscale happens) and a reasonable cost at
  scale 4.
- [The plan now accepts animations that used to be rejected] → Peak memory
  is measured before and after on `train` at scale 4. The new check is still
  an upper bound of what is held, apart from a few frames per thread.

## Measurements

These are the results after implementation. The previous build is `7eb3d0b`
and the new one has this change. Both were built in release mode and run with
their default of 8 worker threads, one per physical core, on an AMD Ryzen AI
7 350 (8 cores, 16 threads). This was checked by counting the process
threads during a render. Each figure is the median of 7 renders, alternating
the two builds. Every run used `--max-image-size none`, because the previous
build rejects `train` at `--scale 4`; the limit does not change the files.

| Case | Before | After | Peak memory |
|---|---|---|---|
| campfire ×1, PNG | 32 ms, 124 ms CPU | 32 ms, 125 ms CPU | 25 → 25 MB |
| campfire ×1, WebP | 18 ms, 66 ms CPU | 18 ms, 63 ms CPU | 25 → 25 MB |
| campfire ×4, PNG | 88 ms, 394 ms CPU | 90 ms, 522 ms CPU | 132 → **97 MB** |
| campfire ×4, WebP | 62 ms, 317 ms CPU | 71 ms, 391 ms CPU | 146 → **96 MB** |
| train ×1, PNG | 44 ms, 192 ms CPU | 44 ms, 189 ms CPU | 53 → 49 MB |
| train ×1, WebP | 25 ms, 115 ms CPU | 23 ms, 107 ms CPU | 57 → 54 MB |
| train ×4, PNG | 275 ms, 1247 ms CPU | **234 ms**, 1645 ms CPU | 553 → **134 MB** |
| train ×4, WebP | 189 ms, 1245 ms CPU | 209 ms, 1506 ms CPU | 591 → **134 MB** |

- Peak memory at `--scale 4` drops by 26–77%. At `--scale 1` nothing is
  upscaled or copied, and the cost is unchanged.
- CPU time at `--scale 4` grows by 20–32%. Each frame is upscaled twice: as
  the current frame of its own job, and as the previous frame of the next
  job. The risk listed above was real. Wall time stays close, from −15% for
  `train` PNG to +14% for `campfire` WebP. Upscaling each frame once and
  sharing it between neighbouring jobs (the alternative rejected in
  decision 1), or computing the differences before scaling (a non-goal),
  would win that CPU back.
- The output is byte-identical: 120 renders, covering every example and
  valid fixture at scales 1, 3 and 4 as PNG and WebP (84 APNG files and 84
  animated WebP files), and the golden files.
- `train` at `--scale 4` now plans and renders under the default 1 GiB
  limit. Its 382 frames before merging are counted at 256 KiB each.
