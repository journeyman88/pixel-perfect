# png-export Specification

## Purpose

Write rendered resources as lossless, deterministic PNG files through the
`render` command: output layout, file-safe ids and `--safe-build` hashed names,
the manifest, export selection, the image size limit, integer upscale, and the
choice between indexed and RGBA encoding by counting the colors of the final
image.

## Requirements

### Requirement: Render command
The CLI SHALL provide `pixel-perfect render <worksheet> [--output-path <dir>]
[--format <format>] [--scale <N>] [--safe-build] [--single-frames]
[--max-image-size <SIZE>] [--threads <N>]`, with `-o` as a short alias of
`--output-path`. It
SHALL load and validate the worksheet as `validate` does and write nothing if
the worksheet is invalid. `--output-path` SHALL default to the current
directory. `--format` SHALL accept `png` and `webp` and SHALL default to `png`.
`--single-frames` is described in the single-frame-export capability.

`--threads` SHALL set the number of worker threads used for rendering and
encoding, an integer from 1 to 65535. Without it, the `RAYON_NUM_THREADS`
environment variable SHALL be used when it is set to a positive integer.
Otherwise the default SHALL be the number of physical CPU cores, at most the
number of CPUs the process may use. The written files SHALL be byte-identical
whatever the number of threads.

#### Scenario: Render a valid worksheet
- **WHEN** `pixel-perfect render hero.json --output-path out` runs on a valid worksheet with sprite `hero`
- **THEN** `out/sprites/hero.png` and `out/manifest.json` are written and the exit code is 0

#### Scenario: Render as WebP
- **WHEN** `pixel-perfect render hero.json -o out --format webp` runs on the same worksheet
- **THEN** `out/sprites/hero.webp` and `out/manifest.json` (with `options.format` `"webp"`) are written and the exit code is 0

#### Scenario: Render single frames
- **WHEN** `pixel-perfect render spin.json -o out --single-frames` runs on a worksheet with flipbook `spin`
- **THEN** the frames are written under `out/flipbooks/spin/`, `out/manifest.json` has `options.singleFrames` true, and the exit code is 0

#### Scenario: Invalid worksheet
- **WHEN** `pixel-perfect render broken.json -o out` runs on an invalid worksheet
- **THEN** the diagnostics are printed as by `validate`, no file is written and the exit code is 1

#### Scenario: Unknown format
- **WHEN** `pixel-perfect render hero.json --format gif` runs
- **THEN** a usage error is printed and the exit code is 2

#### Scenario: Thread count does not change the output
- **WHEN** an animated worksheet is rendered once with `--threads 1` and once with `--threads 4`, as PNG, as WebP and with `--single-frames`
- **THEN** each pair of renders writes byte-identical files

#### Scenario: Default thread count
- **WHEN** `pixel-perfect render hero.json` runs without `--threads` and without `RAYON_NUM_THREADS` on a machine with 8 physical cores and 16 logical CPUs
- **THEN** 8 worker threads are used

#### Scenario: Option over environment
- **WHEN** `RAYON_NUM_THREADS=5 pixel-perfect render hero.json --threads 2` runs
- **THEN** 2 worker threads are used

#### Scenario: Zero threads
- **WHEN** `pixel-perfect render hero.json --threads 0` runs
- **THEN** a usage error is printed and the exit code is 2

### Requirement: Output paths
By default, each exported resource SHALL be written to
`<output-path>/<collection>/<id>.<format>`, where `<collection>` is the worksheet
collection name (`sprites`, `flipbooks`, `panels`, `timelines`, `scenes`,
`sequences`). With `--single-frames`, an animated resource SHALL instead be
written as a directory of frames, `<output-path>/<collection>/<id>/`. The
single-frame-export capability describes its layout. Missing directories SHALL
be created. An existing file SHALL be overwritten when its content differs from
the new one, and left untouched (content and modification time) when it is
already byte-identical. This also applies to `manifest.json`. A file that is
written SHALL be written in place, keeping its identity on disk, so that tools
watching it see a modification and not a replacement. Files not produced by
the current render SHALL NOT be deleted.

#### Scenario: Nested output
- **WHEN** sprites `hero` and `coin` are rendered with `--output-path build/art`
- **THEN** the files are `build/art/sprites/hero.png` and `build/art/sprites/coin.png`

#### Scenario: Same id in different collections
- **WHEN** a sprite and a panel are both called `hero` and both are exported
- **THEN** they are written to `sprites/hero.png` and `panels/hero.png` without conflict

#### Scenario: Sequence output
- **WHEN** sequence `parallax` is exported
- **THEN** it is written to `sequences/parallax.png` with the size of its scene

#### Scenario: Sequence output as single frames
- **WHEN** sequence `parallax` is exported with `--single-frames`
- **THEN** its frames are written under `sequences/parallax/`, starting with `sequences/parallax/0000.png`

