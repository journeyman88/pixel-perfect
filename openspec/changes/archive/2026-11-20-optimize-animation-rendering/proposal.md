## Why

Every APNG and animated WebP frame the engine writes covers the whole canvas,
even when only a few pixels change. The size check of `add-webp-export`
(archived `size-check.md`) shows the cost: the campfire scene is 1.2 MB as
APNG and 1.76 MB as WebP, while libwebp, which stores only what changes, writes
it in 53 KB. Animated pixel art usually changes in small areas (a flame, a
walking sprite, a few stars), so both formats waste most of their bytes
re-encoding unchanged pixels.

## What Changes

- **Changed-area frames.** After the first frame, each APNG and WebP frame
  covers only the bounding box of the pixels that differ from the previous
  frame, and overwrites that area (`APNG_BLEND_OP_SOURCE` / WebP "do not
  blend", dispose none). The result is exact by construction: no decoder
  arithmetic is involved. The first frame stays full canvas.
- WebP frame offsets must be even, so the box is extended by one pixel to the
  left or top when needed.
- A frame identical to the previous one (only produced when a long delay is
  split) is written as a 1x1 frame.
- **Masked frames, conditionally.** A further mode keeps the box but makes
  unchanged pixels transparent and uses `APNG_BLEND_OP_OVER` / WebP alpha
  blending, which compresses much better when changes are scattered (twinkling
  stars). It is only valid when every changed pixel is fully opaque in the new
  frame, and only if decoders reproduce opaque-over exactly. This change first
  verifies that per format against libwebp, `image-webp` and independent APNG
  decoders, and enables the mode only for the formats that pass. Per frame,
  both encodings are tried and the smaller is kept. Result: APNG passes, WebP
  does not (`image-webp` blends opaque pixels inexactly), so masked frames are
  APNG-only.
- Size check against the numbers recorded by `add-webp-export`, and README
  updates (frames no longer "cover the whole canvas").

No worksheet or CLI change. Output bytes change (golden files are
regenerated); decoded pixels and frame timing do not.

## Capabilities

### New Capabilities

_None._

### Modified Capabilities

- `apng-export`: "APNG structure" no longer requires every frame to cover the
  full canvas; it defines changed-area frames and, if verified, masked frames.
- `webp-export`: "Animated WebP structure" likewise (changed-area frames
  only), plus even offsets.

## Impact

- `src/encode.rs` (APNG writer), `src/webp.rs` (ANMF writer), a new shared
  frame-diff helper (bounding box, masking, even alignment).
- Test helpers: APNG decoding in tests must now composite frames (`png`
  returns sub-frame data only); `image-webp` already composites.
- Golden files under `tests/golden/` regenerated, checked by decoding to the
  same pixels as before.
- `README.md` (APNG and WebP paragraphs).
- No new dependencies.
