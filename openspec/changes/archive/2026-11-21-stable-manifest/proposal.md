## Why

`manifest.json` is how tools find what a render wrote. The main audience is
people who package the sprites and animations into a game engine. Today the
format is described only in prose, in the png-export spec. Integrators have
nothing to validate against and no promise about how the format may change.

`schema/manifest.schema.json` now defines a format meant to be a stable
contract. It is a JSON Schema, draft 2020-12, the same draft as the worksheet
schema. The engine does not write that format yet. This change aligns the
engine with the schema and makes the schema the source of truth for the
manifest, as `worksheet.schema.json` already is for the worksheet.

## What Changes

- **BREAKING** The manifest layout changes:
  - `format`, `scale` and `safeBuild` move from the root into an `options`
    object.
  - `files` becomes an array sorted by `id`. It is no longer an object keyed by
    export pointer.
  - Each entry carries its pointer as `id`, and a `kind` field (`static` or
    `animation`) that says which other fields are present.
  - The frame count of an animation is renamed from `frames` to `frameCount`.
- `schema/manifest.schema.json` becomes the normative definition of the
  manifest. Every manifest the engine writes is valid against it, and tests
  enforce this.
- Compatibility policy, stated in the schema and the README:
  - Consumers ignore fields they do not know.
  - An optional field that is missing has its default value.
  - New command-line options appear in `options` and new formats as new
    `format` values. Neither needs a new schema version.
- Out of scope: the schema also defines `kind: frameCollection` and the
  `singleFrames` option, for a future mode that writes one file per frame. This
  change does not add that mode. The engine never writes `frameCollection` and
  does not write `singleFrames`, which therefore keeps its default `false`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `png-export`:
  - The *Manifest* requirement is rewritten for the new layout and references
    `schema/manifest.schema.json`.
  - The WebP scenario of *Render command* checks `options.format`.

## Impact

- `src/export.rs`: the `Manifest` and `ManifestEntry` types and how they are
  serialized, the `manifest()` and `write()` functions, and the way frame counts
  are filled in.
- Library API: `Manifest.files` changes from a map to a vector, so callers that
  index by pointer (for example `tests/examples.rs` and `tests/export.rs`) need
  a lookup helper.
- Tests:
  - The golden manifests under `tests/golden/*/manifest.json` are regenerated
    with `UPDATE_GOLDEN`.
  - `tests/golden.rs` reads the manifest in the new layout.
  - A new test validates every manifest the engine writes against the schema.
    It uses the `jsonschema` crate, already a dependency.
- `README.md`: the manifest section and its example.
- No new dependencies.
