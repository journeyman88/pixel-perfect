# webp-export Specification

## Purpose

Write rendered resources as lossless, deterministic WebP files with
`--format webp`: the simple container for static resources, the animated
container (`VP8X`, `ANIM`, full-canvas `ANMF` frames) for animated ones,
millisecond frame durations rounded without accumulating error, and WebP's
16384-pixel side limit.

## Requirements

### Requirement: Static WebP structure
With `--format webp`, each static resource SHALL be written as a lossless WebP
file in the simple format: a `RIFF` header with form type `WEBP` followed by a
single `VP8L` chunk and nothing else. The `VP8L` header SHALL declare alpha
as used if and only if some pixel has alpha below 255.

#### Scenario: Static sprite
- **WHEN** sprite `hero` is rendered with `--format webp`
- **THEN** `sprites/hero.webp` is written and its chunks are exactly `VP8L`

### Requirement: Animated WebP structure
With `--format webp`, each animated resource SHALL be written as an animated
WebP file whose chunks are, in order: `VP8X`, `ANIM`, then one `ANMF` per
written frame, and nothing else. `VP8X` SHALL set the animation flag, SHALL set
the alpha flag if and only if some pixel of some frame has alpha below 255, and
SHALL give the canvas size. `ANIM` SHALL give background color `(0, 0, 0, 0)`
and loop count 0 (loop forever). Every `ANMF` SHALL use blending method "do
not blend" and disposal method "none", and SHALL contain a single `VP8L`
chunk. Every `VP8L` header SHALL declare alpha as used if and only if the
`VP8X` alpha flag is set, since libwebp rejects frames that use alpha in a
file that declares none. A resource that is animated SHALL be written as an
animated WebP even when it has a single frame.

The first `ANMF` SHALL cover the full canvas at offset (0, 0). Every later
`ANMF` SHALL cover the smallest rectangle containing every pixel that differs
from the previous frame, extended by one pixel to the left and/or top when its
left or top edge is odd (frame offsets are stored halved), and hold the new
frame's pixels in that rectangle, so that the canvas equals the new frame. If
no pixel differs, the rectangle is the 1x1 area at (0, 0). Frames SHALL NOT use
alpha blending: `image-webp` does not composite opaque pixels exactly.

#### Scenario: Chunk layout
- **WHEN** flipbook `spin` with 4 different frames is rendered with `--format webp`
- **THEN** `flipbooks/spin.webp` contains `VP8X`, `ANIM` and 4 `ANMF` chunks, each `ANMF` holding one `VP8L` chunk

#### Scenario: Transparent pixel over an opaque one
- **WHEN** a pixel is opaque in frame 0 and fully transparent in frame 1
- **THEN** decoding frame 1 gives `(0, 0, 0, 0)` at that pixel

#### Scenario: Opaque animation
- **WHEN** every pixel of every frame has alpha 255
- **THEN** the `VP8X` alpha flag is not set and no `VP8L` frame declares alpha

#### Scenario: Odd offset
- **WHEN** the only changed pixel of a frame is at (5, 3)
- **THEN** the `ANMF` covers the 2x2 rectangle at (4, 2)

#### Scenario: Split duration
- **WHEN** a frame is written as two identical frames because its duration exceeds 16777215 ms
- **THEN** the second one is a 1x1 frame at (0, 0)

#### Scenario: Accepted by libwebp
- **WHEN** any animated WebP written by the engine is checked with libwebp's `webpinfo`
- **THEN** no error is reported

#### Scenario: Pixels unchanged after decoding
- **WHEN** an animated WebP is decoded frame by frame by libwebp and by `image-webp`
- **THEN** every frame equals the corresponding rendered frame, exactly as before this change

