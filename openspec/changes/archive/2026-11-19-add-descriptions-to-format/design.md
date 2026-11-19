## Context

`schema/worksheet.schema.json` defines `$defs/text` ("Free-form human-readable
description. Ignored by the renderer.", non-empty string) and already uses it
as `description` on palettes, sprites, flipbooks, elements, panels, timelines
and scenes. The working tree adds it to the root, colors, transforms, frames,
transitions and layers. Every one of these objects is closed with
`unevaluatedProperties: false`, so before the edit a `description` there was a
schema error.

`src/model.rs` mirrors the schema's field names. It already has
`description: Option<String>` on the seven existing structs; nothing reads
them. Serde does not deny unknown fields, so a schema-valid `description` that
the model lacks is silently dropped — rendering already works with the schema
edit alone (checked by validating and rendering `examples/campfire.yaml` with
descriptions added in the five places other than transforms; transforms use
the same mechanism).

## Goals / Non-Goals

**Goals:**
- Allow `description` in the six new places with the same rules as elsewhere.
- Specify that descriptions never affect output, and test it.
- Keep the model aligned with the schema.

**Non-Goals:**
- Descriptions on `exports` entries (plain strings, cannot carry properties).
- Using descriptions anywhere in output (manifest, PNG text chunks, CLI).
- Changing the existing seven `description` properties.

## Decisions

**Reuse `$defs/text` by `$ref`.** Same type, same `minLength: 1`, same editor
tooltip everywhere. Alternative: inline `{"type": "string"}` per site — rejected,
it would drift.

**Transforms and transitions: one property at the object level, not per
`kind` branch.** The property sits next to `kind` (and `_element` for
transitions), which `unevaluatedProperties` sees for every `if/then` branch, so
one declaration covers all five transform kinds and all six transition kinds.

**Model: add `description: Option<String>` to `Worksheet`, `Color`, `Frame`,
`Layer`; leave `Transform` and `Transition` alone.** The struct fields follow
the existing pattern at no cost. `Transform` and `Transition` are internally
tagged enums (five and six variants); adding the field to each variant adds
noise for a value nobody reads, and serde already ignores it. Alternative:
restructure each as a struct with a flattened `kind` enum — rejected as a
refactor out of proportion to the change.

**Output invariance is tested, not assumed.** A test renders a fixture with
descriptions and the same document with every `description` stripped, and
compares files and manifest byte for byte. This guards against a future
change (e.g. hashing the raw worksheet for `--safe-build`) leaking
annotations into output.

## Risks / Trade-offs

- [Model lags schema for `Transform` and `Transition`] → documented here; a
  schema-only field being dropped by serde is harmless because the field is
  never read. A test asserts that a transform with `description` still
  deserializes, so a future `deny_unknown_fields` cannot break it silently.
- [Future code reads descriptions from the model and misses transforms or
  transitions] → revisit their representation then.
