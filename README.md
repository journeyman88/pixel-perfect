# pixel-perfect

A Rust engine that renders **lossless pixel art** from a declarative worksheet.

You describe palettes, sprites, animations and compositions in a plain-text
file. pixel-perfect turns them into pixel-exact images: same input, same bytes,
every time. No antialiasing, no filtering, nearest-neighbor only.

> **Status:** early stage. Worksheets can be loaded and validated; sprites and
> static panels and scenes render to PNG, and animations (flipbooks, timelines,
> and panels or scenes that contain them) render to APNG. Development is planned with
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
resource to its file and size, so tools never have to guess file names.
Animated resources also give the number of `frames` written and the `loop`
length in seconds as an exact fraction:

```json
{
  "format": "png",
  "scale": 4,
  "safeBuild": false,
  "files": {
    "/sprites/hero": { "path": "sprites/hero.png", "width": 28, "height": 24 },
    "/flipbooks/beat": {
      "path": "flipbooks/beat.png", "width": 28, "height": 24,
      "frames": 2, "loop": { "num": 1, "den": 1 }
    }
  }
}
```

Each image is stored as an **indexed PNG** (1, 2, 4 or 8 bits per pixel) when
it has at most 256 distinct colors, and as **RGBA** otherwise. The count is
made on the final pixels, so the choice is always the smallest lossless one.
Output is deterministic: the same worksheet and options give byte-identical
files. Nothing is written if the worksheet or the export request is invalid,
and files from earlier renders are never deleted.

Flipbooks, timelines, and panels or scenes that play a flipbook or a timeline
are written as **APNG** (still with the `.png` extension, at the same paths).
They loop forever, every frame covers the whole canvas, all frames share one
palette chosen by the same color count, and the first frame is the default
image, so viewers without APNG support show it. See [Animation](#animation).

Panels and scenes are composed with **source-over** on non-premultiplied alpha,
computed in integer arithmetic with a fixed rounding rule, so semi-transparent
pixels blend to the same bytes on every platform. Elements are drawn by
`zIndex` (ties in array order), after their own `transforms`, and clipped to
the canvas. In a scene, each panel is first flattened on its own canvas and
then composited as one layer.

`--max-image-size` guards against worksheets that would need huge amounts of
memory, for example a panel declared 100000×100000 or a `scale` transform to a
giant size. Every image the engine builds (sprites, transform results,
including transforms added by `appendTransforms`, canvases, the final image
after `--scale`) is measured as RGBA8 bytes before anything is rendered; if one
is over the limit, the render fails and names the resource. For an animation
the limit applies to all its frames together, since they are kept in memory
until the shared palette is known. SIZE is a number of bytes with an optional `KiB`, `MiB`, `GiB` or
`TiB` suffix, or `none` for no limit. It is a guard against mistakes, not a
memory budget: the peak memory use is a few times the largest image.

### Animation

Time is exact. Each animated resource runs on a clock whose rate is the least
common multiple of the `fps` values it involves (a 12 fps timeline with an
8 fps flipbook runs at 24 ticks per second), and every frame boundary is a whole
number of ticks. Each APNG frame delay is that duration as a reduced fraction
of a second, never rounded; frames too long for APNG's 16-bit delay are split
into identical frames. Consecutive identical frames are merged. A flipbook,
timeline, panel or scene layer loops on its own length, and the file loops on
the least common multiple of them all. In the rare case of `fps` values whose
combination needs a delay denominator above 65535, the render fails before
writing anything and lists the rates.

A timeline starts every loop from its panel's state. Instant transitions
(`setVisible`, `moveToZ`, `appendTransforms`, `resetTransforms`,
`seekFlipbook`) apply at frame-unit `at`, ties in array order. `moveTo` moves
linearly from where the element is at `start` to `coordinates` at
`start + duration`; the offset at frame-unit `t` is
`(p1 − p0) · (t − start) / duration` rounded to the nearest integer with halves
away from zero, so a movement to the left mirrors the same movement to the
right. Flipbook elements play at their own `fps`, restart at every timeline
loop, and `seekFlipbook` jumps them to the start of `frames[frameIndex]`.

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
| JSON | supported |
| YAML | supported |
| TOML | supported |
| XML (dedicated schema) | future |

JSON, YAML and TOML all map onto the same data model and are validated against
the same JSON Schema.

| Output | Status |
|---|---|
| PNG | supported |
| APNG (animated PNG) | supported |
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
(nearest-neighbor), `crop` and `tile` transforms. `tile` repeats the image from
its top-left corner to an exact `width` x `height`, cutting the tiles that do
not fit, so a floor or wall made of one small tile is a single element.

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
`out/sprites/heart.png` and `out/sprites/heart-dark.png` (56×48 each). Add
`"/flipbooks/beat"` to `exports` to also get `out/flipbooks/beat.png`, an APNG
that loops every second (3/4 s bright, 1/4 s dark).

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
