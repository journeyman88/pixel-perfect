## Context

Transforms (`flip`, `rotate`, `scale`, `crop`) are applied in array order by
`raster::apply`, and their output size is computed without pixels by
`raster::output_size`. Two load/plan-time walks rely on that size: the `crop`
bounds check in `semantic.rs` (declared chains and chains extended by
`appendTransforms`) and the `--max-image-size` walk in `export.rs`. Repeating a
tile over an area currently needs one element per tile plus `crop` on the edge
tiles.

## Goals / Non-Goals

**Goals:**
- One transform that repeats the image to an exact size, usable in every
  transform chain.
- Same guarantees as the other transforms: deterministic, nearest-neighbor
  (no resampling at all), size known before rendering.

**Non-Goals:**
- A start offset inside the tile (`offset: [ox, oy]`). It can be added later
  without changing the meaning of `tile` without it.
- Mirrored repetition (alternate tiles flipped), spacing between tiles.
- Random variants per cell (as pxl's `tile`/`pattern` with groups, `rflip`,
  `rswap`): those stay separate elements.

## Decisions

### Target size, not a repeat count
`{ kind: tile, width, height }` gives the final size, like `scale`. A count
(`{ x: 3, y: 2 }`) would need a `crop` after it whenever the area is not a
multiple of the tile, which is the common case (pxl's `pattern()` cuts the edge
tiles). With a size, both cases are one transform, and the resulting size is
explicit in the worksheet.

### Definition by modulo
Output pixel `(x, y)` is input pixel `(x mod w, y mod h)`, where `w` x `h` is the
size of the image at that point of the chain. Consequences, all intended:
- a size smaller than the image keeps its top-left `width` x `height` area
  (same as `crop [0, 0, width, height]`);
- a size equal to the image leaves it unchanged;
- transparency is repeated like any other pixel.

`raster::apply` builds the result with `Image::from_fn`; `output_size` returns
`(width, height)`. `width` and `height` use the schema's `size` definition, the
same as `scale`.

### Reuse the existing size walks
`semantic::walk_crops` and the export size walk only need the new size rule:
the crop check gains one match arm, and the size guard already calls
`raster::output_size`, so a `tile` to 30000x30000 is reported at that transform
like a `scale` would be.

### Flipbook elements tile the current frame
A `tile` on a flipbook element repeats the frame shown at that instant: all
copies change together. Copies with different phases remain separate elements
(and need a timeline to be offset, as today).

## Risks / Trade-offs

- [A large `tile` multiplies memory like `scale`] → covered by the existing
  `--max-image-size` walk; tested explicitly.
- [Authors may expect the repetition to be anchored to the panel, not to the
  element] → documented: it starts at the element's top-left corner; an
  `offset` option is the planned answer if needed.

## Migration Plan

Additive: new `kind` value only. Existing worksheets validate and render
byte-identically.

## Open Questions

None.
