## 1. Verify masked frames in real decoders

- [x] 1.1 Write a verification script (kept in the change folder, `verify-blend.py`, run with `python3 -I`) that builds 2-frame APNG and animated WebP files by hand: frame 0 a full canvas of destination pixels (every channel value 0–255 against destination alpha 0, 1, 128, 254, 255), frame 1 a masked frame (OVER / alpha blending) with opaque and fully transparent source pixels
- [x] 1.2 Decode with Pillow (libwebp for WebP, Pillow's compositor for APNG), FFmpeg (APNG) and `image-webp` (WebP, through a small Rust test), and compare with the expected canvas
- [x] 1.3 Record the results in the change folder (`blend-verification.md`); for each format that fails, remove the masked kind from its spec delta and the related tasks below

## 2. Frame diff

- [x] 2.1 Create `src/delta.rs`: `Rect`, `changed_rect`, `align_even`, `crop`, `masked`
- [x] 2.2 Unit tests: no change gives `None`; one pixel; opposite corners; `align_even` at odd and even edges and at the canvas border; `masked` returns `None` when a changed pixel is not opaque and sets unchanged pixels to `(0, 0, 0, 0)`

## 3. Test helpers

- [x] 3.1 Make `decode_apng` in `tests/common/images.rs` composite frames (SOURCE; OVER for source alpha 0 and 255 only, panicking otherwise) and keep returning each frame's `fcTL` fields
- [x] 3.2 Check that every existing APNG test still passes with the compositing decoder (they still see full-canvas frames at this point)

## 4. APNG

- [x] 4.1 In `encode::apng`, write later frames as changed-area frames (`set_frame_dimension` / `set_frame_position`, blend SOURCE, dispose NONE); 1x1 at (0, 0) for identical frames
- [x] 4.2 Add masked candidates (RGBA, or indexed with `(0, 0, 0, 0)` in the palette) and keep the smaller compressed frame, changed area on a tie
- [x] 4.3 Tests in `tests/apng.rs`: the "Small change" 9x8 rectangle at (10, 4); transparent-over-opaque is a changed-area frame and decodes to `(0, 0, 0, 0)`; split delay gives a 1x1 frame; scattered opaque changes may be masked and decode exactly; indexed palette without `(0, 0, 0, 0)` never masks; round trip of the animations of `tests/fixtures/valid/animation.json` and `full.json` (`scene.json` has none)

## 5. WebP

- [x] 5.1 In `webp::animated`, write later frames as changed-area `ANMF`s with even-aligned rectangles; first frame full canvas; 1x1 at (0, 0) for identical frames
- [x] 5.2 ~~Add masked candidates for WebP~~ Dropped: `image-webp` fails the blend verification (see `blend-verification.md`)
- [x] 5.3 Tests in `tests/webp.rs`: odd offset (5, 3) gives the 2x2 rectangle at (4, 2); transparent-over-opaque is a changed-area frame; split duration gives a 1x1 frame; round trip with `image-webp`

## 6. Goldens and cross-checks

- [x] 6.1 Before regenerating, decode every current golden APNG and WebP to canvases and save them (scratch); regenerate with `UPDATE_GOLDEN=1`; check that the new goldens decode to the same canvases with Pillow and FFmpeg (APNG) and Pillow (WebP)
- [x] 6.2 Keep `webp_goldens_pass_webpinfo` passing; add a similar golden test that decodes every golden APNG with the compositing helper and compares to `Animation::frames()`

## 7. Size check and docs

- [x] 7.1 Re-run the size check of `add-webp-export` on `examples/` (scale 1 and 3) and record before/after for APNG and WebP, plus render times, in `size-check.md` in the change folder
- [x] 7.2 Update `README.md`: APNG and WebP paragraphs no longer say every frame covers the whole canvas; describe changed-area frames (both formats) and masked frames (APNG only); refresh the WebP size comparison
- [x] 7.3 `cargo test`, `cargo clippy --all-targets` and `cargo fmt --check` pass
