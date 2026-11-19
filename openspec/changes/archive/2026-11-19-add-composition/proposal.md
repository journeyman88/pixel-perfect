## Why

Pixel art is rarely a single sprite: backgrounds, characters and props are
combined on a canvas. Panels and scenes describe that combination, but without
precise compositing rules, semi-transparent pixels would render differently
across implementations. This change defines the rules and renders static
compositions.

Compositions also make large images easy to declare (a panel is just a
`width` and a `height`), so this change adds a guard against images that would
exhaust memory.

## What Changes

- Render panels: a transparent canvas of `width` x `height`, with elements drawn
  in `zIndex` order (ties broken by array order). Elements with `visible: false`
  are skipped. Each element's own `transforms` are applied before placement, and
  pixels outside the canvas are clipped. Negative or out-of-range `coordinates`
  are allowed.
- Render scenes: each `_panel` layer is rendered on its own canvas, then the
  layers are stacked on the scene canvas with the same ordering and clipping
  rules.
- Define exact alpha compositing: source-over, non-premultiplied, integer
  arithmetic with round-half-up, specified as a formula so the output is
  bit-exact on every platform.
- Export static panels and scenes as PNG through the existing `render` command.
  A panel with a visible `_flipbook` element and a scene with a `_timeline`
  layer (or an animated panel) are animated: they stay unsupported until
  `add-animation`, with the same rule as flipbooks today (error when listed in
  `exports`, notice when skipped implicitly). There is no static fallback, so a
  resource never silently changes from a still PNG to an APNG.
- Add `--max-image-size <SIZE>` to `render`: every image the engine builds
  (sprite rasters, transform results, canvases, the final upscaled image) is
  limited to SIZE bytes as RGBA8. Default `1GiB`; `none` disables the limit.
  An image over the limit is a plan error and nothing is written.

## Capabilities

### New Capabilities
- `composition`: panel and scene rendering, z-ordering, visibility, clipping
  and the alpha compositing rule.

### Modified Capabilities
- `png-export`: `render` gains `--max-image-size`; exports gain static panels
  and scenes; new image size limit requirement.

## Impact

- New `compose` module (blend, draw with clipping, panel and scene rendering),
  reused by `add-animation` for every frame.
- `export` planning computes the size of every intermediate image before
  rendering.
- No new dependencies.
