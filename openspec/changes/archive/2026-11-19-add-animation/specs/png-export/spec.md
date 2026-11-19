## MODIFIED Requirements

### Requirement: Export selection
The engine SHALL render the resources listed in `exports`, or, when `exports` is
absent, every resource of every collection except palettes. Sprites, static
panels and static scenes SHALL be written as PNG. Flipbooks, timelines and
animated panels and scenes (see the composition capability) SHALL be written as
APNG (see the apng-export capability), with the `.png` extension and the same
output paths, even when the animation has a single frame.

#### Scenario: Explicit exports
- **WHEN** `exports` is `["/sprites/hero"]` and the worksheet also defines sprite `coin`
- **THEN** only `sprites/hero.png` is written

#### Scenario: Implicit exports
- **WHEN** `exports` is absent and the worksheet defines two sprites, a static panel and a flipbook
- **THEN** both sprites and the panel are written as PNG, the flipbook is written as APNG, and no notice is printed

#### Scenario: Static panel and scene
- **WHEN** `exports` is `["/panels/stage", "/scenes/night"]` and both are static
- **THEN** `panels/stage.png` and `scenes/night.png` are written with the panel and scene sizes

#### Scenario: Animated resources
- **WHEN** `exports` contains `/flipbooks/spin`, `/timelines/walk` and `/scenes/night`, and `night` has a `_timeline` layer
- **THEN** `flipbooks/spin.png`, `timelines/walk.png` and `scenes/night.png` are written as APNG with the flipbook, panel and scene sizes

### Requirement: Encoding chosen by color count
For each static resource, the engine SHALL count the distinct RGBA colors of its
final image (after canonical transparency). If there are at most 256 colors, it
SHALL write an indexed PNG (color type 3) with the smallest bit depth among 1,
2, 4 and 8 that fits the count. Otherwise it SHALL write an 8-bit RGBA PNG
(color type 6). Animated resources follow the same rule over all their frames
(see the apng-export capability).

#### Scenario: Few colors
- **WHEN** a sprite uses 3 colors plus transparency (4 distinct RGBA values)
- **THEN** the PNG is indexed with bit depth 2

#### Scenario: Single color
- **WHEN** a sprite is entirely one color
- **THEN** the PNG is indexed with bit depth 1

#### Scenario: Many colors
- **WHEN** a final image has 300 distinct colors
- **THEN** the PNG is RGBA with bit depth 8

### Requirement: Manifest
Every successful render SHALL write `<output-path>/manifest.json`, overwriting
any previous one. It SHALL contain the output `format`, the `scale`, whether
`safeBuild` was used, and a `files` object keyed by export pointer
(`/<collection>/<id>`), whose values give the `path` of the file relative to the
output path (with `/` separators) and its `width` and `height` in pixels after
scaling. Entries of animated resources SHALL also give `frames`, the number of
frames written, and `loop`, the loop length in seconds as an object
`{ "num": n, "den": d }` reduced to lowest terms. Keys SHALL be sorted, and the
file SHALL be pretty-printed JSON ending with a newline, so it is
byte-identical for identical renders.

#### Scenario: Manifest content
- **WHEN** sprite `hero` (16x16) is rendered with `--scale 2`
- **THEN** `manifest.json` has `format` `"png"`, `scale` 2, `safeBuild` false, and `files["/sprites/hero"]` = `{ "path": "sprites/hero.png", "width": 32, "height": 32 }`

#### Scenario: Manifest in safe build
- **WHEN** sprite `hero` is rendered with `--safe-build`
- **THEN** `files["/sprites/hero"].path` is `sprites/8737b11b474b218b.png` and `safeBuild` is true

#### Scenario: Animated entry
- **WHEN** flipbook `spin` (8x8, `fps` 12, frame durations 2, 2, 1, 1, all different) is rendered
- **THEN** `files["/flipbooks/spin"]` = `{ "path": "flipbooks/spin.png", "width": 8, "height": 8, "frames": 4, "loop": { "num": 1, "den": 2 } }`

### Requirement: Image size limit
`render` SHALL accept `--max-image-size <SIZE>`, where SIZE is a positive
integer number of bytes optionally followed, without spaces, by one of the
binary units `KiB`, `MiB`, `GiB`, `TiB` (case-insensitive), or the word `none`.
The default SHALL be `1GiB`. Before rendering, the engine SHALL compute the size
of every image it would build for the selected exports (sprite rasters, the
result of each transform, including transforms added by `appendTransforms`,
panel and scene canvases, and the final image after `--scale`) as
`width × height × 4` bytes. For an animated resource, the final size SHALL be
the number of frames before merging (see the animation capability) times the
size of one final frame. If any of them exceeds SIZE, rendering SHALL fail with
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

#### Scenario: Raised limit
- **WHEN** the 20000 x 20000 panel is rendered with `--max-image-size 2GiB`
- **THEN** the plan accepts it

#### Scenario: No limit
- **WHEN** `--max-image-size none` is given
- **THEN** no size limit is applied besides the PNG maximum side and the APNG maximum frame count

#### Scenario: Invalid size
- **WHEN** `--max-image-size 1GB`, `--max-image-size 0` or `--max-image-size big` is given
- **THEN** a usage error is printed and the exit code is 2
