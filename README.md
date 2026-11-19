# pixel-perfect

A Rust engine that renders **lossless pixel art** from a declarative worksheet.

You describe palettes, sprites, animations and compositions in a plain-text
file. pixel-perfect turns them into pixel-exact images: same input, same bytes,
every time. No antialiasing, no filtering, nearest-neighbor only.

> **Status:** early stage. Worksheets can be loaded and validated, and sprites,
> panels and scenes can be rendered to PNG. Animations (flipbooks, timelines,
> and panels or scenes that contain them) are not rendered yet. Development is planned with
> [OpenSpec](https://github.com/Fission-AI/OpenSpec) (see [`openspec/`](openspec/)).

## Usage

```sh
cargo build --release

# Check one or more worksheets without rendering.
pixel-perfect validate hero.json scenes.yaml palettes.toml
```

Every problem is printed to standard error as `<file>: <location>: <message>`,
where the location is a JSON Pointer into the worksheet (or `line:column` for
syntax errors):

```text
hero.json: /sprites/2/overrideLines/9: row index 9 is out of range: base sprite "hero" has 6 rows
```

Exit codes: `0` all files valid, `1` at least one invalid worksheet, `2` usage
error, unreadable file or unsupported extension.

### Render

```sh
# Write every exported resource under ./out, enlarged 4x.
pixel-perfect render hero.json --output-path out --scale 4
```

| Option | Default | Meaning |
|---|---|---|
| `-o`, `--output-path <dir>` | current directory | Root of the output tree. |
| `--format <format>` | `png` | Output format (only `png` for now). |
| `--scale <N>` | `1` | Integer upscale, 1–64: every pixel becomes an N×N block. |
| `--safe-build` | off | Name files by a hash of their id (see below). |
| `--max-image-size <SIZE>` | `1GiB` | Largest image the engine may build (see below). |

Files are written to `<output-path>/<collection>/<id>.<format>`, for example
`out/sprites/hero.png`. Ids that are not portable file names (containing `/`,
`:` and the like, or differing from another id only by letter case) are
rejected. With `--safe-build`, files are named
`<output-path>/<collection>/<hash>.<format>` instead, where `<hash>` is the
BLAKE2b-64 of the id (`printf '%s' hero | b2sum -l 64` → `8737b11b474b218b`).
Any id then works, and a file keeps its name as long as its id is unchanged.

Every render also writes `<output-path>/manifest.json`, mapping each exported
resource to its file and size, so tools never have to guess file names:

```json
{
  "format": "png",
  "scale": 4,
  "safeBuild": false,
  "files": {
    "/sprites/hero": { "path": "sprites/hero.png", "width": 28, "height": 24 }
  }
}
```

Each image is stored as an **indexed PNG** (1, 2, 4 or 8 bits per pixel) when
it has at most 256 distinct colors, and as **RGBA** otherwise. The count is
made on the final pixels, so the choice is always the smallest lossless one.
Output is deterministic: the same worksheet and options give byte-identical
files. Nothing is written if the worksheet or the export request is invalid,
and files from earlier renders are never deleted.

Exporting resources that cannot be rendered yet (flipbooks, timelines, and
panels or scenes containing them) is an error when they are listed in
`exports`; when `exports` is omitted they are skipped with a note.

Panels and scenes are composed with **source-over** on non-premultiplied alpha,
computed in integer arithmetic with a fixed rounding rule, so semi-transparent
pixels blend to the same bytes on every platform. Elements are drawn by
`zIndex` (ties in array order), after their own `transforms`, and clipped to
the canvas. In a scene, each panel is first flattened on its own canvas and
then composited as one layer.

`--max-image-size` guards against worksheets that would need huge amounts of
memory, for example a panel declared 100000×100000 or a `scale` transform to a
giant size. Every image the engine builds (sprites, transform results,
canvases, the final image after `--scale`) is measured as RGBA8 bytes before
anything is rendered; if one is over the limit, the render fails and names the
resource. SIZE is a number of bytes with an optional `KiB`, `MiB`, `GiB` or
`TiB` suffix, or `none` for no limit. It is a guard against mistakes, not a
memory budget: the peak memory use is a few times the largest image.

## Why

- **Assets as code.** Worksheets are text: they diff, review and version cleanly,
  and they build reproducibly in CI.
- **Cheap variants.** Palette inheritance and per-line overrides make palette
  swaps, damage states and skins a few lines each.
- **Generatable.** Worksheets are easy to produce from scripts or language models,
  and the schema validates whatever they produce.
- **Composition and animation.** Not just single images: layered scenes and
  timelines are part of the format.

## Formats

| Input (worksheet) | Status |
|---|---|
| JSON | planned (first) |
| YAML | planned (first) |
| TOML | planned (first) |
| XML (dedicated schema) | future |

JSON, YAML and TOML all map onto the same data model and are validated against
the same JSON Schema.

| Output | Status |
|---|---|
| PNG | planned (first) |
| APNG (animated PNG) | planned (first) |
| WebP / animated WebP (lossless) | later |

The output format and the integer upscale factor are chosen on the command line,
not in the worksheet.

## Concepts

| Resource | What it is |
|---|---|
| **palette** | Maps single characters to RGBA colors. Can derive from another palette and override or add colors. |
| **sprite** | A static image drawn as a grid of characters resolved through a palette. `.` is transparent. Can derive from another sprite, swapping the palette and/or overriding lines. |
| **flipbook** | A sprite-level animation: sprites shown in sequence on a fixed canvas. |
| **panel** | A single-plane composition of sprites and flipbooks. |
| **timeline** | A panel-level animation: transitions (move, show/hide, z-order, transforms) applied over time. |
| **scene** | A multi-plane composition stacking panels and timelines as layers. |
| **exports** | The resources to render, as pointers like `/sprites/hero`. If omitted, everything renderable is exported. |

Sprites and flipbook frames support `flip`, `rotate` (90/180/270), `scale`
(nearest-neighbor) and `crop` transforms.

## Example

A heart, a darker variant, and a two-frame beating animation built from them:

```json
{
  "$schema": "./schema/worksheet.schema.json",
  "palettes": [
    {
      "id": "warm",
      "colors": [
        { "char": "r", "rgba": [220, 40, 60] },
        { "char": "h", "rgba": [255, 255, 255, 0.6] }
      ]
    },
    {
      "id": "dark",
      "_palette": "warm",
      "colors": [{ "char": "r", "rgba": [120, 10, 30] }]
    }
  ],
  "sprites": [
    {
      "id": "heart",
      "_palette": "warm",
      "lines": [
        ".rr.rr.",
        "rhrrrrr",
        "rrrrrrr",
        ".rrrrr.",
        "..rrr..",
        "...r..."
      ]
    },
    { "id": "heart-dark", "_sprite": "heart", "_palette": "dark" }
  ],
  "flipbooks": [
    {
      "id": "beat",
      "width": 7,
      "height": 6,
      "fps": 4,
      "frames": [
        { "_sprite": "heart", "coordinates": [0, 0], "duration": 3 },
        { "_sprite": "heart-dark", "coordinates": [0, 0], "duration": 1 }
      ]
    }
  ],
  "exports": ["/sprites/heart", "/sprites/heart-dark"]
}
```

`pixel-perfect render heart.json -o out --scale 8` writes
`out/sprites/heart.png` and `out/sprites/heart-dark.png` (56×48 each). The
`beat` flipbook is already valid, but it is not exported yet: add
`"/flipbooks/beat"` to `exports` once animations can be rendered.

More worksheets are in [`examples/`](examples/).

## Schema

The worksheet format is defined by
[`schema/worksheet.schema.json`](schema/worksheet.schema.json) (JSON Schema draft
2020-12). Point the `$schema` field of a worksheet at it to get validation and
autocompletion in editors that support JSON Schema (VS Code, JetBrains IDEs, and
YAML/TOML language servers).

Rules the schema cannot express (references that resolve, no inheritance cycles,
rows of equal width, characters present in the palette, ...) are documented in
the schema as `$comment: "Engine: ..."` and enforced by the engine.

## License

Licensed under the [Apache License, Version 2.0](LICENSE).
