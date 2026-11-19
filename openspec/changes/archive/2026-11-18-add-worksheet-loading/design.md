## Context

The repository holds only `schema/worksheet.schema.json` (JSON Schema draft
2020-12), which defines the worksheet format. The schema uses `$defs`, `$ref`,
`if`/`then`/`else`, `oneOf`, `prefixItems`, `unevaluatedProperties` and
`unevaluatedItems`, plus a Unicode property escape (`\p{Cc}`) in the palette
`char` pattern. Rules it cannot express are marked `$comment: "Engine: ..."`.

This is the first code in the project, so the decisions below also set the crate
layout for later changes.

## Goals / Non-Goals

**Goals:**
- One loading pipeline for JSON, YAML and TOML with identical results.
- Every schema error and every semantic error reported in a single run, with a
  precise location.
- A typed, fully resolved model that later rendering code can trust without
  re-checking.

**Non-Goals:**
- Rendering or writing images (`add-sprite-rendering` and later).
- XML input.
- Line/column positions for schema and semantic errors (JSON Pointers only;
  line/column is reported for syntax errors).
- Validating chains extended by timeline transitions (`add-animation`).

## Decisions

### Crate layout: one crate, library + binary
`src/lib.rs` exposes the engine; `src/main.rs` is a thin `clap` CLI. A single
crate keeps the build simple. It can be split into a workspace (for example a
`core` crate for WASM) when there is a reason to.

Modules: `input` (format detection and parsing), `schema` (embedded schema and
validation), `model` (serde types), `semantic` (Engine rules), `resolve`
(palette/sprite inheritance), `error` (diagnostics).

### Pipeline: parse → JSON value → schema → typed model → semantic checks
All formats are parsed into `serde_json::Value`, then validated, then
deserialized into the model. Validating the generic value first means the schema
alone decides structural validity, and the typed deserialization cannot fail on
valid input. Semantic checks run only if schema validation passes. They collect
all errors instead of stopping at the first.

### Embedded schema
`include_str!("../schema/worksheet.schema.json")`, compiled once into a
validator. The binary needs no files besides the worksheet.

### Dependencies
- `serde`, `serde_json`: data model.
- `toml`: TOML parsing. Date-time values have no JSON equivalent and are rejected.
- YAML: `serde_yaml` is archived; `serde_norway` (a maintained fork) is used.
  YAML is parsed into its own `Value` and converted to JSON by hand, which keeps
  key conversion and tag rejection under our control whichever fork is used.
- `jsonschema` (Rust crate): supports draft 2020-12 including `unevaluated*`.
- `clap` (derive): CLI.
- `thiserror`: error types.

All dependencies are pure Rust.

### Integer keys in YAML
`overrideLines` keys are strings in JSON and TOML. YAML authors naturally write
`3: "..rr.."`, which produces an integer key. The YAML → JSON conversion turns
integer keys into their decimal string. Other non-string keys are an error.

### Characters and widths
"One character per pixel" means one Unicode scalar value (`char`), not one
grapheme cluster or byte. Widths are counted with `chars().count()`.

### Error reporting
A diagnostic carries the file, a location (JSON Pointer, or line/column for
syntax errors) and a message. Schema errors use the validator's instance path.
Semantic errors build pointers from array indices (e.g.
`/sprites/3/overrideLines/9`). Cycles are reported as `a -> b -> a`.

### Resolution order
Palettes are resolved first (topological order, with cycle detection), then
sprites (topological order over `_sprite`, applying `_palette` replacement and
`overrideLines` with `=`). Character checks run on the resolved sprites, so that
a palette swap that drops a character is caught on the derived sprite.

## Risks / Trade-offs

- **`\p{Cc}` support.** Regex dialects vary: Python's `re` rejects it, for
  example. The Rust `jsonschema` crate must accept it. A test pins this
  behavior. If it fails, the fallback is an explicit range in the schema
  (`\x00-\x1f\x7f-\x9f`), which matches the same set.
- **`unevaluatedProperties` with `$ref` + `if/then`.** The transform, transition
  and element definitions rely on it. Fixtures exercise each `kind`, so a
  validator bug shows up as a failing test, not as silently accepted input.
- **YAML 1.1 quirks** (`yes`/`no` as booleans, `.` sequences). Rows that start
  with special characters may need quoting in YAML. This is documented in a
  fixture, not worked around.
- **TOML has no `null`.** The schema never needs one, so this is not a problem.
