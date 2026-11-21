## Why

Today every sprite has to be typed by hand, one character per pixel. Pixel art
that already exists as a PNG (drawn in an editor, or exported from an older
project) cannot be brought into a worksheet without transcribing it, which is
slow and error-prone for anything larger than a few dozen pixels.

A worksheet is self-contained (ids resolve only inside it), so the useful
operation is not "convert a PNG into a new file" but "add this PNG to my
worksheet": a palette and a sprite appended to an existing worksheet, ready to
be referenced by flipbooks, panels and scenes.

## What Changes

- New subcommand:
  `pixel-perfect import <worksheet> <png> --id <id> [options]`.
  It decodes the PNG and appends to the worksheet a palette with one entry per
  distinct color and a sprite whose `lines` reproduce the image exactly.
- Lossless only. Every distinct RGBA value gets its own character; fully
  transparent pixels become `.`. There is no implicit quantization: an image
  with more colors than allowed (`--max-colors`, default 256) is rejected.
- Characters come from an alphabet covering every character the schema allows
  for `char`, so the only hard limit is the format's. Single-column, visible
  characters are used first (ASCII letters and digits leading). Wide,
  right-to-left, unassigned and private-use characters come only after all of
  those are taken.
- **BREAKING** (schema): the `char` pattern becomes
  `^[^.=\s\p{Cc}\p{Cf}\p{M}]$`, which also rejects invisible format characters
  and combining marks. They make `lines` misleading whether written by hand or
  generated. No example or fixture uses them.
- Options:
  - `--palette <id>`: reuse an existing palette of the worksheet instead of
    creating one; every color of the PNG must already be in it.
  - `--palette-id <id>`: id of the new palette (default: the sprite id).
  - `--scale <N>`: the PNG is an N× enlargement; each N×N block must be uniform
    and becomes one pixel.
  - `--description <text>`: description of the new sprite.
  - `--export`: also append `/sprites/<id>` to `exports` when the worksheet has
    an `exports` list (without one, every resource is already exported).
  - `--max-colors <N>`: the color limit above.
  - `--dry-run`: print the resulting worksheet to standard output instead of
    writing it.
- The worksheet is edited in place **without losing comments or formatting**:
  only the new entries are inserted; the rest of the file is byte-for-byte
  unchanged. Supported for JSON, YAML and TOML; layouts the editor cannot edit
  safely are rejected rather than reformatted.
- Before writing, the result is reloaded and fully validated, checked to differ
  from the original only by the new entries, and the new sprite is rendered
  and compared pixel by pixel with the PNG. The file is replaced atomically and
  only if every check passes.
- Existing ids are never overwritten: a palette or sprite id that already
  exists is an error.

## Capabilities

### New Capabilities
- `png-import`: the `import` subcommand: PNG decoding and normalization,
  palette and sprite generation, character assignment, alpha conversion,
  format-preserving insertion into JSON/YAML/TOML worksheets, verification and
  atomic write.

### Modified Capabilities
- `worksheet-loading`: palette `char` must not be a format character (`Cf`)
  or a combining mark (`M`).

## Impact

- CLI: new `import` subcommand in `main.rs`; exit codes follow `validate`
  (`0` success, `1` import rejected, `2` usage error or unreadable file).
- Library: new modules for import (PNG → palette and lines) and for
  format-preserving worksheet editing (one per format).
- New dependency: `toml_edit` (pure Rust) for TOML. JSON and YAML use a small
  in-crate scanner that locates the insertion point; no new parser.
- PNG decoding reuses the existing `png` dependency.
- The import alphabet is a range table generated from a pinned Unicode
  version and committed to the repo, so it adds no runtime dependency on
  Unicode tables.
- Schema: `schema/worksheet.schema.json` changes the `char` pattern.
- Out of scope: lossy inputs (JPEG), color quantization, animated PNG import
  (a future flipbook import), replacing existing sprites.
