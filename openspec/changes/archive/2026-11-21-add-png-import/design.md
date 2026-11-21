## Context

The engine only reads worksheets: `input::parse` turns JSON, YAML or TOML text
into a `serde_json::Value`, which is validated against the schema and then
resolved (`load_str` → `ResolvedWorksheet`). Nothing in the crate writes a
worksheet back. The parsers in use (`serde_json`, `serde_norway`, `toml`) keep
no spans, comments or formatting, so a parse-modify-serialize cycle would
destroy hand-written worksheets, which are heavily commented (see
`examples/sprites.yaml`).

The import has two independent halves:

1. **Image → entries**: decode a PNG, normalize and downscale it, and produce
   a palette (or reuse one) plus a sprite's `lines`. Pure, easy to test.
2. **Entries → worksheet text**: insert those entries into the existing text
   without touching anything else. This is the risky half.

The spec (`specs/png-import/spec.md`) fixes the behavior; this document covers
how to build it safely.

## Goals / Non-Goals

**Goals:**
- Lossless import: the imported sprite renders to exactly the PNG's pixels.
- Insert-only editing: the original file is a byte subsequence of the result.
- Same data model whatever the worksheet format.
- Never leave a broken or partial worksheet on disk.

**Non-Goals:**
- Lossy inputs (JPEG, WebP lossy), color quantization, dithering.
- Animated PNG → flipbook import (possible later, reusing both halves).
- Replacing or updating an existing sprite (`--force`); ids must be new.
- Detecting the upscale factor automatically (see Open Questions).
- A general-purpose, format-preserving worksheet editor.

## Decisions

### 1. Insertion, not re-serialization

The editor computes **insertion points** in the original text and splices
generated snippets into it. The rest of the file is never re-emitted.

*Alternatives:* re-serializing through `serde` (loses comments and style:
rejected); a format-preserving YAML library (none mature in Rust; `yaml-rust2`
and `saphyr` do not round-trip comments); emitting the new entries in a
separate file (worksheets are self-contained, so they could not be
referenced: rejected in discussion).

### 2. One small editor per format

A common trait, roughly
`fn plan(text, &Additions) -> Result<Vec<Insertion>, EditError>`, where an
`Insertion` is `(byte offset, text)`, with three implementations:

- **JSON**: a hand-written scanner that tracks strings, escapes and bracket
  depth, enough to find the top-level object, the value of `palettes`,
  `sprites` and `exports`, and the offset of each array's closing `]`. It
  inserts `,` + newline + element before `]` (or the element alone in an empty
  array), copying the indentation of the previous element; when everything is
  on one line, it inserts on that line. A missing collection is added before
  the root's closing `}`. Elements are written with `serde_json` (pretty,
  re-indented to the detected indent), so escaping is always correct. Any valid
  JSON can be handled this way; there are no unsupported layouts.
- **YAML**: a line-based scanner for the subset worksheets use: a single
  block-style document whose top-level keys start at column 0. A collection's
  block runs from its key to the next column-0 line that is neither blank nor a
  comment. The new item is inserted after the block's last content line, so
  comments and blank lines that precede the next key stay attached to it. The
  item indentation is taken from the block's first `-`. Anything outside the
  subset is an `EditError` naming the construct: flow collection on the key's
  line, `---`/`...` separators, anchors, aliases or tags on the key, tab
  indentation, block scalar (`|`, `>`) on a top-level key. Snippets are
  generated in the style of the examples (`- { char: a, rgba: [...] }` for
  colors, double-quoted `lines`). Scalars are always quoted unless they match a
  conservative plain-safe pattern (letters, digits, `-`, `_`; not a YAML 1.1
  boolean/null word), so a character such as `#`, `:` or `&` cannot change the
  meaning.
- **TOML**: `toml_edit` (pure Rust, from the `toml` maintainers), which parses
  into a document that keeps the original text for every untouched item.
  Appending to `[[palettes]]`/`[[sprites]]` arrays of tables, or to an inline
  array, is done through its API, and the document is printed back. Because
  `toml_edit` re-prints the whole document, the subsequence check
  (Decision 4) also catches any normalization it might do. If it ever changes
  the original bytes, the import fails rather than writing.

