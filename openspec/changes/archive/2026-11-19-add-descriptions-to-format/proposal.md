## Why

Worksheets can already carry a free-form `description` on palettes, sprites,
flipbooks, elements, panels, timelines and scenes, but not on the worksheet
itself or on the smaller objects authors most often want to annotate: a single
palette color, a transform, a flipbook frame, a timeline transition, a scene
layer. Today
those notes can live only in YAML/TOML comments, which JSON cannot hold and
which are lost when a worksheet is converted between formats.

## What Changes

- The schema accepts an optional `description` (non-empty string, `$defs/text`)
  in six more places: the worksheet root, each palette color (`colors[]`),
  each transform (`transforms[]`, every `kind`), each flipbook frame
  (`frames[]`), each timeline transition (`transitions[]`, every `kind`) and
  each scene layer (`layers[]`). The schema edit is already in the working
  tree.
- Descriptions remain annotations: they never change validation outcome
  beyond their own type check, never affect rendered pixels, file names,
  `--safe-build` hashes or `manifest.json`.
- The typed model mirrors the new fields where it is a struct (worksheet,
  color, frame, layer), so the model keeps following the schema's field names.
- Tests and a fixture cover acceptance, rejection of invalid values, and
  byte-identical output with and without descriptions.

No breaking changes: every worksheet valid before stays valid.

## Capabilities

### New Capabilities

_None._

### Modified Capabilities

- `worksheet-loading`: adds a requirement defining where `description` is
  allowed, what values it accepts, and that it has no effect on rendering or
  semantic validation.

## Impact

- `schema/worksheet.schema.json`: six new `description` properties (done).
- `src/model.rs`: optional `description` on `Worksheet`, `Color`, `Frame`,
  `Layer`.
- `tests/schema.rs`, `tests/fixtures/valid/`: new tests and fixture.
- No new dependencies, no CLI changes, no change to existing golden outputs.
