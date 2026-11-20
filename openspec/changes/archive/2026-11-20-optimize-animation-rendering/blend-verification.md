# Blend verification: masked frames

Checked on 2026-11-20 whether decoders composite a "masked" frame exactly:
fully opaque source pixels must replace the canvas and `(0, 0, 0, 0)` source
pixels must keep it, blended OVER (APNG `APNG_BLEND_OP_OVER`, WebP "use alpha
blending").

Test image: 256x10, every channel value 0–255 per column, destination alpha
0, 1, 128, 254 and 255 per row; rows 0–4 opaque sources, rows 5–9
transparent sources. 1280 opaque-over and 1280 transparent-over pixels.

| decoder | how | opaque-over | transparent-over | result |
|---|---|---|---|---|
| Pillow 12.3 (APNG) | `verify-blend.py` | exact | exact | **pass** |
| FFmpeg 8.1 (APNG) | `verify-blend.py` | exact | exact | **pass** |
| libwebp via Pillow (`WebPAnimDecoder`) | `verify-blend.py` | exact | exact | **pass** |
| `image-webp` 0.2.4 | `tests/webp.rs` `masked_blend_is_exact_in_image_webp` (ignored) | 1280/1280 wrong | exact | **fail** |
| FFmpeg 8.1 (animated WebP) | — | — | — | not supported |

A negative control (the same WebP with frame 1 set to "do not blend") makes
the transparent rows erase the canvas, so the check does detect a decoder that
ignores blending.

Why `image-webp` fails: it applies libwebp's non-premultiplied blend formula
to every pixel, including opaque ones. With source alpha 255 the scale is
`2^24 / 255 = 65793` (truncated), and `c * 255 * 65793 >> 24 = c - 1` for every
`c ≥ 1`. libwebp itself skips the blend for opaque source pixels, which is
why it passes.

## Decision

- **APNG**: masked frames enabled (both decoders exact).
- **WebP**: masked frames not used; every later frame is a changed-area frame
  ("do not blend"). The `webp-export` spec delta and the WebP tasks were
  updated accordingly.

To re-check after an `image-webp` update:
`cargo test --test webp -- --ignored`, and
`python3 -I verify-blend.py <scratch-dir>` for the other decoders.
