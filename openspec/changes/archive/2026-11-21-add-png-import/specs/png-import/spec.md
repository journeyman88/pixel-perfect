## ADDED Requirements

### Requirement: Import command
The CLI SHALL provide `pixel-perfect import <worksheet> <png> --id <id>`, which
appends to `<worksheet>` one palette and one sprite reproducing `<png>`, and
accepts the options `--palette <id>`, `--palette-id <id>`, `--scale <N>`,
`--description <text>`, `--export`, `--max-colors <N>` and `--dry-run`. The new
sprite SHALL have `id` set to `--id`, `_palette` set to the palette used,
`lines` reproducing the image and, when `--description` is given,
`description`; it SHALL NOT have `width` or `height`. The new palette SHALL have
`id` and `colors` only. On success the command SHALL print one line to standard
output naming the worksheet, the pointers added, the sprite size and the number
of colors, and SHALL exit with code 0. Every problem SHALL be printed to
standard error as `<file>: <message>`, where `<file>` is the worksheet or the
PNG the problem is about. Exit codes SHALL be `1` when the import is rejected
(invalid worksheet, unsupported image, conflict, failed check) and `2` for
usage errors, unreadable files and unsupported worksheet extensions.
`--palette` and `--palette-id` SHALL be mutually exclusive.

#### Scenario: Basic import
- **WHEN** `pixel-perfect import art.yaml coin.png --id coin` runs on a valid worksheet without a palette or sprite `coin`
- **THEN** `art.yaml` gains palette `coin` and sprite `coin` with `_palette: coin`, the command prints `art.yaml: added /palettes/coin, /sprites/coin (8×8, 2 colors)` and exits with code 0

#### Scenario: Rendering the imported sprite
- **WHEN** the sprite imported from `coin.png` is rendered with `pixel-perfect render art.yaml --scale 1`
- **THEN** `sprites/coin.png` decodes to exactly the same pixels as `coin.png`, every pixel with alpha 0 compared as `(0, 0, 0, 0)`

#### Scenario: Description
- **WHEN** the import runs with `--description "Spinning coin, frame 1"`
- **THEN** the new sprite has `description: "Spinning coin, frame 1"` and the palette has no `description`

#### Scenario: Conflicting options
- **WHEN** both `--palette gold` and `--palette-id coin-colors` are given
- **THEN** the command exits with code 2 and the worksheet is unchanged

#### Scenario: Unsupported worksheet extension
- **WHEN** the worksheet is `art.xml`
- **THEN** the command exits with code 2 and the file is unchanged

### Requirement: PNG decoding
The importer SHALL accept PNG files of every color type and bit depth and SHALL
convert them to 8-bit RGBA without loss: grayscale, palette and 1/2/4-bit
images are expanded, and a `tRNS` chunk is applied. A 16-bit image SHALL be
accepted only if every sample is `v × 257` for some 8-bit `v`, and SHALL then
be converted to `v`; otherwise the import SHALL fail with an error giving the
coordinates of the first pixel that cannot be represented. Ancillary color
chunks (`gAMA`, `cHRM`, `sRGB`, `iCCP`) SHALL be ignored: samples are taken as
stored. Every pixel with alpha 0 SHALL be treated as transparent, whatever its
color channels. An animated PNG (one with an `acTL` chunk) and a file that is
not a PNG SHALL be rejected with an error.

#### Scenario: Indexed PNG with transparency
- **WHEN** the PNG is indexed with 4 bits per pixel and a `tRNS` chunk making entry 0 fully transparent
- **THEN** pixels using entry 0 become `.` and the other entries become palette colors

#### Scenario: Exact 16-bit image
- **WHEN** a 16-bit RGBA PNG has only samples of the form `v × 257`
- **THEN** it is imported as the equivalent 8-bit image

#### Scenario: Lossy 16-bit image
- **WHEN** pixel (3, 5) of a 16-bit PNG has a red sample of 1000
- **THEN** the import fails with an error naming pixel (3, 5) and the worksheet is unchanged

#### Scenario: Transparent pixels with different colors
- **WHEN** two pixels are `(255, 0, 0, 0)` and `(0, 0, 255, 0)`
- **THEN** both become `.` and neither adds a palette entry

#### Scenario: Animated PNG
- **WHEN** the PNG has an `acTL` chunk
- **THEN** the import fails with an error saying animated PNG import is not supported

#### Scenario: JPEG input
- **WHEN** `<png>` is a JPEG file
- **THEN** the import fails with an error saying the file is not a PNG

