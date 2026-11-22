# apng-export Specification

## Purpose

Write animated resources as lossless, deterministic APNG files: the chunk
layout, exact frame delays (never approximated), one palette shared by all
frames, and byte-for-byte reproducible output.

## Requirements

### Requirement: APNG structure
Each animated resource SHALL be written as an APNG file. Its `acTL` chunk SHALL
give the number of written frames and `num_plays` 0 (loop forever). The default
image (`IDAT`) SHALL be the first frame and part of the animation, and SHALL
cover the full canvas. Every frame SHALL use `dispose_op`
`APNG_DISPOSE_OP_NONE`. Every later frame SHALL be written in one of two ways,
both of which leave the canvas equal to the new frame:

- **Changed area**: the frame covers the smallest rectangle containing every
  pixel that differs from the previous frame, with `blend_op`
  `APNG_BLEND_OP_SOURCE` and the new frame's pixels in that rectangle. If no
  pixel differs, the rectangle is the 1x1 area at (0, 0).
- **Masked**: the same rectangle, with `blend_op` `APNG_BLEND_OP_OVER`; pixels
  that changed hold the new frame's value and pixels that did not change are
  `(0, 0, 0, 0)`. This way SHALL only be used when every changed pixel is fully
  opaque in the new frame, and, for an indexed APNG, when the shared palette
  already contains `(0, 0, 0, 0)`.

For an indexed APNG with bit depth 2 or 4, the rectangle SHALL be extended to
the left edge of the canvas (x offset 0), because FFmpeg decodes such frames
wrongly at other x offsets.

When both ways are allowed, the engine SHALL write the one whose compressed
frame data is smaller, choosing changed area on a tie.

#### Scenario: Viewer without APNG support
- **WHEN** an APNG written by the engine is opened by a decoder that ignores APNG chunks
- **THEN** it shows the first frame of the animation

#### Scenario: Transparent pixel over an opaque one
- **WHEN** a pixel is opaque in frame 0 and fully transparent in frame 1
- **THEN** decoding frame 1 gives `(0, 0, 0, 0)` at that pixel, and frame 1 is written as a changed-area frame

#### Scenario: Small change
- **WHEN** a 64x32 timeline moves an opaque 8x8 sprite at (10, 4) one pixel to the right between two frames
- **THEN** the second frame's `fcTL` gives the 9x8 rectangle at (10, 4), not 64x32

#### Scenario: Indexed APNG at 2 bits per pixel
- **WHEN** an 8x8 square at (10, 4) changes color on a 64x32 canvas with 3 colors in total
- **THEN** the frame's `fcTL` gives the 18x8 rectangle at (0, 4)

#### Scenario: Scattered opaque changes
- **WHEN** two opaque pixels at opposite corners of a 64x64 RGBA canvas change color
- **THEN** the frame covers the whole 64x64 rectangle, and is written masked if that compresses smaller

#### Scenario: Split delay
- **WHEN** a frame is written as two identical frames because its delay numerator exceeds 65535
- **THEN** the second one is a 1x1 changed-area frame at (0, 0)

#### Scenario: Pixels unchanged after decoding
- **WHEN** an animation is decoded frame by frame, compositing each frame as the APNG specification defines, by Pillow, by FFmpeg or by the engine's tests
- **THEN** every canvas equals the corresponding rendered frame, exactly as before this change

### Requirement: Exact frame delays
A frame lasting `k` ticks of a clock of `F` ticks per second SHALL have the
delay `k / F` seconds, written as `delay_num / delay_den` reduced to lowest
terms. If, for some pair of consecutive change ticks (see the animation
capability), the reduced denominator of their distance exceeds 65535,
rendering SHALL fail before anything is written, with an error pointing to the
resource and listing the `fps` values it involves. Consecutive identical frames
SHALL be merged only if the merged delay has a denominator of at most 65535;
otherwise they SHALL stay separate frames. If the reduced numerator `n`
exceeds 65535, the frame
SHALL be written as `floor(n / 65535)` identical frames with delay `65535 / d`
followed, if `n mod 65535` is not 0, by one identical frame with delay
`(n mod 65535) / d` reduced, where `d` is the reduced denominator. The engine
SHALL NOT approximate delays.

#### Scenario: Reduced delay
- **WHEN** a frame lasts 6 ticks of a 24 ticks-per-second clock
- **THEN** its delay is `1/4`

