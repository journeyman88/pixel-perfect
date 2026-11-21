## 1. Setup

- [x] 1.1 Add `toml_edit` to `[dependencies]` and move `tempfile` from `[dev-dependencies]` to `[dependencies]` in `Cargo.toml`; check the crate still builds without a C toolchain
- [x] 1.2 Create the empty modules `src/import/{mod,image,entries}.rs` and `src/edit/{mod,json,yaml,toml}.rs`, exported from `lib.rs`, with license headers
- [x] 1.3 Change the `char` pattern in `schema/worksheet.schema.json` to `^[^.=\s\p{Cc}\p{Cf}\p{M}]$` and update its `description`
- [x] 1.4 Schema tests (fixtures under `tests/fixtures/invalid/`): U+200B and U+0301 as `char` fail pointing to the entry's `char`; `é` and `山` pass; all existing examples and fixtures still validate
- [x] 1.5 Add a PNG-writing test helper to `tests/common/` (color type, bit depth, optional `tRNS`/`acTL`) so fixtures can be generated in tests instead of being committed as binaries where possible

## 2. PNG decoding and normalization

- [x] 2.1 Implement `import::image::decode(bytes) -> Image`: `png` decoder with `EXPAND` and decoder `Limits` from the 1 GiB default; reject a non-PNG input and an `acTL` chunk; convert 16-bit samples only when every sample is `v × 257`, else error with the first pixel; canonicalize alpha-0 pixels
- [x] 2.2 Tests: grayscale, gray+alpha, RGB, RGBA, indexed 1/2/4/8-bit with `tRNS`; exact and lossy 16-bit (error names the pixel); two transparent pixels with different RGB become equal; animated PNG rejected; JPEG bytes rejected
- [x] 2.3 Implement `import::image::downscale(&Image, n)`: size must be a multiple of `n`, each aligned block uniform, else error with the size or the block's top-left pixel
- [x] 2.4 Tests: 64×64 of 4×4 blocks → 16×16; `n = 1` is the identity; 30×32 with `n = 4` fails naming 30; a non-uniform block at (8, 12) fails naming it; a block mixing alpha-0 pixels with different RGB counts as uniform

## 3. Palette and lines

- [x] 3.1 Write `tools/gen-alphabet/`: read the pinned Unicode 16.0 UCD files, classify every schema-valid code point into tier 1 / tier 2, emit `src/import/alphabet.rs` (ordered ranges per tier, the explicit ASCII prefix first, Unicode version in a header comment); document how to run it in its README
- [x] 3.2 Generate and commit `src/import/alphabet.rs`; implement `alphabet::len()` and `alphabet::nth(i)` by walking the ranges
- [x] 3.3 Alphabet tests: the first 92 characters are the spec's ASCII order; the set of all characters equals every scalar value matching the schema's `char` regex; no duplicates; tier-1 characters are never wide, right-to-left, default-ignorable, private use or unassigned (spot checks: `山`, `א`, U+E000 and U+0378 are in tier 2, `é`, `Ж` and `█` in tier 1); `nth` agrees with a full enumeration
- [x] 3.4 Implement `format_alpha(u8) -> Option<String>` (none for 255; fewest digits, then smallest) and a test over all values 1–254 against `Rgba::alpha_to_u8`, with explicit cases 128 → `0.5` and 153 → `0.6`
- [x] 3.5 Implement new-palette generation: first-appearance order, `.` for transparent, `--max-colors` limit (error gives the count and says there is no quantization), error for a fully transparent image
- [x] 3.6 Tests: the `aab` / `cb.` example; 288 colors with `--max-colors 300` uses the 92 ASCII characters then tier-1 characters; 300 colors with the default limit fails; 50 000 colors with `--max-colors 50000` round-trips (reaching tier 2); a fully transparent image fails
- [x] 3.7 Implement `--palette` reuse on the resolved palette: 8-bit color match, first matching character in resolved order, error listing up to 10 missing colors with their first pixel plus the total
- [x] 3.8 Tests on `tests/fixtures/valid/` worksheets: reuse `slime`; reuse `ice`, getting an inherited character; missing `(255, 0, 255)` at (2, 7); unknown palette id; duplicate colors in a palette pick the first character
- [x] 3.9 Implement the additions builder: sprite (`id`, optional `description`, `_palette`, `lines`), optional palette (`id`, `colors`), optional export pointer; check id validity and existing ids (palette conflict message suggests `--palette` / `--palette-id`)
- [x] 3.10 Tests: existing sprite id; existing palette id with the suggestion; invalid id; `--export` with and without an `exports` list

