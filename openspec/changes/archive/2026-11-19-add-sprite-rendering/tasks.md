## 1. Image buffer

- [x] 1.1 Add `raster::Image` (width, height, row-major `Vec<Rgba>`) with canonical transparency enforced on construction
- [x] 1.2 Test: a pixel with alpha 0 and non-zero channels is stored as `(0, 0, 0, 0)`

## 2. Transforms

- [x] 2.1 Implement `flip` (x, y), `rotate` (90, 180, 270 clockwise), `scale` (nearest-neighbor, `floor(x * src / dst)`), `crop`
- [x] 2.2 Implement `apply_all` for chains in array order
- [x] 2.3 Tests: every scenario of `specs/sprite-rendering/spec.md` for flip, rotate, scale, crop and chain order, using small letter-coded images

## 3. Sprite rasterization

- [x] 3.1 Rasterize a `ResolvedSprite` with its resolved palette; `.` → transparent
- [x] 3.2 Tests: palette lookup with alpha, derived sprite with its own palette, transparent palette color canonicalized

## 4. PNG encoder

- [x] 4.1 Add the `png` dependency; count distinct colors with early exit above 256
- [x] 4.2 Build the deterministic palette (non-opaque first, then opaque, each by first appearance) and pick bit depth 1/2/4/8
- [x] 4.3 Encode indexed (`PLTE`, `tRNS` only when needed, packed rows for bit depth < 8) or RGBA8, with fixed compression and filter and no metadata chunks
- [x] 4.4 Integer upscale of the final image (`--scale`)
- [x] 4.5 Tests: bit depth for 1, 2, 3, 4, 5, 16, 17, 256 and 257 colors; palette order and `tRNS` length; opaque image without `tRNS`; chunk list; decode round trip (indexed and RGBA) with the `png` decoder
- [x] 4.6 Test: rendering twice gives byte-identical files

## 5. Export planning and writing

- [x] 5.1 Select resources from `exports`, or all sprites when absent; unsupported explicit exports are errors, implicit ones produce a notice
- [x] 5.2 Default mode: check file-safe ids and case-insensitive collisions per collection, with errors suggesting `--safe-build`; collect every error before writing
- [x] 5.3 Safe build: add the `blake2` dependency; name files with BLAKE2b-64 of the id (16 lowercase hex digits); check hash collisions per collection
- [x] 5.4 Write to `<output-path>/<collection>/<name>.<format>`, creating directories and overwriting files
- [x] 5.5 Write `manifest.json` after the images: `format`, `scale`, `safeBuild`, `files` keyed by pointer with `path`, `width`, `height`; sorted keys, pretty-printed, trailing newline
- [x] 5.6 Tests: explicit and implicit selection, unsupported explicit export, each forbidden id form, case collision, same id in different collections, nothing written on error
- [x] 5.7 Tests: safe-build names for `hero` (`8737b11b474b218b`) and `Hero` (`b46e11bab0d34796`), `ui/button` accepted, name unchanged after editing pixels, manifest content in both modes and byte-identical across renders

## 6. CLI

- [x] 6.1 Add `render <worksheet>` with `--output-path`/`-o` (default `.`), `--format` (`png`), `--scale` (1-64, default 1) and `--safe-build`
- [x] 6.2 Exit codes: 0 success, 1 invalid worksheet or invalid export plan, 2 usage or IO error
- [x] 6.3 Integration tests: render the README example into a temp dir and check files and sizes; `--scale 3`; `--format gif` and `--scale 0` → 2; invalid worksheet → 1 with nothing written

## 7. Wrap-up

- [x] 7.1 Golden files: render `tests/fixtures/valid/full.json` sprites and compare with committed PNGs under `tests/golden/`
- [x] 7.2 `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` pass
- [x] 7.3 README: document `render`, the output layout and the indexed/RGBA choice