#### Scenario: Unchanged files are not rewritten
- **WHEN** a worksheet with sprites `hero` and `coin` is rendered, only `coin` is then changed, and it is rendered again to the same output path
- **THEN** `sprites/coin.png` is rewritten, while `sprites/hero.png` keeps its previous modification time

#### Scenario: Changed file is rewritten
- **WHEN** an output file exists with different content, for example from another worksheet
- **THEN** it is overwritten with the new content

### Requirement: File-safe ids
Without `--safe-build`, before writing anything, the engine SHALL reject
exported ids that cannot be used as a portable file name: ids containing `/`,
`\`, `:`, `*`, `?`, `"`, `<`, `>`, `|` or control characters, ids equal to `.`
or `..`, ids ending with a dot or a space, and ids that differ only by letter
case from another exported id in the same collection. The error message SHALL
suggest `--safe-build`.

#### Scenario: Path separator in an id
- **WHEN** an exported sprite has `id` `ui/button` and `--safe-build` is not given
- **THEN** rendering fails with an error pointing to that `id`, mentioning `--safe-build`, and no file is written

#### Scenario: Case-insensitive collision
- **WHEN** sprites `Hero` and `hero` are both exported and `--safe-build` is not given
- **THEN** rendering fails with an error pointing to the second `id`, because they would collide on case-insensitive file systems

### Requirement: Safe build
With `--safe-build`, each exported resource SHALL be written to
`<output-path>/<collection>/<hash>.<format>`, where `<hash>` is the BLAKE2b
digest with an output length of 8 bytes (BLAKE2b-64, unkeyed, as computed by
`b2sum -l 64`) of the UTF-8 bytes of the id, written as 16 lowercase hexadecimal
digits. Every id SHALL be accepted. If two exported ids of the same collection
produce the same hash, rendering SHALL fail before writing anything.

#### Scenario: Hashed file name
- **WHEN** sprite `hero` is rendered with `--safe-build`
- **THEN** it is written to `sprites/8737b11b474b218b.png`

#### Scenario: Ids that differ only by case
- **WHEN** sprites `hero` and `Hero` are rendered with `--safe-build`
- **THEN** they are written to `sprites/8737b11b474b218b.png` and `sprites/b46e11bab0d34796.png`

#### Scenario: Id that is not a valid file name
- **WHEN** sprite `ui/button` is rendered with `--safe-build`
- **THEN** rendering succeeds and the manifest maps `/sprites/ui/button` to its hashed file

#### Scenario: Stable name
- **WHEN** the pixels of sprite `hero` change and the worksheet is rendered again with `--safe-build`
- **THEN** the file name stays `sprites/8737b11b474b218b.png`

### Requirement: Manifest
Every successful render SHALL write `<output-path>/manifest.json`, overwriting
any previous one. The manifest SHALL be valid against
`schema/manifest.schema.json`. That schema is the normative definition of the
format, and the rules below are how the engine fills it in.

The root SHALL contain `options` and `files`.

`options` SHALL contain:
- `format`: the output format;
- `scale`;
- `safeBuild`: whether `--safe-build` was used;
- `singleFrames`: whether `--single-frames` was used.

All four SHALL be written even when they have their default values.

`files` SHALL be an array with one entry per exported resource, sorted by `id`.
It SHALL be empty when nothing is exported. Each entry SHALL give:
- `id`: the export pointer `/<collection>/<id>`, with the id as written in the
  worksheet;
- `kind`;
- `width` and `height`: the size in pixels after scaling, which for a frame
  collection is the size of every frame.

The other fields depend on `kind`:
- `"static"`, for static resources. The entry also gives `path`, the path of
  the file relative to the output path, with `/` separators.
- `"animation"`, for animated resources without `--single-frames`. The entry
  also gives `path`, `frameCount` (the number of frames written to the file)
  and `loop`.
- `"frameCollection"`, for animated resources with `--single-frames`. The entry
  also gives `loop` and `frames`. `frames` lists the frames in playback order,
  each with its `index` (starting at 0), its `path` and its `duration`.

`loop` and `duration` are lengths in seconds, written as an object
`{ "num": n, "den": d }` reduced to lowest terms. Fields SHALL be written in a
fixed order, and the file SHALL be pretty-printed JSON ending with a newline,
so it is byte-identical for identical renders.

#### Scenario: Manifest content
- **WHEN** sprite `hero` (16x16) is rendered with `--scale 2`
- **THEN** `manifest.json` has `options` = `{ "format": "png", "scale": 2, "safeBuild": false, "singleFrames": false }` and `files` = `[{ "id": "/sprites/hero", "kind": "static", "path": "sprites/hero.png", "width": 32, "height": 32 }]`

#### Scenario: Manifest in safe build
- **WHEN** sprite `hero` is rendered with `--safe-build`
- **THEN** the entry with `id` `/sprites/hero` has `path` `sprites/8737b11b474b218b.png` and `options.safeBuild` is true

