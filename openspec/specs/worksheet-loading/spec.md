# worksheet-loading Specification

## Purpose

Read a pixel-perfect worksheet written in JSON, YAML or TOML, validate it against
`schema/worksheet.schema.json` and the engine-only rules the schema marks as
`$comment: "Engine: ..."`, and resolve palette and sprite inheritance into a
model that rendering can trust. Exposed on the command line as
`pixel-perfect validate`.

## Requirements

### Requirement: Input format by file extension
The engine SHALL read a worksheet as JSON for the extension `.json`, as YAML for
`.yaml` and `.yml`, and as TOML for `.toml`, matching extensions
case-insensitively. Any other extension SHALL be rejected before parsing.

#### Scenario: JSON worksheet
- **WHEN** the file `hero.json` contains a valid worksheet
- **THEN** it is parsed as JSON and loading succeeds

#### Scenario: YAML worksheet
- **WHEN** the file `hero.yml` contains a valid worksheet written in YAML
- **THEN** it is parsed as YAML and loading succeeds

#### Scenario: TOML worksheet
- **WHEN** the file `hero.toml` contains a valid worksheet written in TOML
- **THEN** it is parsed as TOML and loading succeeds

#### Scenario: Unknown extension
- **WHEN** the file `hero.txt` is loaded
- **THEN** loading fails with an error naming the unsupported extension and listing the supported ones

### Requirement: Equivalent data model across formats
The engine SHALL convert every input format into the same JSON data model before
validation, so that equivalent JSON, YAML and TOML worksheets produce identical
results. YAML mapping keys that are integers SHALL be converted to their decimal
string form. Values with no JSON equivalent (TOML date-times, YAML tags other than
the core schema) SHALL be rejected with an error.

#### Scenario: Same worksheet in three formats
- **WHEN** the same worksheet is written in JSON, YAML and TOML
- **THEN** all three load into equal models

#### Scenario: Integer keys in YAML overrideLines
- **WHEN** a YAML sprite has `overrideLines` with key `3` written as an unquoted integer
- **THEN** the key is treated as the string `"3"` and the worksheet validates

#### Scenario: TOML date-time
- **WHEN** a TOML worksheet contains a date-time value
- **THEN** loading fails with an error reporting the location of that value

### Requirement: Syntax errors are reported with position
The engine SHALL report a parse failure with the file name and, when the parser
provides it, the line and column of the error.

#### Scenario: Malformed JSON
- **WHEN** a `.json` file has a missing comma on line 12
- **THEN** loading fails with an error that names the file and line 12

### Requirement: Schema validation
The engine SHALL validate the data model against the embedded
`schema/worksheet.schema.json` (JSON Schema draft 2020-12) and SHALL report every
violation, each with the JSON Pointer of the offending node and a message.
Semantic validation SHALL NOT run when schema validation fails.

#### Scenario: Missing required field
- **WHEN** a sprite has no `id`
- **THEN** validation fails with an error whose pointer is that sprite (e.g. `/sprites/0`)

#### Scenario: Reserved character in palette
- **WHEN** a palette color has `char` set to `.`
- **THEN** validation fails with an error pointing to that `char`

#### Scenario: Several violations
- **WHEN** a worksheet has three independent schema violations
- **THEN** all three are reported, not only the first

### Requirement: Descriptions are annotations
The schema SHALL accept an optional `description` property on the worksheet
root and on every palette, palette color (`colors[]`), transform
(`transforms[]`, for every `kind`), sprite, flipbook,
flipbook frame (`frames[]`), panel, element (`elements[]`), timeline,
transition (`transitions[]`, for every `kind`), scene and layer (`layers[]`).
A `description` SHALL be a string of at least one character; any other value
SHALL be a schema violation whose pointer is that `description`.

A `description` SHALL NOT influence semantic validation, rendered pixels,
frame timing, output file names, `--safe-build` hashes or `manifest.json`.

#### Scenario: Description on the worksheet root
- **WHEN** a worksheet has a top-level `description: "Night forest"`
- **THEN** it passes schema validation

#### Scenario: Description on every annotatable object
- **WHEN** a worksheet sets `description` on a palette color, a transform of each `kind`, a flipbook frame, a transition of each `kind` and a scene layer
- **THEN** it passes schema and semantic validation

#### Scenario: Empty description
- **WHEN** a scene layer has `description: ""`
- **THEN** validation fails with an error pointing to `/scenes/0/layers/0/description`

