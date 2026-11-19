## Why

WebP is the second output format the project targets: lossless WebP is usually
smaller than PNG and is widely supported on the web, and animated WebP is a
common alternative to APNG. It is deliberately scheduled after PNG/APNG because
the rendering pipeline must be stable first, and because animated WebP encoding
has no mature pure-Rust implementation.

## What Changes

- Static lossless WebP export (pure Rust, `image-webp` encoder).
- Animated lossless WebP export. This needs libwebp through Rust bindings (e.g.
  `webp-animation`), the project's first C dependency. Put it behind a Cargo
  feature so builds without a C toolchain still work.
- A CLI option to choose the output format (`png` / `webp`). The default stays PNG.
- Frame timing. WebP delays are integer milliseconds, so non-integer delays
  (e.g. 1/3 s) are rounded while carrying the error over to the next frame, so
  the total loop length stays exact.

## Capabilities

### New Capabilities
- `webp-export`: static and animated lossless WebP encoding and delay rounding.

### Modified Capabilities
- `png-export`: the `render` command gains format selection.

## Impact

- New dependencies: `image-webp` (pure Rust) and an optional libwebp binding (C)
  behind a Cargo feature.
- Determinism must be checked against libwebp's encoder settings (lossless mode,
  fixed method/quality, no metadata).