### Requirement: Downscaling enlarged images
With `--scale <N>` (an integer 1–64, default 1), the PNG SHALL be treated as an
N× enlargement: its width and height MUST be multiples of N, and every N×N
block aligned to the origin MUST contain identical pixels (all pixels with
alpha 0 counting as identical). Each block SHALL become one pixel of the
imported sprite. A size that is not a multiple of N, or a block that is not
uniform, SHALL make the import fail with an error giving the size or the
coordinates of the block's top-left pixel in the PNG. All later steps,
including the pixel comparison, SHALL work on the downscaled image.

#### Scenario: Enlarged sprite
- **WHEN** a 64×64 PNG made of uniform 4×4 blocks is imported with `--scale 4`
- **THEN** the sprite has 16 lines of 16 characters

#### Scenario: Size not a multiple
- **WHEN** a 30×32 PNG is imported with `--scale 4`
- **THEN** the import fails with an error saying 30 is not a multiple of 4

#### Scenario: Block not uniform
- **WHEN** the 4×4 block starting at pixel (8, 12) contains two colors and the import uses `--scale 4`
- **THEN** the import fails with an error naming block (8, 12)

### Requirement: New palette generation
Without `--palette`, the importer SHALL create a palette whose id is
`--palette-id`, or `--id` when it is not given. The palette SHALL have one
entry per distinct non-transparent RGBA value of the image, in the order in
which the colors first appear when the image is scanned row by row from the
top-left pixel. Characters SHALL be assigned in that same order from the
import alphabet (see Import alphabet). Transparent pixels SHALL be written as
`.` and SHALL NOT have a palette entry. An image with more distinct
non-transparent colors than `--max-colors` SHALL be rejected with an error
giving the number of colors found; the importer SHALL NOT reduce colors.
`--max-colors` SHALL default to 256 and accept any value from 1 to the length
of the import alphabet. An image with no non-transparent pixel SHALL be
rejected, since a palette needs at least one color.

#### Scenario: Character order
- **WHEN** the first row is `red, red, blue` and the second row is `green, blue, transparent`
- **THEN** the palette is `a` = red, `b` = blue, `c` = green and the lines are `aab` and `cb.`

#### Scenario: Many colors
- **WHEN** the image has 288 distinct colors and the import uses `--max-colors 300`
- **THEN** the 92 ASCII characters of the alphabet are used first, then the next 196 single-column characters, and the rendered sprite matches the PNG

#### Scenario: Too many colors
- **WHEN** a 768×768 image has 41 230 distinct colors
- **THEN** the import fails with an error saying it has 41230 colors, more than `--max-colors` 256, and that the importer does not quantize

#### Scenario: Large explicit limit
- **WHEN** the same image is imported with `--max-colors 50000`
- **THEN** the import succeeds, using characters beyond the single-column tier if needed, and the rendered sprite matches the PNG

#### Scenario: Invalid color limit
- **WHEN** `--max-colors` is 0 or larger than the import alphabet
- **THEN** the command exits with code 2

### Requirement: Import alphabet
The import alphabet SHALL contain every character accepted by the schema's
`char` pattern, each exactly once, in two tiers, so that characters that keep
the `lines` grid readable are always used first.

