## Context

`encode::apng` and `webp::animated` receive the final frames of an animation
(after merging, after `--scale`) and write each one as a full-canvas frame
that replaces the previous one. Both formats can do better natively:

- APNG `fcTL`: `x_offset`, `y_offset`, `width`, `height`, `dispose_op`
  (NONE / BACKGROUND / PREVIOUS), `blend_op` (SOURCE / OVER). The first frame,
  when it is the default image, must cover the canvas. The `png` crate (0.18)
  supports this through `set_frame_dimension` / `set_frame_position`.
- WebP `ANMF`: offset stored as `x/2`, `y/2` (so even offsets only), width,
  height, "do not blend" or "use alpha blending", dispose none / background.

Measured on `examples/` (scale 1): APNG 1.28 MB and our WebP 1.92 MB for the 7
animations, against 91 KB for libwebp's `img2webp`, which uses sub-rectangles
and blending. The engine's principles are lossless and byte-for-byte
deterministic output.

## Goals / Non-Goals

**Goals:**
- Write only what changes between frames, in both formats, with no change to
  decoded pixels or timing.
- Use blending only where it is provably exact in real decoders.
- Measure the gain on `examples/`.

**Non-Goals:**
- Lossy tricks (near-equal colors treated as equal).
- Several rectangles per frame, or reordering/merging frames differently.
- `APNG_DISPOSE_OP_PREVIOUS` / dispose to background: with an overwrite of the
  changed box there is nothing they add that the box does not already do.
- A palette transform for WebP (an `image-webp` limitation).

## Decisions

**Two frame kinds, both "dispose none".**
- *Changed area* (overwrite the bounding box of differing pixels): correct by
  construction for every pixel, transparent ones included, because no decoder
  arithmetic is involved. This is the default and always allowed.
- *Masked* (same box, unchanged pixels set to `(0, 0, 0, 0)`, blend OVER /
  alpha blending): long runs of transparent pixels compress far better when
  the changes inside the box are scattered (twinkling stars across a sky).
  It is only exact if, for a fully opaque source pixel, the decoder outputs
  the source, and for a fully transparent one, the destination. It is used
  only when every changed pixel is opaque in the new frame.
Alternative: dispose-to-background frames to clear areas. Rejected: a
transparent pixel in the new frame is already handled by overwriting.

**Verify the masked kind before relying on it.** The APNG and WebP specs give
blending formulas, but decoders implement them with their own integer
arithmetic. libwebp's non-premultiplied blend, for example, computes a scale
`2^24 / blend_alpha` and may not return the source exactly for alpha 255.
Before any masked frame is written, a verification step encodes test frames
covering every channel value 0–255 over destinations with alpha 0, 1, 128,
254 and 255, and decodes them with:
- WebP: libwebp (through Pillow's `WebPAnimDecoder`) and `image-webp`;
- APNG: Pillow and FFmpeg, two independent compositors.
Masked frames are enabled only for a format whose decoders all reproduce the
expected pixels. If a format fails, its spec delta loses the masked kind
before the change is archived, and the result is recorded. Browsers cannot be
tested here; restricting masked frames to sources with alpha 0 or 255 keeps
them on the cases every compositor special-cases or computes exactly when
implemented to spec.

**A shared frame-diff module, `src/delta.rs`.** Pure functions on `Image`:
`changed_rect(prev, next) -> Option<Rect>`, `align_even(rect) -> Rect`,
`crop(image, rect) -> Image`, `masked(prev, next, rect) -> Option<Image>`
(`None` when some changed pixel is not opaque). Both encoders use it, so the
geometry is tested once.

**Choose per frame by encoded size.** When both kinds are allowed, both are
encoded and the smaller is kept (changed area on a tie). Deterministic, and
it never makes a frame larger than the changed-area version.
- APNG: the frame data is compressed by the `png` writer; candidates are
  measured by compressing their rows with the same `Encoding` and compression
  settings in a scratch encoder, then the winner is written.
- WebP: each candidate is a `VP8L` bitstream; compare lengths directly.

**WebP alpha flag with masked frames.** Masked frames need alpha in their
bitstream, and every `VP8L` must agree with the `VP8X` alpha flag. If the
animation already has alpha, all frames are RGBA and each frame chooses
freely. If it is fully opaque, the encoder builds the file twice — once with
changed-area frames only, as RGB; once allowing masked frames, all RGBA — and
keeps the smaller (the first on a tie). At most twice the encoding work, only
for opaque animations.

**Indexed APNG and masked frames.** Masked frames need `(0, 0, 0, 0)` in the
palette. The palette rules of `apng-export` are not changed to add it; masked
frames are simply not allowed for an indexed APNG whose palette lacks it.
RGBA APNGs can always use them.

**FFmpeg and 2- or 4-bit indexed sub-frames.** Found while checking the
regenerated goldens: FFmpeg 8.1 decodes APNG sub-frames of indexed images at
bit depth 2 or 4 wrongly whenever their x offset is not 0 (even when the
offset is byte-aligned); 1-bit, 8-bit and RGBA images are fine, and Pillow and
the `png` crate decode every case correctly. The files are valid, but FFmpeg
is a common way to convert APNG, so for those two bit depths the changed area
is extended to the left edge (x = 0). Its height still shrinks.
Alternatives: ignore the bug (FFmpeg users see wrong frames), or never write
2- and 4-bit animations (changes the palette rules). Chosen with the user.

**Identical frames.** Only produced when a long delay is split. They are
written as a 1x1 changed-area frame at (0, 0) holding the current pixel: the
smallest valid frame (both formats require width and height ≥ 1).

**Tests composite like a decoder.** `png` returns sub-frame data, so
`tests/common/images.rs` gets an APNG compositor (SOURCE, and OVER for alpha 0
and 255 only, panicking otherwise, since the engine never writes anything
else). `image-webp` and Pillow already return composited WebP frames. All
existing round-trip tests then hold unchanged: decoded canvases equal
rendered frames.

## Risks / Trade-offs

- [A decoder not tested here (a browser) blends opaque-over inexactly] →
  only alpha 0/255 sources; the verification step documents which decoders
  were checked; a format that fails anywhere tested keeps changed-area only.
- [Encoding both candidates doubles CPU per frame] → only for frames where
  masked is allowed; encoding is cheap next to rendering at these sizes; the
  size check reports render times before and after.
- [Golden files change] → regenerated once, and checked by decoding them to the
  same pixels as the previous goldens (APNG and WebP) before committing.
- [Bounding box of scattered changes is the whole canvas] → this is exactly
  the case masked frames target; if masked frames fail verification for a
  format, that format gains little on such animations, and the size check
  will show it.

## Migration Plan

No user action: same worksheets, same CLI, same decoded output. File bytes
change once; anyone comparing hashes of rendered files across engine versions
sees new values (determinism is per engine build).

## Open Questions

- ~~Does libwebp's alpha blending return the source exactly for alpha 255?~~
  Answered (`blend-verification.md`): libwebp, Pillow and FFmpeg are exact;
  `image-webp` 0.2.4 is not (opaque `c` becomes `c - 1`). Masked frames are
  enabled for APNG only; WebP uses changed-area frames, so the WebP alpha
  flag rule and the "build twice" decision below no longer apply.
