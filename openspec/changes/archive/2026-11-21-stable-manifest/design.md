## Context

The manifest is built in `src/export.rs` from two types:
- `Manifest`: `format`, `scale`, `safe_build`, and
  `files: BTreeMap<String, ManifestEntry>`;
- `ManifestEntry`: `path`, `width`, `height`, and two optional fields, `frames`
  and `loop`.

`manifest()` builds the manifest from the `Plan`. `write()` encodes each output,
fills in the frame count by looking up the entry by pointer, and then serializes
with `serde_json::to_string_pretty` plus a trailing newline.

`schema/manifest.schema.json` (draft 2020-12, `$id`
`urn:pixelPerfect:manifest:v1`) defines the target layout. It has `options`, a
`files` array of entries discriminated by `kind`, and a shared `fractional`
type. The worksheet schema is embedded in the binary for validation. The
manifest schema is not, because the engine writes manifests and never reads
them.

The readers are the people who integrate the sprites and animations into game
engines. They need a layout that does not drift and that can be validated.

## Goals / Non-Goals

**Goals:**
- The engine writes exactly the layout defined in `schema/manifest.schema.json`,
  for `kind` `static` and `animation`.
- Tests guarantee that every manifest the engine writes validates against the
  schema, so the code and the schema cannot diverge silently.
- Output stays byte-identical for identical renders.

**Non-Goals:**
- Single-frame output: `kind` `frameCollection` and `options.singleFrames`. The
  schema reserves them for a later change.
- Writing a `$schema` field into the manifest. The schema's `$id` is a URN,
  not a URL that can be fetched.
- Embedding the manifest schema in the binary or exposing it in the library API.
- Reading or migrating old manifests.

## Decisions

### 1. `ManifestEntry` is a struct with a `kind` enum, not a serde-tagged enum

```rust
pub struct Manifest { pub options: ManifestOptions, pub files: Vec<ManifestEntry> }
pub struct ManifestOptions { pub format: &'static str, pub scale: u32, pub safe_build: bool }
pub struct ManifestEntry {
    pub id: String, pub kind: EntryKind, pub path: String, pub width: u32, pub height: u32,
    #[serde(skip_serializing_if = "Option::is_none")] pub frame_count: Option<usize>,
    #[serde(rename = "loop", skip_serializing_if = "Option::is_none")] pub loop_seconds: Option<Fraction>,
}
pub enum EntryKind { Static, Animation }   // serialized as "static" / "animation"
```

Fields are serialized in declaration order: `id`, `kind`, `path`, `width`,
`height`, `frameCount`, `loop`. Each entry starts with its key and its kind,
which makes a diff easy to read.

*Alternative considered:* `#[serde(tag = "kind")] enum ManifestEntry { Static {..}, Animation {..} }`.
It enforces at the type level that `frameCount` and `loop` are present only for
animations. The cost is that the common fields are repeated in every variant,
or need `#[serde(flatten)]`, which serde handles poorly together with internal
tags. It also forces `kind` to come first, before `id`. With only two kinds, a
constructor that sets `kind` from `loop_seconds` keeps the invariant in one
place, and the schema test catches any violation.

### 2. Entries are sorted by `id`, as plain string order

`manifest()` sorts `files` by `id` with Rust's `String` ordering, which is
byte-wise and therefore code-point order. It is the same order the current
`BTreeMap` gives, so the order of the golden files does not change. `write()`
finds the entry for an output with `binary_search_by` on `id` when it fills in
`frame_count`.

*Alternative considered:* sort `plan.outputs` instead. That also changes the
order in which files are written and would tie the plan to a concern that
belongs to the manifest. It is rejected.

A small helper, `Manifest::entry(&self, id) -> Option<&ManifestEntry>`,
replaces indexing by pointer for library callers and tests.

### 3. The engine writes every option it supports and omits the rest

`options` always contains `format`, `scale` and `safeBuild`, even when they hold
their default values. Integrators then see the values explicitly. The defaults
in the schema exist for compatibility: a manifest from an older engine has no
field for a newer option, and a consumer reads the missing field as its
default. The engine does not write `singleFrames` because the mode does not
exist. This is equivalent to `false`.

*Alternative considered:* omit every field that holds its default value. That
gives a smaller file, but it hides `scale` from integrators, and they need
`scale` to derive pixels-per-unit. It is rejected.

### 4. Schema conformance is enforced by tests, not at runtime

A helper in `tests/common` loads `schema/manifest.schema.json` once, using the
`jsonschema` crate (already a dependency) with draft 2020-12, and asserts that a
manifest is valid. It is called:
- on every golden `manifest.json` (PNG, WebP, `x4-safe`);
- on the manifests written by the example tests;
- on the empty-export case.

Runtime validation would add cost to every render to catch something that only
a code change can break.

### 5. The schema stays `v1`, and its root `$comment` is corrected

This is the first published version of the manifest contract, so the version
does not change. The root `$comment` currently says "keys sorted". That no
longer describes an array, so it becomes "entries sorted by id, fields in a
fixed order".

## Risks / Trade-offs

- [Breaking change for anyone already reading `manifest.json`] → The project is
  at an early stage and the old layout was never published as a contract. The
  README documents the new layout and the compatibility policy, and there is
  no transition period.
- [Regenerating golden files with `UPDATE_GOLDEN` could hide unintended changes
  to images] → After the regeneration, check that `git diff --stat tests/golden`
  touches only the `manifest.json` files.
- [The schema cannot express some guarantees: unique ids, sort order, `loop`
  equal to the sum of durations] → Unit tests in `tests/export.rs` cover sorting
  and the content of entries. Uniqueness already follows from planning.
