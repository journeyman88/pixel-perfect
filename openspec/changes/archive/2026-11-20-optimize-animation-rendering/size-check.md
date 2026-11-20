# Size check: changed-area frames

Measured on 2026-11-20 with a release build, rendering every file in
`examples/` with `--format png` and `--format webp` at `--scale 1` and `3`.
"Before" is the measurement recorded by `add-webp-export`
(`openspec/changes/archive/2026-11-20-add-webp-export/size-check.md`), with
full-canvas frames. Sizes in bytes.

## Totals

| scale | output | before | after | after/before |
|---|---|---|---|---|
| 1 | PNG (19 static) | 11 037 | 11 037 | 1.00 |
| 1 | APNG (7 animated) | 1 283 483 | 170 766 | 0.13 |
| 1 | WebP (19 static) | 38 542 | 38 542 | 1.00 |
| 1 | animated WebP (7) | 1 917 932 | 432 250 | 0.23 |
| 3 | PNG (19 static) | 17 374 | 17 374 | 1.00 |
| 3 | APNG (7 animated) | 1 759 722 | 254 988 | 0.14 |
| 3 | WebP (19 static) | 44 396 | 44 396 | 1.00 |
| 3 | animated WebP (7) | 2 201 502 | 494 106 | 0.22 |

Static images are unchanged, as expected.

## Animated resources, scale 1

| resource | APNG before | APNG after | WebP before | WebP after |
|---|---|---|---|---|
| animation `/flipbooks/walk` | 439 | 388 | 1 064 | 824 |
| animation `/panels/torch-post` | 349 | 327 | 752 | 724 |
| animation `/scenes/evening` | 19 150 | 12 913 | 40 302 | 24 522 |
| animation `/timelines/stroll` | 9 861 | 8 268 | 20 276 | 15 214 |
| campfire `/flipbooks/flames` | 2 687 | 2 660 | 5 210 | 5 188 |
| campfire `/scenes/campfire` | 1 201 382 | 121 538 | 1 759 562 | 331 788 |
| campfire `/timelines/fire` | 49 615 | 24 672 | 90 766 | 53 990 |

For reference, libwebp's `img2webp` wrote these 7 animations in 90 584 bytes.

## Frame kinds (APNG, scale 1)

| resource | later frames | masked | area written (of full canvases) |
|---|---|---|---|
| animation `/flipbooks/walk` | 3 | 0 | 21% |
| animation `/panels/torch-post` | 2 | 0 | 25% |
| animation `/scenes/evening` | 111 | 0 | 29% |
| animation `/timelines/stroll` | 63 | 0 | 31% |
| campfire `/flipbooks/flames` | 5 | 0 | 95% |
| campfire `/scenes/campfire` | 22 | 21 | 15% |
| campfire `/timelines/fire` | 19 | 0 | 15% |

Masked frames win where changes are scattered (the twinkling stars of the
campfire scene) and nowhere else. Overall, later frames write 16% of the area
of full canvases.

## Render times

Release build, wall clock, including process start (ms). Not recorded before
this change, so there is no before/after comparison; trying both frame kinds
is visible only on the campfire scene and is small next to rendering.

| example | PNG ×1 | WebP ×1 | PNG ×3 | WebP ×3 |
|---|---|---|---|---|
| animation | 11 | 10 | 37 | 15 |
| campfire | 125 | 50 | 351 | 140 |
| composition | 6 | 4 | 4 | 6 |
| masquerade | 8 | 6 | 22 | 12 |
| sprites | 6 | 4 | 5 | 3 |

## Conclusion

Animations are 7–8× smaller as APNG and 4–5× smaller as WebP, with identical
decoded frames. APNG is now within 2× of libwebp's animated WebP on these
examples. Our WebP stays larger than APNG (no palette transform in
`image-webp`, no masked frames since `image-webp` blends opaque pixels
inexactly), so PNG remains the recommendation.