#### Scenario: Non-string description
- **WHEN** a flipbook frame has `description: 42`
- **THEN** validation fails with an error pointing to `/flipbooks/0/frames/0/description`

#### Scenario: Description on a transform
- **WHEN** a sprite has `transforms: [{ kind: flip, axis: x, description: "Face left" }]`
- **THEN** it passes validation and renders exactly as the same transform without `description`

#### Scenario: Output unaffected
- **WHEN** the same worksheet is rendered once with descriptions on every annotatable object and once with none
- **THEN** both runs write byte-identical image files and an identical `manifest.json`

### Requirement: Unique ids
Ids SHALL be unique among resources of the same type, element `id`s SHALL be
unique inside their panel, and layer `id`s SHALL be unique inside their scene.
The same id MAY be reused across different resource types, and layers without
`id` SHALL NOT conflict.

#### Scenario: Duplicate sprite id
- **WHEN** two sprites both have `id` `hero`
- **THEN** validation fails pointing to the second one

#### Scenario: Same id in different collections
- **WHEN** a palette and a sprite both have `id` `hero`
- **THEN** validation succeeds

#### Scenario: Duplicate layer id
- **WHEN** two layers of scene `night` both have `id` `lamp`
- **THEN** validation fails pointing to the `id` of the second one

#### Scenario: Same layer id in different scenes
- **WHEN** scenes `night` and `day` both have a layer with `id` `lamp`
- **THEN** validation succeeds

### Requirement: References resolve to the expected type
References SHALL resolve: every `_palette`, `_sprite`, `_flipbook`, `_panel`,
`_timeline` and `_scene` field SHALL name an existing resource of that type. Every transition `_element`
SHALL name an element `id` of the timeline's `_panel`. Every action `_layer`
SHALL name a layer `id` of the sequence's `_scene`.

#### Scenario: Missing palette
- **WHEN** a sprite has `_palette` set to `night` and no palette `night` exists
- **THEN** validation fails pointing to that `_palette`

#### Scenario: Unknown element in a transition
- **WHEN** a timeline transition has `_element` set to `door` and its panel has no element with `id` `door`
- **THEN** validation fails pointing to that `_element`

#### Scenario: Unknown scene
- **WHEN** a sequence has `_scene` set to `night` and no scene `night` exists
- **THEN** validation fails pointing to that `_scene`

#### Scenario: Unknown layer in an action
- **WHEN** a sequence action has `_layer` set to `lamp` and its scene has no layer with `id` `lamp`
- **THEN** validation fails pointing to that `_layer`

### Requirement: No inheritance cycles
The engine SHALL reject cycles in palette inheritance (`_palette` on a palette)
and in sprite inheritance (`_sprite` on a sprite).

#### Scenario: Palette cycle
- **WHEN** palette `a` derives from `b` and `b` derives from `a`
- **THEN** validation fails with an error listing the cycle `a -> b -> a`

#### Scenario: Self-derived sprite
- **WHEN** sprite `x` has `_sprite` set to `x`
- **THEN** validation fails with an error listing the cycle `x -> x`

### Requirement: Palette characters are unique
Inside the `colors` array of a single palette, each `char` SHALL appear at most
once. A derived palette SHALL override base entries with the same `char` and add
the others.

#### Scenario: Duplicate char in one palette
- **WHEN** a palette lists two colors with `char` `r`
- **THEN** validation fails pointing to the second one

#### Scenario: Override in a derived palette
- **WHEN** palette `dark` derives from `warm` and redefines `char` `r`
- **THEN** validation succeeds and `dark` resolves `r` to its own color

### Requirement: Sprite grids are consistent
After resolving inheritance, every sprite SHALL have rows of equal width, measured
in Unicode scalar values. If `width` or `height` is present it SHALL match the
resolved grid. Every character other than `.` SHALL exist in the sprite's resolved
palette.

#### Scenario: Ragged rows
- **WHEN** a sprite has rows of width 6 and 7
- **THEN** validation fails pointing to the first row whose width differs from row 0

#### Scenario: Declared size mismatch
- **WHEN** a sprite declares `height` 8 but has 6 rows
- **THEN** validation fails pointing to `height`

#### Scenario: Unknown character
- **WHEN** a sprite row contains `z` and the resolved palette has no `z`
- **THEN** validation fails reporting the character, row and column

