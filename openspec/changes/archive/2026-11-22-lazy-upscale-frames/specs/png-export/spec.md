## MODIFIED Requirements

### Requirement: Image size limit
`render` SHALL accept `--max-image-size <SIZE>`, where SIZE is a positive
integer number of bytes optionally followed, without spaces, by one of the
binary units `KiB`, `MiB`, `GiB`, `TiB` (case-insensitive), or the word `none`.
The default SHALL be `1GiB`. Before rendering, the engine SHALL compute the size
of every image it would build for the selected exports (sprite rasters, the
result of each transform, including transforms added by `appendTransforms`,
panel and scene canvases, and the final image after `--scale`) as
`width × height × 4` bytes. For an animated resource, two sizes SHALL be
checked: one final frame, after `--scale`, and the number of frames before
merging (see the animation capability) times the size of one frame before
`--scale`. These are what the engine holds at most: frames are rendered and
merged at their size before `--scale`, and each is upscaled only when it is
encoded. If any of them exceeds SIZE, rendering SHALL fail with
an error pointing to the resource or element that produces it, giving the size
and the limit and suggesting `--max-image-size`; nothing SHALL be written and
the exit code SHALL be 1. With `none` there SHALL be no limit other than the
format's own (each side at most 2³¹ − 1 pixels for PNG, and at most 2³¹ − 1
frames for APNG). An invalid SIZE SHALL be a usage error with exit code 2.

#### Scenario: Default limit exceeded
- **WHEN** a static panel of 20000 x 20000 pixels (1.49 GiB as RGBA) is rendered without `--max-image-size`
- **THEN** rendering fails with an error pointing to the panel that mentions the 1 GiB limit and `--max-image-size`, no file is written and the exit code is 1

#### Scenario: Limit applies after scale
- **WHEN** a 4096 x 4096 panel (64 MiB) is rendered with `--scale 8` (4 GiB)
- **THEN** rendering fails because the final image exceeds the default limit

#### Scenario: Intermediate image
- **WHEN** an element has a `scale` transform to 30000 x 30000 inside a 16 x 16 panel
- **THEN** rendering fails with an error pointing to that element's transform, even though the panel itself is small

#### Scenario: Appended transform
- **WHEN** a timeline's `appendTransforms` adds a `scale` to 30000 x 30000 to an element of a 16 x 16 panel
- **THEN** rendering fails with an error pointing to that transition's transform

#### Scenario: Long animation
- **WHEN** a 512 x 512 timeline (1 MiB per frame) would produce 2000 frames before merging
- **THEN** rendering fails with an error pointing to the timeline that mentions the 1 GiB limit, before any frame is rendered

#### Scenario: Scaled animation within the limit
- **WHEN** a 256 x 256 sequence with 382 frames before merging (256 KiB each, 96 MiB in all) is rendered with `--scale 4`, so that each final frame is 1024 x 1024 (4 MiB)
- **THEN** the plan accepts it under the default 1 GiB limit, although 382 final frames would take 1.49 GiB

#### Scenario: Scaled animation over the limit
- **WHEN** a 32 x 32 timeline with 1000 frames before merging is rendered with `--scale 16` and `--max-image-size 2MiB`
- **THEN** rendering fails, because 1000 frames of 32 x 32 take 3.9 MiB, more than the limit

#### Scenario: Raised limit
- **WHEN** the 20000 x 20000 panel is rendered with `--max-image-size 2GiB`
- **THEN** the plan accepts it

#### Scenario: No limit
- **WHEN** `--max-image-size none` is given
- **THEN** no size limit is applied besides the PNG maximum side and the APNG maximum frame count

#### Scenario: Invalid size
- **WHEN** `--max-image-size 1GB`, `--max-image-size 0` or `--max-image-size big` is given
- **THEN** a usage error is printed and the exit code is 2