## 4. Format-preserving editors

- [x] 4.1 Define `edit::Additions`, `Insertion { offset, text }`, `EditError`, the line-ending detection (LF/CRLF) and `apply(text, insertions)`
- [x] 4.2 Implement the JSON editor: string/escape/depth scanner, top-level keys, append before `]` with the previous element's indentation (single-line and empty arrays included), missing collection before the root `}`, elements emitted by `serde_json`
- [x] 4.3 JSON tests (fixtures under `tests/fixtures/import/json/`): pretty file, 4-space and tab indentation, minified file, empty array, missing `sprites`, keys containing escaped quotes, CRLF
- [x] 4.4 Implement the YAML editor: top-level block scan, insertion after the block's last content line, item indentation from the first `-`, conservative quoting of scalars (always for `lines`), errors for flow collections, multiple documents, anchors/aliases/tags on the target key, tabs, block scalars on top-level keys
- [x] 4.5 YAML tests (fixtures under `tests/fixtures/import/yaml/`): a copy of `examples/sprites.yaml` (comments and blank lines kept, original is a subsequence); comment before the next key stays attached to it; indented and non-indented sequences; missing collection; generated chars `#`, `:`, `&`, `-`, `?`, `y`, `n`, `"`, `\` and U+FFFE (escaped) load back unchanged; every rejected construct gives its error and leaves the text unchanged; file without a final newline; CRLF
- [x] 4.6 Implement the TOML editor on `toml_edit`: append to arrays of tables and to inline arrays; add a missing collection
- [x] 4.7 TOML tests (fixtures under `tests/fixtures/import/toml/`): `readme-example.toml`; arrays of tables split by other tables; inline `exports` array; comments kept; original is a subsequence of the result
- [x] 4.8 Cross-format test: the same PNG imported into `readme-example.{json,yaml,toml}` gives three results that load to the same data model

## 5. Pipeline, verification and writing

- [x] 5.1 Implement `import::run(options)`: validate the original (report like `validate`); build the additions; plan and apply insertions; run the four checks (full validation, data model equals original plus additions, rendered sprite equals the image, original is a subsequence); each failure names its check
- [x] 5.2 Tests: invalid original worksheet is reported with its JSON Pointer; a deliberately wrong insertion (test-only editor injected through the trait) is caught by the data-model check; a forged rendering mismatch is caught by the pixel check; nothing is written in any failure case
- [x] 5.3 Implement writing: `canonicalize` the path, `NamedTempFile::new_in` the same directory, copy permissions, `persist`; `--dry-run` prints the full text to stdout and the summary to stderr
- [x] 5.4 Tests (with `tempfile`): the file is replaced and keeps its permissions; a symlinked worksheet keeps its link; a failed import leaves content and mtime unchanged and no temporary file; `--dry-run` writes nothing and its stdout equals what a real run writes

## 6. CLI

- [x] 6.1 Add the `Import` subcommand to `main.rs` (`<worksheet> <png> --id`, `--palette` conflicting with `--palette-id`, `--scale 1..=64`, `--description`, `--export`, `--max-colors` from 1 to `alphabet::len()`, `--dry-run`), with the summary line and exit codes 0/1/2
- [x] 6.2 CLI tests in `tests/cli.rs`: basic import exits 0 with the exact summary line; rejected import exits 1; `--palette` with `--palette-id`, `--max-colors 0`, `--max-colors 2000000` and an `.xml` worksheet exit 2; a missing PNG exits 2
- [x] 6.3 End-to-end test: import a PNG into a copy of `examples/sprites.yaml`, then `render` at `--scale 1` and compare the output pixels with the PNG; repeat with `--scale 4` on an enlarged PNG

## 7. Docs

- [x] 7.1 README: an `Import` section with usage, options, the lossless/no-quantization rule, the two-tier alphabet, the supported YAML subset, and the note about concurrent edits; note the breaking `char` pattern change
- [x] 7.2 Add an import example to `examples/README.md`, using a small PNG committed under `examples/` and generated by rendering an existing sprite, so the round trip is self-evident
