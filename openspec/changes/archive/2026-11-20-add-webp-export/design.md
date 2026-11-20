## Context

`render` already has `--format` with `png` as its only value
(`main.rs` `Format`, `export::OutputFormat`). Export runs in two phases:
`export::plan` checks everything without touching the disk, `export::write`
renders and writes. Animated resources go through `Animation`
(`src/animate.rs`): `change_ticks` finds when frames may change on a common
clock, and `frames()` renders the segments and merges consecutive identical
ones *when the merged delay still fits APNG*. `time::delays` turns tick counts
into exact APNG fractions, and `plan_animation` rejects animations whose
delays do not fit APNG's 16-bit denominators.

`encode.rs` writes PNG/APNG through the `png` crate. There is no WebP code and
no C dependency.

Container facts this design relies on (WebP RIFF container spec):
- Simple lossless file: `RIFF` size `WEBP` + one `VP8L` chunk.
- Extended file: `VP8X` (flags, canvas width−1 and height−1 as 24-bit),
  `ANIM` (background BGRA, 16-bit loop count), then `ANMF` chunks (24-bit
  x/2, y/2, width−1, height−1, 24-bit duration in ms, flags byte with
  blending and disposal bits, then the frame's `VP8L` chunk).
- `VP8L` stores each side in 14 bits: at most 16384 pixels.
- Chunks are padded to an even size; the RIFF size is 32-bit.

## Goals / Non-Goals

**Goals:**
- `--format webp` for every exportable resource, static and animated, lossless,
  deterministic, pure Rust.
- Leave PNG/APNG output byte-identical (golden files unchanged).
- Report WebP-only problems (side over 16384) in the plan phase, before writing.

**Non-Goals:**
- Lossy WebP, quality settings, metadata (`ICCP`, `EXIF`, `XMP `).
- Inter-frame optimization (sub-rectangle frames, blending, disposal tricks).
- A libwebp backend. It stays the fallback named in the proposal if the size
  check shows WebP is not worth it.
- Writing both formats in one run.

## Decisions

**Encode bitstreams with `image-webp`, write the container ourselves.**
`image-webp` encodes one RGBA image to a lossless WebP file. The engine
encodes each image with it, takes the payload of the single `VP8L` chunk out
of the result (a strict parser that panics on anything else, since the
encoder's output is under our control), and wraps it:
- static: emit `RIFF`/`WEBP`/`VP8L` again from the payload, so the layout is
  ours even if the crate changes what it writes around the bitstream;
- animated: `VP8X` + `ANIM` + one `ANMF` per frame.
Alternatives: `webp-animation`/libwebp (C dependency, determinism tied to
libwebp settings), or our own VP8L encoder (large, error-prone). Rejected.

Encoder parameters are set explicitly rather than taken from `Default`, so a
change of defaults in a crate update shows up as a code change, not as a
silent output change.

**RGBA8 only when the file has alpha.** A `VP8L` header has its own
`alpha_is_used` bit, which `image-webp` sets from the input color type.
libwebp rejects an animation whose frames use alpha while `VP8X` says the file
has none ("Unexpected alpha data detected"). So the alpha of the whole file
is computed first (any pixel of any frame below 255), and every bitstream is
encoded as RGBA8 if it is set and as RGB8 otherwise; a still uses its own
pixels. (First planned as "always RGBA8"; changed after `webpinfo` rejected an
opaque animation.)

**New module `src/webp.rs`**, next to `encode.rs`: `still(&Image) -> Vec<u8>`
and `animated(&[WebpFrame]) -> Result<(Vec<u8>, usize), WebpError>` with
`WebpFrame { image, duration_ms }`.
Chunk writing is a few small functions (`chunk(fourcc, payload)` with padding,
24-bit little-endian helpers). `encode.rs` keeps PNG/APNG only.

**Share frame merging, keep the APNG rule.** `Animation::frames()` merges
identical frames only when the APNG delay fits, which is meaningless for
WebP. A private `Animation::merged()` renders the segment at each change tick,
skips segments whose duration is zero without rendering them, and merges a
frame into the previous identical one when a format-specific rule allows it.
Both lists are built on it:
- APNG: `frames()`, durations in ticks, today's rule (golden files prove the
  output is unchanged);
- WebP: `ms_frames()`, durations from `time::ms_durations(ends, clock)`, which
  rounds each end tick to ms (`(2 · 1000 · t + clock) / (2 · clock)`, `u128`
  arithmetic, halves up) and takes differences; merging has no limit.
Merging as segments are rendered keeps memory use as it is today (an unmerged
`segments()` list, as first planned, would hold every rendered segment).
Durations over 16777215 ms are split by the encoder into frames that borrow
the same image, as APNG does for long delays.

**Plan phase per format.** `plan_animation` runs the APNG delay check only for
`png`. For `webp` it skips it, and `plan` adds the 16384-pixel side check for
every output (static and animated), pointing at the resource like the size
limit does. `MAX_FRAMES` stays as the frame-count bound for both formats: it
also bounds the work done by `change_ticks`. For WebP the plan also checks
`Animation::ms_frame_bound` (frames after splitting, an upper bound), so a
very long still frame cannot expand into billions of `ANMF` chunks.

**Manifest.** No new fields: `format` is `"webp"`, paths end in `.webp`, and
`frames` counts the `ANMF` chunks written. `loop` stays the exact loop length,
not the rounded one — it describes the animation, not the encoding.

**Tests decode with `image-webp`.** It is already a dependency and decodes
animated WebP, so round-trip tests need nothing new. Container layout is
checked by a small chunk lister in `tests/common/images.rs`, like `chunks()`
for PNG. Golden `.webp` files sit next to the PNG ones. Because `image-webp`
decodes files that libwebp rejects, the golden WebP files are also checked
with libwebp's `webpinfo` when it is installed (the test skips otherwise).

## Risks / Trade-offs

- [WebP larger than indexed PNG for pixel art, especially animated, since
  every frame is a full canvas] → the size check task measures it on
  `examples/`; the README states the result. If WebP is never smaller, that is
  the trigger to reconsider libwebp, outside this change.
- [`image-webp` changes its output in a minor version] → determinism is
  defined per engine build; `Cargo.lock` pins it; golden files catch any
  change on update.
- [Very short frames: some viewers raise durations of a few ms to a minimum]
  → out of the engine's control; frames under 0.5 ms are already dropped. Noted
  in the README.
- [RIFF size limit (4 GiB) exceeded by a huge animation] → it can only be
  known after encoding; `webp::animated` returns an error and `write` reports
  it as a render error (exit 2). Files written earlier in the same run stay.
  With the default 1 GiB image size limit this needs a highly incompressible
  animation; accepted.
- [Rounded loop differs from the exact `loop` in the manifest by < 0.5 ms] →
  documented; the manifest keeps the exact value on purpose.

## Open Questions

- ~~Does `image-webp`'s lossless encoder use the color-indexing (palette)
  transform?~~ Resolved: it does not (0.2.4 uses subtract-green and
  predictor only). WebP comes out 1.05× to 4.2× larger than PNG on the
  examples; see `size-check.md`. The README recommends PNG. Sub-rectangle
  frames (for APNG and WebP) are the next change.
