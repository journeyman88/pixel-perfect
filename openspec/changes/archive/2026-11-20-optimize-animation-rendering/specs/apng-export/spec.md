## MODIFIED Requirements

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
