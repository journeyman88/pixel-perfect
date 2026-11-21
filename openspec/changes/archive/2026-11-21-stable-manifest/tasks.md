## 1. Schema test harness

- [x] 1.1 Add `assert_valid_manifest(&Value)` to `tests/common/mod.rs`. It loads `schema/manifest.schema.json` once, compiles it with `jsonschema::draft202012`, and fails the test listing every violation it finds.
- [x] 1.2 Add a test that compiles the schema against the 2020-12 meta-schema, so a broken schema file fails on its own and not inside an unrelated test.
- [x] 1.3 Add fixture `tests/fixtures/valid/palettes-only.json`, a worksheet that defines only palettes. Use it to test the "Nothing to export" case in section 3.

## 2. Manifest types and serialization (`src/export.rs`)

- [x] 2.1 Replace `Manifest` and `ManifestEntry` with the following types, as in design decision 1:
  - `Manifest { options, files: Vec<ManifestEntry> }`
  - `ManifestOptions { format, scale, safe_build }`
  - `ManifestEntry { id, kind, path, width, height, frame_count, loop_seconds }`
  - `EntryKind { Static, Animation }`, serialized as camelCase.
- [x] 2.2 Set `kind` in a single constructor: `Animation` when `loop_seconds` is set, `Static` otherwise.
- [x] 2.3 Make `manifest()` sort `files` by `id`. Make `write()` fill in `frame_count` by looking the entry up with `binary_search_by` on `id`.
- [x] 2.4 Add `Manifest::entry(&self, id: &str) -> Option<&ManifestEntry>`.
- [x] 2.5 Update the root `$comment` in `schema/manifest.schema.json` to "entries sorted by id, fields in a fixed order" (design decision 5).

## 3. Tests for the new layout

- [x] 3.1 Update `animated_entry_in_the_manifest` in `tests/export.rs` to expect the full entry of the "Animated entry" scenario: `id`, `kind` `"animation"`, `frameCount` 4, `loop` 1/2. Validate the whole manifest with `assert_valid_manifest`.
- [x] 3.2 Add a "Manifest content" test: sprite `hero` 16x16 rendered at `--scale 2` gives exactly the `options` and `files` of the spec, and the manifest is valid against the schema.
- [x] 3.3 Add a safe-build test: the `hero` entry has `path` `sprites/8737b11b474b218b.png` and `options.safeBuild` is true.
- [x] 3.4 Add an "Entries sorted by id" test: `exports` lists `hero`, `coin`, `spin` in that order, and the ids in `files` come out as `/flipbooks/spin`, `/sprites/coin`, `/sprites/hero`.
- [x] 3.5 Add a "Nothing to export" test with `palettes-only.json`: the render succeeds, `files` is `[]`, and the manifest is valid against the schema.
- [x] 3.6 In `tests/export.rs` and `tests/examples.rs`, replace `manifest.files[pointer]` with `manifest.entry(pointer).unwrap()`, and `.frames` with `.frame_count`.
- [x] 3.7 Update the WebP assertion in `tests/cli.rs` (`"format": "webp"`) so it reads `options.format` from the parsed JSON.

## 4. Golden files

- [x] 4.1 Regenerate the golden files with `UPDATE_GOLDEN=1 cargo test --test golden`. Then check that `git diff --stat tests/golden` touches only `manifest.json` files.
- [x] 4.2 Update `golden_apngs_decode_to_the_rendered_frames` in `tests/golden.rs`. It should read `options.scale`, iterate the `files` array, take the pointer from `id`, and select animations by `kind == "animation"`.
- [x] 4.3 Add a test that validates every `tests/golden/*/manifest.json` with `assert_valid_manifest`.
- [x] 4.4 Validate with `assert_valid_manifest` the manifests produced by the example tests in `tests/examples.rs`, so the PNG, WebP and sequence examples are all covered.

## 5. Documentation

- [x] 5.1 Rewrite the manifest section of `README.md`:
  - replace the example with the new layout;
  - link `schema/manifest.schema.json`;
  - state the compatibility policy: ignore unknown fields, treat a missing optional field as its default, `options` and `format` can grow without a new version;
  - mention that `scale` is needed to derive source sizes.
- [x] 5.2 Run `cargo test` and `cargo clippy --all-targets`, and render `examples/` once by hand to check the manifest by eye.
