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

### Requirement: Unique ids
Ids SHALL be unique among resources of the same type, and element `id`s SHALL be
unique inside their panel. The same id MAY be reused across different resource
types.

#### Scenario: Duplicate sprite id
- **WHEN** two sprites both have `id` `hero`
- **THEN** validation fails pointing to the second one

#### Scenario: Same id in different collections
- **WHEN** a palette and a sprite both have `id` `hero`
- **THEN** validation succeeds

### Requirement: References resolve to the expected type
Every `_palette`, `_sprite`, `_flipbook`, `_panel` and `_timeline` field SHALL
name an existing resource of that type, and every transition `_element` SHALL name
an element `id` of the timeline's `_panel`.

#### Scenario: Missing palette
- **WHEN** a sprite has `_palette` set to `night` and no palette `night` exists
- **THEN** validation fails pointing to that `_palette`

#### Scenario: Unknown element in a transition
- **WHEN** a timeline transition has `_element` set to `door` and its panel has no element with `id` `door`
- **THEN** validation fails pointing to that `_element`

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

### Requirement: Static transforms stay within bounds
Each `crop` area SHALL lie inside the image as it is at that point of the
`transforms` chain declared on a flipbook frame or panel element, taking
previous `rotate`, `scale` and `crop` steps into account. Chains extended at run
time by `appendTransforms` are out of scope here and are checked when timelines
are rendered.

#### Scenario: Crop after rotation
- **WHEN** a 4x8 sprite is rotated by 90 degrees and then cropped with area `[0, 0, 8, 4]`
- **THEN** validation succeeds

#### Scenario: Crop out of bounds
- **WHEN** a 4x8 sprite is cropped with area `[2, 0, 4, 8]`
- **THEN** validation fails pointing to that `area`

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