*Alternative:* one generic text scanner for all three formats. Rejected: TOML
arrays of tables can be spread through the file, and `toml_edit` already
solves this correctly.

### 3. Character assignment and alpha

- Colors are keyed by canonical 8-bit RGBA (`raster::canonical`, so every
  alpha-0 pixel is the same transparent value) and assigned characters in
  first-appearance order over the downscaled image. This keeps the result
  stable when the PNG is re-exported by another editor, and puts characters in
  reading order, which makes small sprites easy to read.
- The alphabet covers every character the `char` pattern accepts, so the
  importer imposes no limit of its own. It is split into two tiers (see spec).
  Tier 1 is everything that occupies exactly one visible column in a
  monospace editor, and tier 2 is the rest. The tier-1 criteria
  (East Asian Width not W/F, bidi class not R/AL, assigned, not private use,
  not default-ignorable, not a noncharacter) are a property test on the table,
  not a judgement made per character. Ambiguous-width (`A`) characters such as
  Greek, Cyrillic and box drawing stay in tier 1: they are single-column in
  every non-CJK terminal, and excluding them would lose the most useful
  symbols.
- The alphabet is stored as a **committed range table**
  (`src/import/alphabet/table.rs`: ordered `(start, end)` ranges per tier,
  about 1,400 in all; `src/import/alphabet.rs` walks them). A generator in `tools/gen-alphabet/` (a small Rust
  binary outside the main crate) builds it from the Unicode Character Database
  files of a pinned version (Unicode 16.0: `UnicodeData.txt`,
  `EastAsianWidth.txt`, `DerivedBidiClass.txt`, `DerivedCoreProperties.txt`,
  `PropList.txt`). The n-th character is found by walking the ranges, so
  nothing near 1.1 million entries is ever materialized.
  *Alternatives:* `unicode-width` and similar crates at runtime. Rejected
  because their Unicode version follows crate updates, which would silently
  change which characters an import assigns. Committing the table makes a
  Unicode upgrade a visible diff and a spec change.
- A test enumerates all 1,114,112 code points, keeps those matching the
  schema's `char` regex (compiled with the same `regex` engine `jsonschema`
  uses), and checks that this set equals the alphabet. This ties the alphabet
  to the schema: changing the pattern without regenerating the table fails CI.
- Characters a format cannot hold literally (YAML forbids raw noncharacters
  such as U+FFFE; some characters need quoting) are escaped by the editors.
  The data-model check (Decision 4) proves the escaped text loads back to the
  same characters.
- `--max-colors` defaults to 256 and can go up to the alphabet length. It is
  not a format limit but a guard against accidentally importing a photo or a
  lossy image (thousands of colors and an unreadable worksheet). It stays out
  of the way when larger palettes are asked for explicitly.

### 3b. Tightening the `char` pattern

The schema pattern becomes `^[^.=\s\p{Cc}\p{Cf}\p{M}]$`. Format characters
(zero-width spaces, BOM, bidi controls) are invisible, and combining marks
draw on top of the previous character. In both cases a row of `lines` looks
shorter or different from what it is. Rejecting them helps hand-written
worksheets as much as imported ones, and lets tier 2 of the alphabet be "all
remaining valid characters" without containing anything that corrupts the
grid. `\p{Cf}` and `\p{M}` are supported by both `regex` and `fancy-regex`.
A schema test confirms the compiled pattern rejects U+200B and U+0301 and
accepts `é` and `山`.
- Alpha: for an 8-bit `k` in 1–254, try 1, 2 and 3 fraction digits and take
  the smallest `a` with `Rgba::alpha_to_u8(a) == k`. Three digits always
  suffice, since each 8-bit step is ~0.0039 wide. A unit test checks all 254
  values against the engine's own conversion, so the two cannot drift.

### 4. Verify, then write atomically