**Tier 1** holds the single-column visible characters: those whose general
category is not unassigned (`Cn`), private use (`Co`) or surrogate (`Cs`),
whose East Asian Width is not Wide (`W`) or Fullwidth (`F`), whose bidi class
is not right-to-left (`R`, `AL`), and that are not `Default_Ignorable_Code_Point`
and not noncharacters. Tier 1 SHALL be ordered as `a`–`z`, `A`–`Z`, `0`–`9`,
then `! # $ % & ( ) * + , - / : ; < > ? @ [ ] ^ _ { | } ~`, then
`"`, `'`, `\` and the backtick U+0060 (characters that need escaping in some
formats), then every
other tier-1 character by increasing code point.

**Tier 2** holds every other character accepted by the `char` pattern (wide,
right-to-left, unassigned, private use, noncharacters, and so on), by
increasing code point.

The alphabet SHALL be derived from a fixed, documented Unicode version, so
that the same image always gets the same characters whatever the build.
Changing that version SHALL be an explicit change of this specification.
Characters a worksheet format cannot hold literally SHALL be written escaped
(see Format-preserving edit).

#### Scenario: Readable characters first
- **WHEN** an image has 100 colors
- **THEN** every assigned character is in tier 1, and none is wide, right-to-left, invisible or a combining mark

#### Scenario: Complete coverage
- **WHEN** the alphabet is compared with every Unicode scalar value matching the `char` pattern
- **THEN** both sets are equal and the alphabet has no duplicates

#### Scenario: Wide characters only after tier 1
- **WHEN** a character of tier 2 is assigned
- **THEN** every character of tier 1 has already been assigned

#### Scenario: Fully transparent image
- **WHEN** every pixel has alpha 0
- **THEN** the import fails with an error saying the image has no visible pixel

### Requirement: Alpha authoring
Each generated palette entry SHALL write `rgba` as `[r, g, b]` when its 8-bit
alpha is 255. Otherwise it SHALL write `[r, g, b, a]`, where `a` is the decimal
number with the fewest fraction digits, and among those the smallest, such that
`round_half_up(a × 255)` equals the 8-bit alpha. Written alphas SHALL therefore
convert back to exactly the alpha of the PNG.

#### Scenario: Opaque color
- **WHEN** a color is `(240, 190, 40, 255)`
- **THEN** the entry is written with `rgba: [240, 190, 40]`

#### Scenario: Half alpha
- **WHEN** a color has 8-bit alpha 128
- **THEN** its alpha is written as `0.5`, since `round_half_up(0.5 × 255)` is 128

#### Scenario: Alpha needing more digits
- **WHEN** a color has 8-bit alpha 153
- **THEN** its alpha is written as `0.6`

#### Scenario: Every alpha round-trips
- **WHEN** a PNG uses each 8-bit alpha value from 1 to 254
- **THEN** after import every rendered pixel has the same alpha as the PNG

### Requirement: Reusing an existing palette
With `--palette <id>`, the importer SHALL NOT create a palette: the sprite's
`_palette` SHALL be `<id>`, which MUST be a palette of the worksheet. Each
non-transparent color of the image SHALL be written with the character of the
resolved palette (after `_palette` inheritance) whose color, converted to 8
bits, is equal to it; when several characters match, the first one in the
resolved palette's order SHALL be used. If any color of the image has no
matching character, the import SHALL fail with an error listing the missing
colors (at most 10, with the first pixel where each appears) and the total
number missing.

#### Scenario: Shared palette
- **WHEN** `slime-blue.png` uses only colors of palette `slime` and the import uses `--palette slime --id slime-blue`
- **THEN** only sprite `slime-blue` is added, with `_palette: slime`, and its lines use the characters of `slime`

#### Scenario: Inherited colors
- **WHEN** palette `ice` derives from `slime` and the PNG uses an eye color defined only in `slime`
- **THEN** the importer uses the eye character inherited by `ice`

#### Scenario: Missing color
- **WHEN** the PNG contains `(255, 0, 255)`, first at pixel (2, 7), and palette `slime` has no such color
- **THEN** the import fails with an error naming `(255, 0, 255)` at (2, 7) and the worksheet is unchanged

#### Scenario: Unknown palette
- **WHEN** `--palette nope` is given and the worksheet has no palette `nope`
- **THEN** the import fails and the worksheet is unchanged

### Requirement: No overwriting
The import SHALL fail, leaving the worksheet unchanged, if the worksheet
already has a sprite with id `--id` or, when a palette is created, a palette
with the new palette's id. The new ids MUST be valid ids per the schema. The
importer SHALL never modify or remove existing entries.

#### Scenario: Existing sprite
- **WHEN** the worksheet already has sprite `coin`
- **THEN** importing with `--id coin` fails with an error pointing to that sprite

#### Scenario: Existing palette
- **WHEN** the worksheet has palette `coin` but no sprite `coin`
- **THEN** importing with `--id coin` fails with an error suggesting `--palette coin` or `--palette-id`

### Requirement: Exports
With `--export`, if the worksheet has an `exports` list, the importer SHALL
append `/sprites/<id>` to it. If the worksheet has no `exports`, every resource
is already exported, so `--export` SHALL change nothing more. Without
`--export`, `exports` SHALL NOT be modified.

#### Scenario: Explicit exports list
- **WHEN** the worksheet has `exports: ["/scenes/night"]` and the import uses `--export --id coin`
- **THEN** `exports` becomes `["/scenes/night", "/sprites/coin"]`

#### Scenario: No exports list
- **WHEN** the worksheet has no `exports` and the import uses `--export`
- **THEN** no `exports` key is added

### Requirement: Format-preserving edit
The importer SHALL edit the worksheet by inserting text only: every byte of the
original file SHALL appear unchanged and in the same order in the result,
including comments, blank lines, key order, quoting and number formatting.
New entries SHALL be appended at the end of `palettes`, `sprites` and, with
`--export`, `exports`. When a collection is missing, the importer SHALL add it
at the end of the top-level mapping. Inserted text SHALL use the file's line
endings (LF or CRLF) and the indentation of the existing entries of the same
collection, and SHALL quote or escape characters as each format requires. If
the file uses a layout the importer cannot extend by insertion alone (for
example a YAML flow collection of mappings or spanning several lines, a
multi-document YAML file, or an anchor, alias or tag on a collection to
extend), the import SHALL fail with an error naming the construct, instead of
rewriting the file. A YAML flow sequence of scalars written on one line (such
as `exports: ["/sprites/a"]`) SHALL be extended in place, after its last
item.

#### Scenario: Comments kept
- **WHEN** a PNG is imported into a copy of `examples/sprites.yaml`
- **THEN** removing the inserted lines from the result gives back the original file byte for byte

#### Scenario: Same result in every format
- **WHEN** the same PNG is imported into equivalent JSON, YAML and TOML worksheets
- **THEN** the three results load to the same data model

#### Scenario: Missing sprites collection
- **WHEN** the worksheet has `palettes` but no `sprites`
- **THEN** a `sprites` collection holding the new sprite is added at the end of the top-level mapping

#### Scenario: Character needing quotes
- **WHEN** a generated character is `#` in a YAML worksheet
- **THEN** it is written quoted so that it is not read as a comment, and the result loads with `char` equal to `#`

