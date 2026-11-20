## MODIFIED Requirements

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
