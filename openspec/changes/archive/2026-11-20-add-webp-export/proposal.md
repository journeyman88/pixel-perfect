## Why

WebP is the second output format the project targets: lossless WebP is widely
supported on the web, and animated WebP is a common alternative to APNG. It is
deliberately scheduled after PNG/APNG because the rendering pipeline must be
stable first.

No mature pure-Rust crate encodes animated WebP, but an animated WebP is only a
RIFF container (`VP8X`, `ANIM`, one `ANMF` per frame) around ordinary lossless
(`VP8L`) bitstreams. `image-webp` already produces those bitstreams in pure
Rust, so the engine can write the container itself, as it already does for
APNG chunks, and stay free of C dependencies.

## What Changes

- Static lossless WebP export with the `image-webp` encoder (pure Rust).
- Animated lossless WebP export, also pure Rust: each frame is encoded as a
  full-canvas `VP8L` bitstream by `image-webp`, and the engine writes the
  `RIFF`/`VP8X`/`ANIM`/`ANMF` container itself (infinite loop, no blending
  between frames, no metadata).
- `--format` gains `webp`. The default stays `png`; output paths keep the same
  layout with the `.webp` extension.
- Frame timing. WebP durations are integer milliseconds, so each frame's end
  time is rounded to the nearest millisecond and durations are the differences:
  errors never accumulate, and the loop length is exact whenever it is a whole
  number of milliseconds (otherwise it is off by less than half a millisecond).
  Frames that round to 0 ms are not written.
- WebP's own limit: each side at most 16384 pixels, checked before writing.
- Size check. Before the change is closed, the examples are rendered as PNG
  and WebP and the sizes compared. `image-webp`'s lossless encoder is simple
  and writes no inter-frame optimizations, so WebP may come out larger than
  indexed PNG; the comparison is recorded so the README can say when WebP is
  worth choosing.

## Capabilities

### New Capabilities
- `webp-export`: static and animated lossless WebP encoding, container layout
  and delay rounding.

### Modified Capabilities
- `png-export`: `--format` accepts `webp` in addition to `png`.

## Impact

- New dependency: `image-webp` (pure Rust). No C dependency, no Cargo feature
  gating.
- New module writing the animated WebP container; it can share frame planning
  and delay handling with the APNG encoder.
- Determinism: output depends only on `image-webp`'s encoder, pinned through
  `Cargo.lock`; golden files cover both static and animated WebP.
- Alternative kept in reserve: if the size check shows WebP is not useful,
  libwebp through Rust bindings (e.g. `webp-animation`) could replace the
  encoder later, behind a Cargo feature, as the project's first C dependency.
  Not part of this change.
