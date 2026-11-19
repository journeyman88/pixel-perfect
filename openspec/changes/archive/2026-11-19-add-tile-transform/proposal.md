## Why

Backgrounds, floors, walls and fences are usually one small tile repeated over
an area. Today a worksheet must place one element per tile (and `crop` the
edge tiles), so a 256x96 floor of 16x16 tiles takes 96 elements. The pxl
examples show how common this is (`pattern()` is used in 9 of 13 of them). A
`tile` transform expresses it as one element, keeps worksheets small, and also
enables scrolling backgrounds: a tiled element wider than the panel moved with
`moveTo`.

## What Changes

- New transform `{ kind: tile, width, height }`: the image becomes exactly
  `width` x `height`, where pixel `(x, y)` is pixel `(x mod w, y mod h)` of the
  image as it is at that point of the chain (`w` x `h`). Edge tiles are cut
  automatically; a size smaller than the image keeps its top-left area.
- It works wherever transforms do: sprite frames of flipbooks, panel elements
  (sprite or flipbook) and `appendTransforms`. On a flipbook element every copy
  shows the same frame.
- Load-time `crop` bounds checks and the `--max-image-size` walk use the size
  after `tile`, like after `scale`.
- No offset option for now: the repetition always starts at the image's
  top-left corner.

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
- `sprite-rendering`: new "Tile transform" requirement.
- `worksheet-loading`: "Transforms stay within bounds" also follows `tile`.

## Impact

- `schema/worksheet.schema.json`: new `kind` value and its fields.
- `src/model.rs` (`Transform::Tile`), `src/raster.rs` (`apply`,
  `output_size`), `src/semantic.rs` (crop walk).
- The size guard in `src/export.rs` already goes through
  `raster::output_size`; it needs tests, not new logic.
- README transform list and the schema description of `kind`.
- No new dependencies; existing worksheets are unaffected.
