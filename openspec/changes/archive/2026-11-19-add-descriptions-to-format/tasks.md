## 1. Schema

- [x] 1.1 Add `description: { "$ref": "#/$defs/text" }` to the worksheet root, palette color, transform, flipbook frame, transition and layer in `schema/worksheet.schema.json`
- [x] 1.2 Add `tests/fixtures/valid/descriptions.yaml`: a small worksheet with `description` on the root, a color, a transform of every `kind`, a frame, a transition of every `kind` and a layer, plus on each of the seven pre-existing sites
- [x] 1.3 Test in `tests/schema.rs`: `descriptions.yaml` passes schema and semantic validation
- [x] 1.4 Test in `tests/schema.rs`: for each of the six new sites, `""` and `42` are rejected with an error pointing to that `description`

## 2. Model

- [x] 2.1 Add `description: Option<String>` to `Worksheet`, `Color`, `Frame` and `Layer` in `src/model.rs`
- [x] 2.2 Test in `tests/model.rs`: loading `descriptions.yaml` fills those four fields with the authored text, and its transforms and transitions deserialize to the same values as without `description`

## 3. Output invariance

- [x] 3.1 Test (in `tests/export.rs`): render `descriptions.yaml` and the same document with every `description` removed, with and without `--safe-build`; image files and `manifest.json` are byte-identical

## 4. Wrap-up

- [x] 4.1 `cargo test` and `cargo clippy --all-targets` pass; existing golden files unchanged
- [x] 4.2 Validate every file in `examples/` still passes `pixel-perfect validate`
