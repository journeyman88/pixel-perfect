# pixel-perfect

A Rust engine that renders **lossless pixel art** from a declarative worksheet.

You describe palettes, sprites, animations and compositions in a plain-text
file. pixel-perfect turns them into pixel-exact images: same input, same bytes,
every time. No antialiasing, no filtering, nearest-neighbor only.

> **Status:** early stage. Worksheets can be loaded and validated; sprites and
> static panels and scenes render to PNG, and animations (flipbooks, timelines,
> sequences, and panels or scenes that contain them) render to APNG. Existing PNG pixel art
> can be imported into a worksheet. Development is planned with
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
| `--format <format>` | `png` | Output format: `png` or `webp` (see [WebP](#webp)). |
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
Output is deterministic: the same compiled engine with the same worksheet
and options gives byte-identical files. Nothing is written if the worksheet
or the export request is invalid, and files from earlier renders are never
deleted.

Flipbooks, timelines, and panels or scenes that play a flipbook or a timeline
are written as **APNG** (still with the `.png` extension, at the same paths).
They loop forever, all frames share one palette chosen by the same color
count, and the first frame is the default image, so viewers without APNG
support show it. After the first frame, each frame stores only the rectangle
that changed since the previous one, either overwriting it or, when that is
smaller and every changed pixel is opaque, blending it over the previous frame
with the unchanged pixels left transparent. Decoders see exactly the rendered
frames; on the examples this makes animations about 7× smaller than full
frames. See [Animation](#animation).

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

### WebP

With `--format webp` the same resources are written as lossless **WebP**
(`.webp`, same paths): static resources as simple WebP, animated ones as
animated WebP that loops forever, each frame after the first overwriting only
the rectangle that changed (extended to even coordinates, as WebP requires). It is
pure Rust (the `image-webp` encoder, with the container written by the engine),
deterministic like PNG, and accepted by libwebp.

WebP stores frame durations in whole milliseconds, so each frame's end time is
rounded to the nearest millisecond (halves up) and durations are the
differences: errors never add up, and a loop that lasts a whole number of
milliseconds keeps its exact length (`loop` in the manifest stays exact).
Frames shorter than half a millisecond are dropped. Combinations of `fps` that
APNG cannot store exactly are fine in WebP. Each side of a WebP image is at
most 16384 pixels, checked before anything is written.

**PNG is the better choice for now.** The current encoder does not use
WebP's palette transform, so on the examples every WebP file is larger than
its PNG: about 3.5× for stills and 2.5× for animations. WebP frames are never
blended over the previous one, because `image-webp` does not composite opaque
pixels exactly. WebP is there for pipelines that require it.

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

### Sequences

A sequence animates the layers of a scene the way a timeline animates the
elements of a panel. It has its own `fps` and `duration`, and its `actions`
target layers by `id` (`_layer`):

- `moveTo` moves a layer linearly, with the same rounding as timelines. Use
  it for parallax: planes wider than the canvas, made with `tile`, moved by
  exactly one tile width per loop, loop seamlessly.
- `setVisible` shows or hides a layer at `at`, starting from its `visible`
  in the scene.
- `moveToZ` changes a layer's `zIndex` at `at`, for example to bring a light
  in front of a plane.

Time is global: animated layers (timelines, panels with flipbooks) keep
their own rhythm while the sequence moves them, exactly as in a scene. The
file therefore loops on the least common multiple of the sequence's duration
and the loops of its animated layers. When that is longer than the sequence
itself, `render` prints a note:

```text
night.yaml: note: /sequences/night loops every 12 s (its own duration is 4 s) because of its animated layers
```

Sequences are written to `sequences/<id>.png`, as APNG, or as animated WebP
with `--format webp`. A layer with `visible: false` is not drawn, in static
scenes too. Before sequences existed, `visible` on a layer passed validation
but was ignored. A hidden animated layer does not lengthen a scene's loop.
In a sequence it counts only if some `setVisible` can show it.

### Import

```sh
# Add coin.png to art.yaml as palette "coin" and sprite "coin".
pixel-perfect import art.yaml coin.png --id coin
```

`import` appends a palette and a sprite that reproduce the PNG exactly. Every
distinct color gets its own character and fully transparent pixels become `.`.
The worksheet is edited **in place by insertion only**: comments, blank
lines, key order and quoting are kept, and the new entries go at the end of
`palettes` and `sprites`, using the indentation and line endings already in
the file.

| Option | Default | Meaning |
|---|---|---|
| `--id <ID>` | required | Id of the new sprite, and of the new palette. |
| `--palette <ID>` | | Reuse this palette of the worksheet instead of creating one. Every color of the PNG must be in it. |
| `--palette-id <ID>` | the sprite id | Id of the new palette. |
| `--scale <N>` | `1` | The PNG is an N× enlargement. Each N×N block must be one color and becomes one pixel. |
| `--description <TEXT>` | | `description` of the new sprite. |
| `--export` | off | Also append `/sprites/<id>` to `exports`, if the worksheet has that list. |
| `--max-colors <N>` | `256` | Most colors to accept. |
| `--dry-run` | off | Print the resulting worksheet instead of writing it. |

**Lossless only.** The importer never reduces colors. An image with more
colors than `--max-colors` is rejected; the limit protects against importing a
photo or a JPEG by mistake. Every PNG color type is accepted; 16-bit images
only when each sample has an exact 8-bit value. Animated PNGs and other
formats are rejected. Alpha is written as the shortest decimal that converts
back to the same 8-bit value (128 → `0.5`).

**Characters** come from an alphabet holding every character the schema
allows for `char` (1,109,307 of them). Single-column visible characters come
first: `a`–`z`, `A`–`Z`, `0`–`9`, ASCII punctuation, then about 27,000 more,
such as accented letters, Greek, Cyrillic and box drawing. Wide, right-to-left,
private-use and unassigned characters are only used after those run out.
Colors take characters in the order they first appear, reading the image row
by row.

**Nothing is written unless everything checks out.** The original worksheet
must be valid. Before the file is replaced, the result must be valid, must
equal the original plus the new entries, must render the new sprite exactly
as the PNG, and must contain the original text unchanged. The file is then
replaced atomically: a temporary file is written and renamed over it. Ids that
already exist are never overwritten. The worksheet is read once, so changes
saved by an editor while the import runs are lost.

In YAML, the importer understands the layout worksheets use: one block-style
document with top-level keys at column 0. A collection written in flow style
(`sprites: [...]`) or carrying an anchor or a tag is refused rather than
rewritten. A one-line flow list of pointers such as `exports: [/sprites/a]` is
extended in place. JSON may use any layout. TOML is edited with `toml_edit`.

Exit codes: `0` imported, `1` worksheet invalid or import rejected, `2` usage
error or unreadable file.

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

JSON, YAML and TOML all map onto the same data model and are validated against
the same JSON Schema.

| Output | Status |
|---|---|
| PNG | supported |
| APNG (animated PNG) | supported |
| WebP (lossless) | supported, larger than PNG for now |
| Animated WebP (lossless) | supported, larger than APNG for now |

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
| **scene** | A multi-plane composition stacking panels and timelines as layers. A layer can be hidden (`visible: false`) and given an `id`. |
| **sequence** | A scene-level animation: actions (move, show/hide, z-order) applied over time to the scene's layers, for parallax and effect layers such as lights. |
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

A palette `char` is any single character except `.`, `=`, whitespace, control
characters, invisible format characters (such as U+200B ZERO WIDTH SPACE) and
combining marks, so that every row of `lines` shows exactly one visible
character per pixel. Format characters and combining marks were accepted
before the `import` command was added.

## License

Licensed under the [Apache License, Version 2.0](LICENSE).
