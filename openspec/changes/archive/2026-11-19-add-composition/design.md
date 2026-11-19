## Context

After `add-sprite-rendering` the engine rasterizes sprites, applies transforms
(`raster::apply_all`), encodes any final RGBA image as indexed or RGBA PNG
(`encode::png`), and plans every output before writing (`export::plan`). Only
`sprites` are exportable; the rest is rejected or skipped with a notice.

Panels and scenes are already loaded and checked: references resolve, element
ids are unique, `crop` areas fit along declared transform chains. Nothing is
composed yet.

## Goals / Non-Goals

**Goals:**
- Bit-exact compositing defined by a formula, identical on every platform.
- Static panels and scenes exported as PNG with the existing pipeline (color
  count, palette order, manifest, safe build).
- A memory guard that is checked before anything is rendered and can be raised
  or disabled.

**Non-Goals:**
- Flipbooks, timelines and animated panels or scenes (`add-animation`).
- Other blend modes, layer opacity, or per-layer transforms (not in the schema).
- A static "poster frame" of animated content. It may come later as an explicit
  option.

## Decisions

### Integer source-over
`compose::blend(src, dst) -> Rgba` implements source-over on non-premultiplied
8-bit values without floats:

```
A  = sa·255 + da·(255 − sa)          // result alpha × 255, exact
oa = round(A, 255)
oc = round(sc·sa·255 + dc·da·(255 − sa), A)
round(n, d) = (2n + d) div (2d)       // half up, as for authored alpha
```

The color uses the exact `A`, not the rounded `oa`, so the only rounding is the
final one. Maximum intermediate value is about 3.4·10⁷, so `u32` is enough.
`sa = 255`, `sa = 0` and `da = 0` give the source, the destination and the
source respectively by construction; the code may shortcut them, and a test
checks that the shortcuts agree with the full formula on every input.
`A = 0` only when both alphas are 0, giving `(0, 0, 0, 0)` and keeping
canonical transparency.

Alternatives considered: `f32`/`f64` with a final rounding is deterministic
under IEEE 754 but easy to break with a refactor or compiler flags (fused
multiply-add); premultiplied alpha loses color precision for low alpha, which
matters in pixel art where a 10% shadow keeps its hue.

### Draw with clipping
`compose::draw(canvas: &mut Image, image: &Image, x: i64, y: i64)` intersects
the image rectangle with the canvas in `i64` and blends only the overlap. An
image entirely outside the canvas is a no-op, not an error.

### Groups: render each panel, then composite
A scene layer is rendered as a whole panel on its own transparent canvas and
then drawn on the scene. Source-over is associative in exact arithmetic but not
after rounding, so "flatten then composite" and "draw every element directly on
the scene" can differ by one unit. The group rule is chosen because it matches
the user model (a layer behaves like a single image, as layer groups in image
editors), keeps a panel looking the same whether exported alone or as a layer,
and lets `add-animation` reuse a rendered panel across frames.

### Ordering
Elements and layers are sorted by `zIndex` with a stable sort, so ties keep
array order. Hidden elements are filtered before drawing.

### Static or animated
`compose::is_animated_panel` is true if a visible element has `_flipbook`.
`is_animated_scene` is true if a layer has `_timeline` or uses an animated
panel. A hidden flipbook draws nothing in a static render, so it does not make
its panel animated (it may matter in a timeline, which is animated anyway).
Animated resources go through the same unsupported path as flipbooks today:
error when listed in `exports`, notice when implicit. The notice and the error
name the reason (`animated panels are not supported yet`).

### Image size limit
`--max-image-size <SIZE>`: bytes of an RGBA8 image (`w × h × 4`), default
`1GiB`, `none` to disable.

- **Unit:** bytes rather than pixels, because the concern is memory and users
  reason in MiB/GiB. Only binary units (`KiB`, `MiB`, `GiB`, `TiB`, any case)
  and plain integers are accepted; `KB`/`MB`/`GB` are rejected to avoid the
  1000/1024 ambiguity, with an error that suggests the binary unit.
- **Scope:** every image the engine allocates, not only the output. Planning
  walks each selected export and computes sizes without rendering: sprite
  raster, each step of every element's transform chain (the same walk as the
  `crop` check), panel canvases, scene canvas, final image × `scale²`. This
  catches a `scale` transform to 30000 × 30000 inside a small panel.
- **Not a memory budget:** the peak use is higher (canvas, upscaled copy,
  encoder buffers, roughly 2–3× the largest image). The limit is a guard
  against mistakes, not an accounting of memory. The README says so.
- **Errors:** plan errors (exit 1), one per offending resource or element,
  pointing to it (e.g. `/panels/0`, `/panels/0/elements/2/transforms/0`), with
  the size in human units and the hint `--max-image-size`.
- **Applies to sprites too**, so the rule is the same for every export.
- The PNG side limit (2³¹ − 1) stays and is checked even with `none`.

`clap` parses SIZE with a custom value parser returning `Option<u64>`; an
invalid value is a usage error (exit 2).

### Module layout
- `compose.rs`: `blend`, `draw`, `panel(ws, id) -> Image`,
  `scene(ws, id) -> Image`, `is_animated_panel`, `is_animated_scene`.
- `render.rs` gains `render::resource(ws, pointer) -> Image` dispatching to
  sprite, panel or scene.
- `export.rs`: `SUPPORTED` becomes a per-resource check (collection plus
  static/animated), and the plan gains the size walk.
- `RenderOptions` gains `max_image_size: Option<u64>`.

## Risks / Trade-offs

- **Group rounding differs from direct drawing** by at most one unit per
  channel in rare semi-transparent cases. Documented in the spec scenario;
  golden files fix the behavior.
- **Per-pixel blending cost** is `O(canvas area × elements)`. Fine for pixel
  art sizes; an opaque fast path covers the common case.
- **The default limit may block legitimate large renders.** It is one flag
  away, and the error says which flag.
- **Golden files change:** `full.json` now exports the static panel `stage`
  too. The animated scene `night` is skipped with a notice.
