## Why

After `add-worksheet-loading` the engine can trust a resolved model but cannot yet
produce an image. Sprites are the smallest renderable unit and the base of every
composition and animation, so rendering them to PNG is the first visible result
and fixes the pixel and encoding rules everything else reuses.

## What Changes

- Rasterize resolved sprites into an RGBA8 image: palette lookup, `.` as fully
  transparent, every fully transparent pixel stored as `(0, 0, 0, 0)`.
- Implement the transform operations (`flip`, `rotate`, `scale`, `crop`) on
  images, applied in array order. Sprites have no `transforms` of their own;
  frames and elements will use these operations in later changes.
- Add `pixel-perfect render <worksheet>` (kebab-case flags) with:
  - `--output-path <dir>` / `-o`: the root output directory;
  - `--format <format>`: only `png` for now;
  - `--scale <N>`: integer nearest-neighbor upscale of the final image, default 1;
  - `--safe-build`: hashed file names, see below.
- Write each exported resource to `<output-path>/<collection>/<id>.<format>`
  (e.g. `out/sprites/hero.png`), creating directories as needed. Ids that are
  not portable file names (separators, reserved characters, case-only
  differences) are an error that suggests `--safe-build`.
- With `--safe-build`, write to `<output-path>/<collection>/<hash>.<format>`,
  where `<hash>` is the BLAKE2b-64 of the id (16 hex digits, same as
  `b2sum -l 64`). Any id works, and a file keeps its name as long as its id
  does not change.
- Always write `<output-path>/manifest.json`, mapping each export pointer to
  its file path and size, so tools never have to guess file names.
- Pick the PNG encoding per image by counting the distinct colors of the final
  image: up to 256 colors → indexed PNG (smallest bit depth that fits, with
  `tRNS` when needed); more → RGBA8. The count runs on the rendered pixels, not
  on the worksheet characters, so the same rule holds unchanged when panels and
  scenes blend semi-transparent colors.
- Encode deterministically: fixed encoder settings, no timestamps or text chunks.
- Export the sprites listed in `exports`, or every sprite when `exports` is
  absent. Exporting resource types not yet supported is an error when they are
  listed explicitly and a notice when they are only implied.

## Capabilities

### New Capabilities
- `sprite-rendering`: rasterizing sprites and the image transform operations.
- `png-export`: the `render` command, output paths, safe build, manifest,
  upscaling, color-count based PNG encoding and deterministic output.

### Modified Capabilities
<!-- None. -->

## Impact

- New modules: `raster` (image buffer and transforms), `render` (sprite
  rasterization), `png` (encoder), `export` (selection, paths, writing).
- New dependencies: `png` and `blake2` (both pure Rust).
- The CLI gains the `render` subcommand.