#### Scenario: Palette swap removes a character
- **WHEN** a derived sprite sets `_palette` to a palette lacking a character used by the base sprite
- **THEN** validation fails reporting that character on the derived sprite

### Requirement: Line overrides are consistent
Each `overrideLines` key SHALL be lower than the base sprite's height, and each
value SHALL have the base sprite's width. The character `=` SHALL copy the base
sprite's character in the same column, which is then resolved with the derived
sprite's palette.

#### Scenario: Index out of range
- **WHEN** a derived sprite overrides line `9` of a 6-row base sprite
- **THEN** validation fails pointing to that key

#### Scenario: Wrong override width
- **WHEN** an override value is 5 characters wide and the base sprite is 7 wide
- **THEN** validation fails pointing to that value

#### Scenario: Pass-through character
- **WHEN** an override is `==rr===` on a base row `.hhhhh.`
- **THEN** the resolved row is `.hrrhh.`

### Requirement: Transforms stay within bounds
Each `crop` area SHALL lie inside the image as it is at that point of the
`transforms` chain, taking previous `rotate`, `scale`, `tile` and `crop` steps
into account. This SHALL hold for the chains declared on flipbook frames and
panel elements, and for every chain an element can have during a timeline: its
own `transforms` followed by the `transforms` of the `appendTransforms` applied
since the last `resetTransforms`, in the order in which the transitions are
applied (by `at`, ties in array order). An error in an appended `crop` SHALL
point to the `area` inside that `appendTransforms` transition.

#### Scenario: Crop after rotation
- **WHEN** a 4x8 sprite is rotated by 90 degrees and then cropped with area `[0, 0, 8, 4]`
- **THEN** validation succeeds

#### Scenario: Crop out of bounds
- **WHEN** a 4x8 sprite is cropped with area `[2, 0, 4, 8]`
- **THEN** validation fails pointing to that `area`

#### Scenario: Crop after tile
- **WHEN** a 4x8 sprite is tiled to 32x8 and then cropped with area `[20, 0, 12, 8]`
- **THEN** validation succeeds

#### Scenario: Crop outside the tiled area
- **WHEN** a 4x8 sprite is tiled to 16x8 and then cropped with area `[8, 0, 12, 8]`
- **THEN** validation fails pointing to that `area`

#### Scenario: Appended crop after an appended rotation
- **WHEN** a 4x8 sprite element gets `appendTransforms` `[rotate 90]` at 2 and `appendTransforms` `[crop [0, 0, 8, 4]]` at 4
- **THEN** validation succeeds

#### Scenario: Appended crop out of bounds
- **WHEN** a 4x8 sprite element gets `appendTransforms` `[crop [0, 0, 8, 4]]` at 4 and no rotation before it
- **THEN** validation fails pointing to the `area` of that transition's `crop`

#### Scenario: Reset restores the declared chain
- **WHEN** a 4x8 sprite element gets `appendTransforms` `[rotate 90]` at 2, `resetTransforms` at 3 and `appendTransforms` `[crop [0, 0, 8, 4]]` at 4
- **THEN** validation fails pointing to the `crop` added at 4, because the rotation was removed at 3

### Requirement: Export pointers resolve
Each entry of `exports` SHALL resolve to an existing resource of the named
collection.

#### Scenario: Unknown export
- **WHEN** `exports` contains `/scenes/night` and no scene `night` exists
- **THEN** validation fails pointing to that entry

### Requirement: Validate command
The CLI SHALL provide `pixel-perfect validate <file>...`. It SHALL validate each
file independently, print every error to standard error as
`<file>: <pointer or line:column>: <message>`, and exit with code 0 when all files
are valid, 1 when at least one file is invalid, and 2 on usage errors,
unreadable files or unsupported extensions.

#### Scenario: All files valid
- **WHEN** `pixel-perfect validate a.json b.yaml` runs and both are valid
- **THEN** the exit code is 0 and nothing is printed to standard error

#### Scenario: One invalid file
- **WHEN** `pixel-perfect validate a.json bad.toml` runs and only `bad.toml` is invalid
- **THEN** the errors of `bad.toml` are printed and the exit code is 1

#### Scenario: Missing file
- **WHEN** `pixel-perfect validate missing.json` runs and the file does not exist
- **THEN** an error is printed and the exit code is 2

### Requirement: Timeline transitions are consistent
The engine SHALL check the transitions of every timeline:
- `at` and `start` SHALL be less than the timeline's `duration`, and
  `start + duration` of a `moveTo` SHALL be at most the timeline's `duration`;
