# single-frame-export Specification

## Purpose

Write animated resources as frame collections with `--single-frames`: one full
still image per distinct frame, with exact durations, a stable zero-padded file
layout and a color space shared by every frame of a collection, so that
third-party tools can pack or import them.

## Requirements

### Requirement: Single-frames mode
`render` SHALL accept a `--single-frames` flag, which is off by default. When it
is set, every exported resource that would otherwise be written as an animated
image SHALL be written instead as a frame collection: one still image per
frame. This applies to flipbooks, timelines, sequences, and animated panels and
scenes. Static resources SHALL be written exactly as without the flag. The
flag SHALL combine with `--format`, `--scale`, `--safe-build` and
`--max-image-size`.

#### Scenario: Animated resource as frames
- **WHEN** flipbook `spin` is rendered with `--single-frames`
- **THEN** no `flipbooks/spin.png` is written, and its frames are written under `flipbooks/spin/`

#### Scenario: Static resources unaffected
- **WHEN** sprite `hero` and static panel `stage` are rendered with `--single-frames`
- **THEN** `sprites/hero.png` and `panels/stage.png` are written as without the flag

### Requirement: Frames and durations
The frames of a collection SHALL be the frames of one loop of the animation,
at each tick where the image may change (see the animation capability).
Consecutive identical frames SHALL be merged into one, with no upper bound on
the merged duration. The duration of each frame SHALL be its exact length in
seconds, reduced to lowest terms. The durations SHALL add up exactly to the
loop length. Every frame SHALL be a full image of the animation's canvas, after
`--scale`. The limits that come from the APNG delay fields and from WebP
millisecond durations SHALL NOT apply in this mode.

#### Scenario: Exact durations
- **WHEN** flipbook `spin` (`fps` 12, frame durations 2, 2, 1, 1, all different) is rendered with `--single-frames`
- **THEN** it has 4 frames, with durations 1/6, 1/6, 1/12 and 1/12 seconds, and a loop of 1/2 second

#### Scenario: Identical frames merged
- **WHEN** a flipbook (`fps` 10) shows sprite `a` for 1 frame-unit, sprite `a` again for 2, then sprite `b` for 1, and is rendered with `--single-frames`
- **THEN** it has 2 frames, with durations 3/10 and 1/10 seconds

#### Scenario: Duration that no APNG delay can store
- **WHEN** a scene combines rates of 97, 89 and 83 fps, so that some frame durations, reduced, have a denominator over 65535, and it is rendered with `--single-frames`
- **THEN** rendering succeeds with no APNG timing error, every frame keeps its exact duration, and an image that stays the same across those durations is a single frame

### Requirement: Frame file layout
The frames of a resource SHALL be written to
`<output-path>/<collection>/<name>/<index>.<format>`:
- `<name>` is the id, or its hash with `--safe-build`. It follows the same
  file-safe id rules as file names (see png-export).
- `<index>` is the position of the frame in playback order, in decimal,
  starting at 0. It is zero-padded to the same width for every frame of the
  collection: 4 digits, or the number of digits of the last index when that is
  more. Names sorted alphabetically are therefore in playback order.

Frame files from earlier renders that are not part of the current one SHALL
NOT be deleted.

#### Scenario: Frame paths
- **WHEN** flipbook `spin` with 4 distinct frames is rendered with `--single-frames`
- **THEN** the files are `flipbooks/spin/0000.png`, `flipbooks/spin/0001.png`, `flipbooks/spin/0002.png` and `flipbooks/spin/0003.png`

#### Scenario: More than 10000 frames
- **WHEN** an animation with 12345 distinct frames is rendered with `--single-frames` and a `--max-image-size` large enough
- **THEN** its frames are named `00000` to `12344`, all with 5 digits

#### Scenario: Frame paths in safe build
- **WHEN** flipbook `spin` is rendered with `--single-frames --safe-build`
- **THEN** its frames are written under `flipbooks/<hash of "spin">/`

### Requirement: Frame encoding
Each frame SHALL be a still image in the chosen `--format`, with no animation
chunks. Every frame of a collection SHALL share the same color space, so that
third-party tools can import the frames together:
- With `png`, the encoding SHALL be chosen once for the whole collection, from
  the colors of all its frames together, as APNG does (see apng-export). If
  there are at most 256 distinct colors in total, every frame SHALL be an
  indexed PNG with the same palette, the same `tRNS` and the same bit depth.
  The palette follows the deterministic palette order of png-export, applied to
  the frames in playback order. Otherwise every frame SHALL be RGBA.
- With `webp`, each frame SHALL be a static lossless WebP (see webp-export).

Output SHALL be byte-identical for identical renders.

#### Scenario: Frames as PNG share one palette
- **WHEN** flipbook `spin`, whose 4 frames each have a single, different color, is rendered with `--single-frames`
- **THEN** every frame is an indexed PNG with no `acTL` chunk, and all 4 frames have the same 4-color `PLTE` and the same bit depth (2)

#### Scenario: Too many colors across frames
- **WHEN** an animation has two frames of 144 colors each, all different, for 288 in total, and is rendered with `--single-frames`
- **THEN** both frames are RGBA PNGs, although each would fit a palette on its own

#### Scenario: Frames as WebP
- **WHEN** the same animation is rendered with `--single-frames --format webp`
- **THEN** every frame is a static lossless WebP with no `ANIM` chunk
