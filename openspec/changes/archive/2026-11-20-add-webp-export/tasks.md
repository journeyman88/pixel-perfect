## 1. Setup

- [x] 1.1 Add `image-webp` to `[dependencies]` in `Cargo.toml`; check it builds without a C toolchain
- [x] 1.2 Add a WebP chunk lister to `tests/common/images.rs` (RIFF chunks in order, and the chunks nested in each `ANMF`), with a unit test on a hand-built file

## 2. Millisecond durations

- [x] 2.1 Add `time::ms_durations(ends, clock)`: round each end tick to ms, halves up, in `u128`; return the differences
- [x] 2.2 Unit tests in `src/time.rs`: thirds of a second give 333/334/333; a whole-ms loop sums exactly; a half ms rounds up; a sub-ms segment gives 0

## 3. Animation segments

- [x] 3.1 Add a private `Animation::merged()` (render each segment, skip zero durations, merge identical neighbours through a format-specific rule) and rewrite `frames()` on top of it, keeping the APNG merge rule unchanged
- [x] 3.2 Test: `frames()` output is unchanged for `tests/fixtures/valid/animation.json` and `scene.json`, and the existing APNG golden files still match
- [x] 3.3 Add the WebP frame list `Animation::ms_frames()` (`ms_durations` over the segments, drop 0 ms without rendering, merge identical neighbours) and `Animation::ms_frame_bound()` (frames written after splitting, upper bound); splitting over 16777215 ms happens in the encoder (4.4)
- [x] 3.4 Tests: sub-ms frame dropped and the identical neighbours around it merged (flipbooks at `fps` 100 and 99, fixture `tests/fixtures/valid/webp-submillisecond.yaml`); thirds of a second; frame bound

## 4. WebP encoder

- [x] 4.1 Create `src/webp.rs` with chunk helpers (fourcc, little-endian size, even padding, 24-bit fields) and the `VP8L` payload extraction from `image-webp` output, with explicit encoder parameters
- [x] 4.2 Implement `webp::still(&Image)`: `RIFF`/`WEBP`/`VP8L` only
- [x] 4.3 Tests: static round trip (including a fully transparent pixel and a half-transparent one) and chunk list is exactly `VP8L`
- [x] 4.4 Implement `webp::animated(&[WebpFrame])`: `VP8X` (animation flag, alpha flag only if some alpha < 255, canvas size), `ANIM` (background 0, loop 0), one full-canvas `ANMF` per frame (no blend, no dispose); split durations over 16777215 ms into identical frames; return an error past the RIFF size limit
- [x] 4.5 Tests: chunk layout for 4 frames; alpha flag set and unset; opaque-to-transparent pixel decodes to `(0, 0, 0, 0)`; animated round trip with durations; single-frame animation is still animated WebP; a 20000 s frame split into 16777215 + 3222785 ms

## 5. Export and CLI

- [x] 5.1 Add `OutputFormat::Webp` (extension `webp`) and the `webp` value of `--format` in `main.rs`
- [x] 5.2 In `plan`: run the APNG delay check only for `png`; for `webp` reject any final image side over 16384, pointing to the resource and mentioning PNG, and reject animations whose `ms_frame_bound` exceeds `MAX_FRAMES`
- [x] 5.3 In `write`: dispatch static and animated outputs to `webp::still` / `webp::animated`; fill manifest `frames` with the `ANMF` count
- [x] 5.4 Tests in `tests/export.rs`: `--format webp` writes `.webp` paths with manifest `format` `"webp"`; 300x300 sprite at `--scale 64` fails with nothing written; the same render as PNG is unaffected; the apng-export "Denominator too large" scene renders as WebP; a timeline of 10^14 frame-units at `fps` 1 fails in the plan as WebP
- [x] 5.5 CLI tests in `tests/cli.rs`: `--format webp` exit 0; `--format gif` still a usage error (exit 2); WebP size-limit failure exits 1

## 6. Determinism and goldens

- [x] 6.1 Extend `tests/golden.rs` with WebP renders of the `full`, `animation` and `scene` fixtures (golden files under `tests/golden/`), regenerated with `UPDATE_GOLDEN=1`
- [x] 6.2 Test: rendering a timeline twice with `--format webp` gives byte-identical files, and no metadata chunk is ever written

## 7. Size check and docs

- [x] 7.1 Render every file in `examples/` as PNG and as WebP (`--scale 1` and `--scale 3`) and record the file sizes, static and animated, in the change folder (`size-check.md`)
- [x] 7.2 Update `README.md`: WebP and animated WebP marked supported, the ms rounding rule, the 16384-pixel limit, and when WebP is worth choosing according to 7.1; resolve the design's open question with the result
- [x] 7.3 `cargo test`, `cargo clippy --all-targets` and `cargo fmt --check` pass; existing PNG/APNG golden files unchanged