#### Scenario: Animated entry
- **WHEN** flipbook `spin` (8x8, `fps` 12, frame durations 2, 2, 1, 1, all different) is rendered
- **THEN** its entry is `{ "id": "/flipbooks/spin", "kind": "animation", "path": "flipbooks/spin.png", "width": 8, "height": 8, "frameCount": 4, "loop": { "num": 1, "den": 2 } }`

#### Scenario: Frame collection entry
- **WHEN** the same flipbook is rendered with `--single-frames`
- **THEN** its entry has `kind` `"frameCollection"`, `width` 8, `height` 8, `loop` `{ "num": 1, "den": 2 }`, no `path` and no `frameCount`, and `frames` = `[{ "index": 0, "path": "flipbooks/spin/0000.png", "duration": { "num": 1, "den": 6 } }, …]` with 4 frames whose durations are 1/6, 1/6, 1/12 and 1/12

#### Scenario: Entries sorted by id
- **WHEN** sprites `hero` and `coin` and flipbook `spin` are exported, with `exports` listing them in that order
- **THEN** the `id`s in `files` are, in order, `/flipbooks/spin`, `/sprites/coin`, `/sprites/hero`

#### Scenario: Nothing to export
- **WHEN** a worksheet that defines only palettes is rendered
- **THEN** rendering succeeds and `files` is an empty array

#### Scenario: Valid against the schema
- **WHEN** any example or test fixture is rendered, as PNG or WebP, at any scale, with or without `--safe-build`, with or without `--single-frames`
- **THEN** the written `manifest.json` is valid against `schema/manifest.schema.json`

### Requirement: Export selection
The engine SHALL render the resources listed in `exports`, or, when `exports` is
absent, every resource of every collection except palettes. With `--format png`,
sprites, static panels and static scenes SHALL be written as PNG, and
flipbooks, timelines, sequences and animated panels and scenes (see the
composition capability) SHALL be written as APNG (see the apng-export
capability), with the `.png` extension and the same output paths, even when the
animation has a single frame. With `--format webp`, the same resources SHALL be
written as static and animated WebP respectively (see the webp-export
capability), with the `.webp` extension and the same output paths. With
`--single-frames`, the animated resources SHALL instead be written as frame
collections (see the single-frame-export capability), and the static ones SHALL
be written as without the flag.

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

#### Scenario: Animated resources as WebP
- **WHEN** the same exports are rendered with `--format webp`
- **THEN** `flipbooks/spin.webp`, `timelines/walk.webp` and `scenes/night.webp` are written as animated WebP, and a static panel would be written as a static WebP

#### Scenario: Animated resources as single frames
- **WHEN** the same exports are rendered with `--single-frames`
- **THEN** `flipbooks/spin/`, `timelines/walk/` and `scenes/night/` each contain one PNG per frame, and none of them is an APNG

#### Scenario: Implicit export of a sequence
- **WHEN** `exports` is absent and the worksheet defines scene `night` and sequence `flicker` over it
- **THEN** `scenes/night.png` and `sequences/flicker.png` are both written, the sequence as APNG

### Requirement: Integer upscale
With `--scale N` (an integer from 1 to 64), the engine SHALL enlarge each final
image by N in both directions, turning every pixel into an N x N block.

#### Scenario: Scale 3
- **WHEN** a 2x1 sprite is rendered with `--scale 3`
- **THEN** the written image is 6x3 and every source pixel fills a 3x3 block

#### Scenario: Invalid scale
- **WHEN** `--scale 0` is given
- **THEN** a usage error is printed and the exit code is 2

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

### Requirement: Deterministic indexed palette
In an indexed PNG, the palette SHALL list the colors with alpha below 255 first
and the opaque colors after them, each group in order of first appearance in
row-major order. A `tRNS` chunk SHALL be written only if some color has alpha
below 255, and SHALL contain exactly the entries of the non-opaque group.

#### Scenario: Palette order
- **WHEN** an image in row-major order contains red, transparent, green, then half-transparent blue
- **THEN** the palette is transparent, half-transparent blue, red, green and `tRNS` has 2 entries

#### Scenario: Opaque image
- **WHEN** every pixel of an image is opaque
- **THEN** no `tRNS` chunk is written

### Requirement: Lossless and deterministic output
Decoding a written PNG SHALL give exactly the final RGBA image. Rendering the
same worksheet with the same options and the same engine version SHALL produce
byte-identical files. The engine SHALL NOT write time, text or other metadata
chunks.

#### Scenario: Round trip
- **WHEN** a written PNG is decoded to RGBA8
- **THEN** every pixel equals the rendered pixel, including canonical transparency

#### Scenario: Repeated render
- **WHEN** the same worksheet is rendered twice
- **THEN** the two output files are byte-identical

#### Scenario: Minimal chunks
- **WHEN** a written PNG is inspected
- **THEN** it contains only `IHDR`, `PLTE` and `tRNS` when indexed, `IDAT` and `IEND`

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
