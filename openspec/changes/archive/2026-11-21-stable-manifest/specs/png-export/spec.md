## MODIFIED Requirements

### Requirement: Render command
The CLI SHALL provide `pixel-perfect render <worksheet> [--output-path <dir>]
[--format <format>] [--scale <N>] [--safe-build] [--max-image-size <SIZE>]`,
with `-o` as a short alias of `--output-path`. It SHALL load and validate the
worksheet as `validate` does and write nothing if the worksheet is invalid.
`--output-path` SHALL default to the current directory. `--format` SHALL accept
`png` and `webp` and SHALL default to `png`.

#### Scenario: Render a valid worksheet
- **WHEN** `pixel-perfect render hero.json --output-path out` runs on a valid worksheet with sprite `hero`
- **THEN** `out/sprites/hero.png` and `out/manifest.json` are written and the exit code is 0

#### Scenario: Render as WebP
- **WHEN** `pixel-perfect render hero.json -o out --format webp` runs on the same worksheet
- **THEN** `out/sprites/hero.webp` and `out/manifest.json` (with `options.format` `"webp"`) are written and the exit code is 0

#### Scenario: Invalid worksheet
- **WHEN** `pixel-perfect render broken.json -o out` runs on an invalid worksheet
- **THEN** the diagnostics are printed as by `validate`, no file is written and the exit code is 1

#### Scenario: Unknown format
- **WHEN** `pixel-perfect render hero.json --format gif` runs
- **THEN** a usage error is printed and the exit code is 2

### Requirement: Manifest
Every successful render SHALL write `<output-path>/manifest.json`, overwriting
any previous one. The manifest SHALL be valid against
`schema/manifest.schema.json`. That schema is the normative definition of the
format, and the rules below are how the engine fills it in.

The root SHALL contain `options` and `files`.

`options` SHALL contain:
- `format`: the output format;
- `scale`;
- `safeBuild`: whether `--safe-build` was used.

It SHALL NOT contain `singleFrames`, because single-frame output is not
supported.

`files` SHALL be an array with one entry per exported resource, sorted by `id`.
It SHALL be empty when nothing is exported. Each entry SHALL give:
- `id`: the export pointer `/<collection>/<id>`, with the id as written in the
  worksheet;
- `kind`;
- `path`: the path of the file, relative to the output path, with `/`
  separators;
- `width` and `height`: the size in pixels after scaling.

Static resources SHALL have `kind` `"static"` and no other fields. Animated
resources SHALL have `kind` `"animation"` and SHALL also give:
- `frameCount`: the number of frames written to the file;
- `loop`: the loop length in seconds, as an object `{ "num": n, "den": d }`
  reduced to lowest terms.

The engine SHALL NOT write `kind` `"frameCollection"`. Fields SHALL be written
in a fixed order, and the file SHALL be pretty-printed JSON ending with a
newline, so it is byte-identical for identical renders.

#### Scenario: Manifest content
- **WHEN** sprite `hero` (16x16) is rendered with `--scale 2`
- **THEN** `manifest.json` has `options` = `{ "format": "png", "scale": 2, "safeBuild": false }` and `files` = `[{ "id": "/sprites/hero", "kind": "static", "path": "sprites/hero.png", "width": 32, "height": 32 }]`

#### Scenario: Manifest in safe build
- **WHEN** sprite `hero` is rendered with `--safe-build`
- **THEN** the entry with `id` `/sprites/hero` has `path` `sprites/8737b11b474b218b.png` and `options.safeBuild` is true

#### Scenario: Animated entry
- **WHEN** flipbook `spin` (8x8, `fps` 12, frame durations 2, 2, 1, 1, all different) is rendered
- **THEN** its entry is `{ "id": "/flipbooks/spin", "kind": "animation", "path": "flipbooks/spin.png", "width": 8, "height": 8, "frameCount": 4, "loop": { "num": 1, "den": 2 } }`

#### Scenario: Entries sorted by id
- **WHEN** sprites `hero` and `coin` and flipbook `spin` are exported, with `exports` listing them in that order
- **THEN** the `id`s in `files` are, in order, `/flipbooks/spin`, `/sprites/coin`, `/sprites/hero`

#### Scenario: Nothing to export
- **WHEN** a worksheet that defines only palettes is rendered
- **THEN** rendering succeeds and `files` is an empty array

#### Scenario: Valid against the schema
- **WHEN** any example or test fixture is rendered, as PNG or WebP, at any scale, with or without `--safe-build`
- **THEN** the written `manifest.json` is valid against `schema/manifest.schema.json`
