## Why

Pixel art is rarely a single sprite: backgrounds, characters and props are
combined on a canvas. Panels and scenes describe that combination, but without
precise compositing rules, semi-transparent pixels would render differently
across implementations. This change defines the rules and renders static
compositions.

## What Changes

- Render panels: a transparent canvas of `width` x `height`, with elements drawn
  in `zIndex` order (ties broken by array order). Elements with `visible: false`
  are skipped. Each element's own `transforms` are applied before placement, and
  pixels outside the canvas are clipped. Negative or out-of-range `coordinates`
  are allowed.
- Render scenes: layers (panels) stacked in the same way on the scene canvas.
- Define exact alpha compositing: source-over, non-premultiplied, 8-bit integer
  arithmetic with a fixed rounding rule, specified as a formula so the output is
  bit-exact.
- Static fallback for animated content inside a static render: a flipbook
  element shows its first frame; a timeline layer shows its panel's initial state.
- Export panels and scenes as PNG through the existing `render` command.

## Capabilities

### New Capabilities
- `composition`: panel and scene rendering, z-ordering, clipping and the alpha
  compositing rule.

### Modified Capabilities
- `png-export`: exports gain panels and scenes, not just sprites.

## Impact

- New compositing module, reused by `add-animation` for every frame.
- No new dependencies.