### Requirement: Millisecond frame durations
For an animated WebP, the engine SHALL compute each frame's end time exactly
(as the animation capability defines) and round it to the nearest millisecond,
halves rounding up; the first frame starts at 0 ms. A frame's duration SHALL be
its rounded end time minus the rounded end time of the previous frame, so
rounding errors never accumulate and the loop length is the exact loop length
rounded to the nearest millisecond. Frames whose duration is 0 ms SHALL NOT be
written. After that, consecutive frames with identical pixels SHALL be merged
into one frame whose duration is the sum. A frame whose duration exceeds
16777215 ms SHALL be written as `floor(n / 16777215)` identical frames of
16777215 ms followed, if `n mod 16777215` is not 0, by one identical frame of
`n mod 16777215` ms, where `n` is its duration. Unlike APNG, no combination of
`fps` values SHALL make a WebP render fail because of its delays. If the frames
to write, counted after splitting and before merging, could exceed 2³¹ − 1,
rendering SHALL fail before anything is written, with an error pointing to the
resource.

#### Scenario: Thirds of a second
- **WHEN** a flipbook with `fps` 3 has three different frames of `duration` 1
- **THEN** the frame durations are 333, 334 and 333 ms and the loop lasts 1000 ms

#### Scenario: Whole-millisecond loop stays exact
- **WHEN** a timeline with `fps` 30 and `duration` 45 is rendered
- **THEN** the frame durations add up to exactly 1500 ms

#### Scenario: Sub-millisecond frame
- **WHEN** an animated panel plays two flipbooks whose frames change at every frame-unit, at `fps` 100 and 99, so that one frame lasts from 1/100 s to 1/99 s
- **THEN** that frame is not written, because both ends round to 10 ms

#### Scenario: Rates that APNG rejects
- **WHEN** the scene of the apng-export scenario "Denominator too large" is rendered with `--format webp`
- **THEN** rendering succeeds

#### Scenario: Long still frame
- **WHEN** a frame lasts 20000 s
- **THEN** it is written as a frame of 16777215 ms followed by an identical frame of 3222785 ms

#### Scenario: Too many frames after splitting
- **WHEN** a timeline with `fps` 1 and `duration` 10¹⁴ (one still frame of 10¹⁴ s, about 6·10⁹ parts of 16777215 ms) is rendered with `--format webp`
- **THEN** rendering fails before anything is written, with an error pointing to the timeline

### Requirement: WebP size limit
With `--format webp`, before writing anything, the engine SHALL reject every
exported resource whose final image (after `--scale`) is wider or taller than
16384 pixels, the largest side a `VP8L` bitstream can store. The error SHALL
point to the resource, give its size and the 16384 limit, and mention that PNG
has no such limit; nothing SHALL be written and the exit code SHALL be 1. This
check SHALL apply regardless of `--max-image-size`, including `none`.

#### Scenario: Upscale past the limit
- **WHEN** a 300x300 sprite is rendered with `--format webp --scale 64` (19200x19200)
- **THEN** rendering fails with an error pointing to the sprite that mentions 16384, no file is written and the exit code is 1

#### Scenario: Same render as PNG
- **WHEN** the same sprite is rendered with `--format png --scale 64`
- **THEN** the WebP limit does not apply

### Requirement: Lossless and deterministic WebP
Decoding a written WebP SHALL give exactly the final RGBA image, and decoding
each frame of an animated WebP SHALL give exactly the corresponding frame, after
`--scale`, including canonical transparency. Rendering the same worksheet with
the same options and the same engine build SHALL produce byte-identical files.
The engine SHALL NOT write `ICCP`, `EXIF`, `XMP ` or any other metadata chunk.

#### Scenario: Round trip
- **WHEN** a written WebP is decoded to RGBA8
- **THEN** every pixel equals the rendered pixel

#### Scenario: Animated round trip
- **WHEN** a written animated WebP is decoded frame by frame
- **THEN** every frame equals the rendered frame and every duration equals the duration computed above

#### Scenario: Repeated render
- **WHEN** a worksheet with a timeline is rendered twice with `--format webp`
- **THEN** the two files are byte-identical