#### Scenario: Character that needs escaping
- **WHEN** a generated character is the noncharacter U+FFFE in a YAML worksheet, where it cannot appear unescaped
- **THEN** it is written as `"￾"` in `char` and as `￾` inside the double-quoted `lines`, and the result loads with that character

#### Scenario: CRLF file
- **WHEN** the worksheet uses CRLF line endings
- **THEN** every inserted line ends with CRLF

#### Scenario: YAML one-line flow sequence of pointers
- **WHEN** the YAML worksheet has `exports: ["/sprites/heart", "/sprites/heart-dark"]` and the import uses `--export --id coin`
- **THEN** the line becomes `exports: ["/sprites/heart", "/sprites/heart-dark", "/sprites/coin"]`

#### Scenario: YAML flow collection
- **WHEN** the YAML worksheet has `sprites: [ { id: a, _palette: p, lines: ["a"] } ]`
- **THEN** the import fails with an error about the flow collection and the file is unchanged

### Requirement: Verification before writing
Before writing anything, the importer SHALL load and fully validate (schema
and semantic checks) the original worksheet; if it is invalid, the import
SHALL fail and report its problems as `validate` does. After building the new
text, the importer SHALL check that:
1. the new text loads and passes full validation;
2. its data model equals the original one with only the new entries appended
   to `palettes`, `sprites` and (with `--export`) `exports`;
3. rendering the new sprite at scale 1 gives exactly the pixels of the
   (downscaled) PNG, every pixel with alpha 0 compared as `(0, 0, 0, 0)`;
4. the original text is a subsequence of the new text, so that nothing was
   removed or changed (see Format-preserving edit).

If any check fails, the import SHALL fail with an error naming the check, and
the worksheet SHALL be unchanged.

#### Scenario: Invalid original worksheet
- **WHEN** the worksheet has a sprite using a character missing from its palette
- **THEN** the import fails, reports that problem with its JSON Pointer, and the file is unchanged

#### Scenario: Wrong insertion point
- **WHEN** a bug in the editor inserts the sprite inside another collection
- **THEN** check 2 fails and the file is unchanged

### Requirement: Writing the result
Without `--dry-run`, the importer SHALL replace the worksheet atomically: it
SHALL write the new text to a temporary file in the same directory, keep the
original file's permissions, and rename it over the original, so that the
worksheet is always either the original or the complete result. With
`--dry-run`, the importer SHALL run every check and print the complete
resulting worksheet to standard output, and SHALL NOT modify any file; the
summary line SHALL then go to standard error.

#### Scenario: Dry run
- **WHEN** the import runs with `--dry-run`
- **THEN** standard output holds the full new worksheet, the file on disk is unchanged, and the exit code is 0

#### Scenario: Failed check leaves file untouched
- **WHEN** any check fails
- **THEN** the worksheet's content and modification time are unchanged and no temporary file is left behind
