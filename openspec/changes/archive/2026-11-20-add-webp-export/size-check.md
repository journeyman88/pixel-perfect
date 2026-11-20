# Size check: PNG/APNG vs WebP

Measured on 2026-11-20 with a release build, rendering every file in
`examples/` (5 worksheets, 26 exported resources: 19 static, 7 animated) with
`--format png` and `--format webp`. Sizes in bytes.

## Engine output (`image-webp` 0.2.4 encoder, our container)

| scale | kind | files | PNG/APNG | WebP | WebP / PNG | WebP smaller |
|---|---|---|---|---|---|---|
| 1 | static | 19 | 11 037 | 38 542 | 3.49 | 0/19 |
| 1 | animated | 7 | 1 283 483 | 1 917 932 | 1.49 | 0/7 |
| 3 | static | 19 | 17 374 | 44 396 | 2.56 | 0/19 |
| 3 | animated | 7 | 1 759 722 | 2 201 502 | 1.25 | 0/7 |

Per example (PNG → WebP, all resources):

| example | scale 1 | scale 3 |
|---|---|---|
| animation | 29 799 → 62 394 (2.09) | 42 456 → 69 202 (1.63) |
| campfire | 1 253 684 → 1 855 538 (1.48) | 1 717 266 → 2 132 300 (1.24) |
| composition | 1 859 → 3 344 (1.80) | 2 373 → 3 626 (1.53) |
| masquerade | 7 962 → 33 204 (4.17) | 13 361 → 38 450 (2.88) |
| sprites | 1 216 → 1 994 (1.64) | 1 640 → 2 320 (1.41) |

WebP is never smaller: from 1.05× (`campfire` `/timelines/fire`, scale 3) to
4.17× (`masquerade` `/scenes/masquerade`, scale 1).

Why: the `image-webp` encoder applies only the subtract-green and predictor
transforms. It never uses the color-indexing (palette) transform, which is
what makes lossless WebP small on images with few colors, and the engine
writes every animation frame as a full canvas.

## Reference: libwebp (not used by the engine)

Same images, encoded by libwebp's command-line tools for comparison only.

| scale 1 | PNG/APNG | engine WebP | libwebp |
|---|---|---|---|
| 19 static (`cwebp -lossless -exact -z 9`) | 11 037 | 38 542 | 8 358 |
| 7 animated (`img2webp -lossless -exact -m 6 -q 100`) | 1 283 483 | 1 917 932 | 90 584 |

Animated resources, scale 1:

| resource | APNG | engine WebP | libwebp |
|---|---|---|---|
| animation `/flipbooks/walk` | 439 | 1 064 | 270 |
| animation `/panels/torch-post` | 349 | 752 | 238 |
| animation `/scenes/evening` | 19 150 | 40 302 | 10 018 |
| animation `/timelines/stroll` | 9 861 | 20 276 | 6 730 |
| campfire `/flipbooks/flames` | 2 687 | 5 210 | 2 006 |
| campfire `/scenes/campfire` | 1 201 382 | 1 759 562 | 53 348 |
| campfire `/timelines/fire` | 49 615 | 90 766 | 17 974 |

libwebp beats PNG on stills (−24%) and APNG on animations by 14× overall,
mostly through sub-rectangle frames (only the changed area of each frame is
stored): the campfire scene goes from 1.2 MB to 53 KB.

## Conclusion

With the pure-Rust encoder, choosing WebP never saves space on these examples.
The format itself is not the problem: libwebp shows large gains. The gap comes
from (1) the missing palette transform in `image-webp` and (2) full-canvas
frames. (2) is in the engine's control and would also shrink APNG.
