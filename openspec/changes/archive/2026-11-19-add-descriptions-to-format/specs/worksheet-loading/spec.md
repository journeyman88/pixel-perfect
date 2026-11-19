## ADDED Requirements

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
