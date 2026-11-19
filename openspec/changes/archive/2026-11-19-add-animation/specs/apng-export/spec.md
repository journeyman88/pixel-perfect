## ADDED Requirements

### Requirement: APNG structure
Each animated resource SHALL be written as an APNG file. Its `acTL` chunk SHALL
give the number of written frames and `num_plays` 0 (loop forever). The default
image (`IDAT`) SHALL be the first frame and part of the animation. Every frame
SHALL cover the full canvas at offset (0, 0) with `dispose_op`
`APNG_DISPOSE_OP_NONE` and `blend_op` `APNG_BLEND_OP_SOURCE`, so that each frame
replaces the previous one.

#### Scenario: Viewer without APNG support
- **WHEN** an APNG written by the engine is opened by a decoder that ignores APNG chunks
- **THEN** it shows the first frame of the animation

#### Scenario: Transparent pixel over an opaque one
- **WHEN** a pixel is opaque in frame 0 and fully transparent in frame 1
- **THEN** decoding frame 1 gives `(0, 0, 0, 0)` at that pixel

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
`IDAT`, `fdAT` and `IEND`.

#### Scenario: Round trip
- **WHEN** a written APNG is decoded frame by frame to RGBA8
- **THEN** every frame equals the rendered frame and every delay equals the frame's duration

#### Scenario: Repeated render
- **WHEN** a worksheet with a timeline is rendered twice
- **THEN** the two APNG files are byte-identical

#### Scenario: Single frame
- **WHEN** a timeline's frames all merge into one
- **THEN** the file is still an APNG with one frame whose delay is the whole loop (split as above if its numerator exceeds 65535)
