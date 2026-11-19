## Why

Every later feature (rendering, composition, animation, export) needs a worksheet
that is parsed, validated and resolved into a trustworthy in-memory model. Today
the project has only the JSON Schema and no code, so this change lays the
foundation: a Rust crate that loads a worksheet in any supported text format and
reports every structural and semantic error before anything is rendered.

## What Changes

- Create the Rust crate `pixel-perfect` (library + CLI binary), licensed Apache-2.0.
- Load worksheets from JSON, YAML and TOML, choosing the format by file extension,
  into a single JSON data model.
- Validate the data against `schema/worksheet.schema.json` (embedded in the
  binary) and report schema errors with the path of the offending node.
- Deserialize the validated data into a typed worksheet model.
- Enforce the semantic rules the schema marks as `$comment: "Engine: ..."`
  (references, inheritance cycles, palette characters, line widths, overrides,
  export pointers, crop bounds, element ids).
- Add a `pixel-perfect validate <file>...` command with well-defined exit codes.

## Capabilities

### New Capabilities
- `worksheet-loading`: reading JSON/YAML/TOML worksheets, schema validation,
  typed model, semantic validation and the `validate` CLI command.

### Modified Capabilities
<!-- None: no specs exist yet. -->

## Impact

- New files: `Cargo.toml`, `src/` (library and binary), `tests/` with fixtures.
- The schema file `schema/worksheet.schema.json` becomes a build input
  (`include_str!`); its content is unchanged.
- New dependencies: `serde`, `serde_json`, `toml`, a maintained YAML crate,
  `jsonschema`, `clap`, `thiserror`.
- No rendering or image output yet; see `add-sprite-rendering`.