The pipeline is linear and nothing touches the disk before the last step:

1. read and fully validate the original worksheet (`load_str`);
2. decode, normalize and downscale the PNG; build the additions
   (palette/sprite/export) and check ids and palette coverage;
3. plan the insertions and splice them;
4. checks: the new text passes `load_str`; `input::parse(new)` equals
   `input::parse(old)` with the additions appended (a `serde_json::Value`
   comparison, before resolution); `render::sprite` on the new worksheet
   equals the downscaled image; the old text is a subsequence of the new text
   (a linear scan);
5. `--dry-run` prints the text; otherwise write a temporary file in the same
   directory (`tempfile::NamedTempFile::new_in`, `tempfile` moving from
   dev-dependencies to dependencies), copy the permissions, `persist` it over
   the original (rename).

Check 4b is what makes the hand-written YAML/JSON scanners safe: a wrong
insertion point produces a different data model and is rejected, so a scanner
bug can fail an import but cannot corrupt a worksheet.

### 5. Module layout

- `src/import/mod.rs`: options, the pipeline above, diagnostics.
- `src/import/image.rs`: PNG decoding (`png` crate with `Transformations::EXPAND`,
  plus explicit 16-bit and `acTL` handling), normalization, `--scale`.
- `src/import/entries.rs`: palette generation/reuse, alpha formatting, lines.
- `src/import/alphabet.rs`: the generated range table and `nth(i) -> char`.
- `tools/gen-alphabet/`: the generator (not part of the published crate).
- `src/edit/{json,yaml,toml}.rs`: insertion planning per format.
- `main.rs`: the `Import` subcommand, which reports errors like `validate`.

`edit` is independent of PNGs, so a later command (for example flipbook
import) can reuse it.

## Risks / Trade-offs

- [The YAML scanner misreads an unusual but valid file] → check 4b rejects any
  result whose data model is not exactly "original + additions". The worst
  case is a refused import, never a damaged file. Unsupported constructs fail
  early with a clear message.
- [`toml_edit` normalizes something in the untouched text] → check 4d detects
  it and the import fails. If it happens with real worksheets, fall back to
  appending raw `[[sprites]]` tables at the end of the file, which is valid
  TOML for arrays of tables.
- [Huge PNGs use a lot of memory] → the `png` decoder's `Limits` is set from
  the same default as `--max-image-size` (1 GiB), and the color limit stops
  before any text is generated.
- [Rename across a symlinked worksheet replaces the link with a file] → the
  path is resolved with `canonicalize` before writing, so the target file is
  replaced and the link is kept.
- [Concurrent edits (an editor saving while the import runs)] → not handled
  beyond atomic rename. The import reads the file once, so a save made in
  between is overwritten. This is documented in the README.
- [First-appearance order changes when pixels move] → re-importing an edited
  PNG can reassign characters. This is acceptable because imports never
  update existing entries; `--palette` gives stable characters when needed.

- [Tier 2 characters (CJK, Arabic, unassigned code points) make a worksheet
  hard to read] → they are only reached past several tens of thousands of
  colors, which needs an explicit `--max-colors`. At that point the worksheet
  is no longer meant to be edited by hand anyway.
- [Ambiguous-width characters render double-width in CJK locales] → accepted.
  Readability is degraded but not broken, and only beyond the 92 ASCII
  characters.

## Migration Plan

A new subcommand, two dependencies (`toml_edit`, and `tempfile` moved out of
dev-dependencies) and one breaking schema change: the `char` pattern no longer
accepts format characters or combining marks. No example or fixture uses
them. The README's changelog notes the change. Rollback means removing the
subcommand and restoring the old pattern.

## Open Questions

- `--scale auto`: detect the largest N for which all blocks are uniform. It is
  convenient, but it can guess wrong on images with large flat areas. It is
  left out until there is a real need.
- Should `--palette` also accept a palette that only partly covers the image,
  adding the missing colors to a new derived palette (`_palette: <id>`)? This
  is useful for recolors. It is deferred to keep this change small.
