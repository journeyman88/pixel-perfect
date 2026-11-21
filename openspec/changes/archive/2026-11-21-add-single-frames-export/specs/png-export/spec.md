## MODIFIED Requirements

### Requirement: Render command
The CLI SHALL provide `pixel-perfect render <worksheet> [--output-path <dir>]
[--format <format>] [--scale <N>] [--safe-build] [--single-frames]
[--max-image-size <SIZE>]`, with `-o` as a short alias of `--output-path`. It
SHALL load and validate the worksheet as `validate` does and write nothing if
the worksheet is invalid. `--output-path` SHALL default to the current
directory. `--format` SHALL accept `png` and `webp` and SHALL default to `png`.
`--single-frames` is described in the single-frame-export capability.

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

### Requirement: Output paths
By default, each exported resource SHALL be written to
`<output-path>/<collection>/<id>.<format>`, where `<collection>` is the worksheet
collection name (`sprites`, `flipbooks`, `panels`, `timelines`, `scenes`,
`sequences`). With `--single-frames`, an animated resource SHALL instead be
written as a directory of frames, `<output-path>/<collection>/<id>/`. The
single-frame-export capability describes its layout. Missing directories SHALL
be created and existing files SHALL be overwritten. Files not produced by the
current render SHALL NOT be deleted.

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