#### Scenario: Long still frame
- **WHEN** a frame lasts 70000 ticks of a 1 tick-per-second clock
- **THEN** it is written as a frame with delay `65535/1` followed by an identical frame with delay `4465/1`

#### Scenario: Two rates always fit
- **WHEN** an animated panel plays flipbooks at `fps` 97 and 89
- **THEN** every frame delay has a denominator of at most 8633 and the panel renders without error

#### Scenario: Denominator too large
- **WHEN** a scene has a timeline layer at `fps` 97 whose flipbook element at `fps` 89 is moved by a `seekFlipbook` at 1, and an animated panel layer playing a flipbook at `fps` 83, so that some change ticks are 1 tick apart on a 716539 ticks-per-second clock
- **THEN** rendering fails before anything is written with an error pointing to the scene that lists the rates 83, 89 and 97

#### Scenario: Merge limited by the denominator
- **WHEN** two consecutive frames have identical pixels and their merged delay would have a denominator above 65535
- **THEN** they are written as two frames

### Requirement: APNG encoding chosen by color count
For an animated resource, the engine SHALL count the distinct RGBA colors over
all frames together (after canonical transparency). If there are at most 256,
it SHALL write an indexed APNG (color type 3) with a single `PLTE` and, when
needed, a single `tRNS` shared by all frames, with the smallest bit depth among
1, 2, 4 and 8 that fits the count. The palette SHALL follow the order of the
png-export capability (colors with alpha below 255 first, then opaque ones),
each group in order of first appearance scanning the frames in order and each
frame in row-major order. Otherwise it SHALL write an 8-bit RGBA APNG (color
type 6).

#### Scenario: Shared palette
- **WHEN** frame 0 uses red and green and frame 1 uses green and blue, all opaque
- **THEN** the APNG is indexed with bit depth 2 and palette red, green, blue

#### Scenario: Colors spread over frames
- **WHEN** each of 3 frames uses 100 different colors (300 in total)
- **THEN** the APNG is RGBA with bit depth 8

### Requirement: Lossless and deterministic APNG
Decoding each frame of a written APNG SHALL give exactly the corresponding
rendered frame, after `--scale`. Rendering the same worksheet with the same
options and the same engine version SHALL produce byte-identical files. The
file SHALL contain only `IHDR`, `acTL`, `PLTE` and `tRNS` when indexed, `fcTL`,
`IDAT`, `fdAT` and `IEND`, in this order:

- the PNG signature, `IHDR`, `acTL`, then `PLTE` and `tRNS` when indexed;
- for each frame, in playback order, its `fcTL`, then its data: in `IDAT`
  chunks for the first frame, in `fdAT` chunks for the others;
- `IEND`.

The `fcTL` and `fdAT` chunks SHALL carry consecutive sequence numbers starting
at 0, in file order. `IDAT` chunks have no sequence number. Each frame's data
SHALL be a complete zlib stream of its own, holding the filtered rows of that
frame's rectangle. It SHALL be split into several chunks only when it exceeds
the largest chunk payload: 2^31 − 1 bytes for `IDAT`, 2^31 − 5 bytes of data
after the sequence number for `fdAT`.

#### Scenario: Round trip
- **WHEN** a written APNG is decoded frame by frame to RGBA8
- **THEN** every frame equals the rendered frame and every delay equals the frame's duration

#### Scenario: Repeated render
- **WHEN** a worksheet with a timeline is rendered twice
- **THEN** the two APNG files are byte-identical

#### Scenario: Single frame
- **WHEN** a timeline's frames all merge into one
- **THEN** the file is still an APNG with one frame whose delay is the whole loop (split as above if its numerator exceeds 65535)

#### Scenario: Chunk order and sequence numbers
- **WHEN** an animation with three frames is written
- **THEN** the chunks after the header are `fcTL` (0), `IDAT`, `fcTL` (1), `fdAT` (2), `fcTL` (3), `fdAT` (4), `IEND`

#### Scenario: Frames are independent zlib streams
- **WHEN** the data of any frame of a written APNG is taken alone
- **THEN** it inflates by itself to that frame's filtered rows

#### Scenario: Same bytes as before
- **WHEN** the golden fixtures are rendered after the encoder is rewritten
- **THEN** every APNG file is byte-identical to the committed golden file