- two `moveTo` on the same element SHALL NOT overlap: when ordered by `start`,
  each SHALL start at or after `start + duration` of the previous one;
- a `seekFlipbook` SHALL target an element that has `_flipbook`, and its
  `frameIndex` SHALL be less than the number of `frames` of that flipbook.

Each error SHALL point to the offending field of the transition.

#### Scenario: Instant transition after the end
- **WHEN** a timeline with `duration` 40 has a `setVisible` with `at` 40
- **THEN** validation fails pointing to that `at`

#### Scenario: Movement ending at the loop boundary
- **WHEN** a timeline with `duration` 40 has a `moveTo` with `start` 30 and `duration` 10
- **THEN** validation succeeds

#### Scenario: Movement past the end
- **WHEN** a timeline with `duration` 40 has a `moveTo` with `start` 35 and `duration` 10
- **THEN** validation fails pointing to that transition's `duration`

#### Scenario: Overlapping movements
- **WHEN** element `hero` has `moveTo` with `start` 0 and `duration` 10, and another with `start` 5
- **THEN** validation fails pointing to the `start` of the second one

#### Scenario: Back-to-back movements
- **WHEN** element `hero` has `moveTo` with `start` 0 and `duration` 10, and another with `start` 10
- **THEN** validation succeeds

#### Scenario: Seek on a sprite element
- **WHEN** a `seekFlipbook` targets element `hero`, which has `_sprite`
- **THEN** validation fails pointing to that `_element`

#### Scenario: Frame index out of range
- **WHEN** a `seekFlipbook` has `frameIndex` 4 and its element's flipbook has 4 frames
- **THEN** validation fails pointing to that `frameIndex`

### Requirement: Palette characters are visible
A palette `char` SHALL be a single character that is not `.`, `=`,
whitespace, a control character (`Cc`), a format character (`Cf`, for example
U+200B ZERO WIDTH SPACE or U+FEFF) or a combining mark (`M`: `Mn`, `Mc`,
`Me`). The schema SHALL express this as the pattern
`^[^.=\s\p{Cc}\p{Cf}\p{M}]$`, so that no character is invisible or merges
with its neighbour in `lines`.

**BREAKING**: worksheets using a format character or a combining mark as
`char` no longer validate. None of the examples or test fixtures do.

#### Scenario: Zero width space rejected
- **WHEN** a palette has `{ char: "​", rgba: [0, 0, 0] }`
- **THEN** validation fails with an error pointing to that entry's `char`

#### Scenario: Combining mark rejected
- **WHEN** a palette has `char` U+0301 COMBINING ACUTE ACCENT
- **THEN** validation fails with an error pointing to that entry's `char`

#### Scenario: Precomposed letter accepted
- **WHEN** a palette has `char` `é` (U+00E9, a single precomposed character)
- **THEN** validation succeeds

#### Scenario: Wide character still accepted
- **WHEN** a palette has `char` `山`
- **THEN** validation succeeds

### Requirement: Sequence actions are consistent
The engine SHALL check the actions of every sequence:
- `at` and `start` SHALL be less than the sequence's `duration`, and
  `start + duration` of a `moveTo` SHALL be at most the sequence's `duration`;
- two `moveTo` on the same layer SHALL NOT overlap: when ordered by `start`,
  each SHALL start at or after `start + duration` of the previous one.

Each error SHALL point to the offending field of the action.

#### Scenario: Instant action after the end
- **WHEN** a sequence with `duration` 16 has a `setVisible` with `at` 16
- **THEN** validation fails pointing to that `at`

#### Scenario: Movement ending at the loop boundary
- **WHEN** a sequence with `duration` 16 has a `moveTo` with `start` 8 and `duration` 8
- **THEN** validation succeeds

#### Scenario: Movement past the end
- **WHEN** a sequence with `duration` 16 has a `moveTo` with `start` 10 and `duration` 8
- **THEN** validation fails pointing to that action's `duration`

#### Scenario: Overlapping movements
- **WHEN** layer `far` has `moveTo` with `start` 0 and `duration` 8, and another with `start` 4
- **THEN** validation fails pointing to the `start` of the second one

#### Scenario: Removed action kinds
- **WHEN** a sequence action has `kind` `appendTransforms`
- **THEN** validation fails pointing to that `kind`
