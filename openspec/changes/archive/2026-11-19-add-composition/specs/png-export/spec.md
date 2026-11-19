## MODIFIED Requirements

### Requirement: Render command
The CLI SHALL provide `pixel-perfect render <worksheet> [--output-path <dir>]
[--format <format>] [--scale <N>] [--safe-build] [--max-image-size <SIZE>]`,
with `-o` as a short alias of `--output-path`. It SHALL load and validate the
worksheet as `validate` does and write nothing if the worksheet is invalid.
`--output-path` SHALL default to the current directory and `--format` SHALL
default to `png`, the only value accepted for now.

#### Scenario: Render a valid worksheet
- **WHEN** `pixel-perfect render hero.json --output-path out` runs on a valid worksheet with sprite `hero`
- **THEN** `out/sprites/hero.png` and `out/manifest.json` are written and the exit code is 0

#### Scenario: Invalid worksheet
- **WHEN** `pixel-perfect render broken.json -o out` runs on an invalid worksheet
- **THEN** the diagnostics are printed as by `validate`, no file is written and the exit code is 1

#### Scenario: Unknown format
- **WHEN** `pixel-perfect render hero.json --format gif` runs
- **THEN** a usage error is printed and the exit code is 2

### Requirement: Export selection
The engine SHALL render the resources listed in `exports`, or, when `exports` is
absent, every supported resource. Supported resources are sprites, static
panels and static scenes (see the composition capability for which panels and
scenes are animated). Flipbooks, timelines and animated panels and scenes are
not supported until a later change: an unsupported resource listed in `exports`
SHALL be an error raised before anything is written, while unsupported
resources implied by an absent `exports` SHALL be skipped with a notice on
standard error.

#### Scenario: Explicit exports
- **WHEN** `exports` is `["/sprites/hero"]` and the worksheet also defines sprite `coin`
- **THEN** only `sprites/hero.png` is written

#### Scenario: Implicit exports
- **WHEN** `exports` is absent and the worksheet defines two sprites, a static panel and a flipbook
- **THEN** both sprites and the panel are written, and a notice says the flipbook was skipped because flipbooks are not supported yet

#### Scenario: Static panel and scene
- **WHEN** `exports` is `["/panels/stage", "/scenes/night"]` and both are static
- **THEN** `panels/stage.png` and `scenes/night.png` are written with the panel and scene sizes

#### Scenario: Unsupported explicit export
- **WHEN** `exports` contains `/flipbooks/spin`
- **THEN** rendering fails with an error pointing to that entry, no file is written and the exit code is 1

#### Scenario: Animated scene listed in exports
- **WHEN** `exports` contains `/scenes/night` and that scene has a `_timeline` layer
- **THEN** rendering fails with an error pointing to that entry saying animated scenes are not supported yet, and no file is written

## ADDED Requirements

### Requirement: Image size limit
`render` SHALL accept `--max-image-size <SIZE>`, where SIZE is a positive
integer number of bytes optionally followed, without spaces, by one of the
binary units `KiB`, `MiB`, `GiB`, `TiB` (case-insensitive), or the word `none`.
The default SHALL be `1GiB`. Before rendering, the engine SHALL compute the size
of every image it would build for the selected exports (sprite rasters, the
result of each transform, panel and scene canvases, and the final image after
`--scale`) as `width × height × 4` bytes. If any of them exceeds SIZE, rendering
SHALL fail with an error pointing to the resource or element that produces it,
giving the image size and the limit and suggesting `--max-image-size`; nothing
SHALL be written and the exit code SHALL be 1. With `none` there SHALL be no
limit other than the format's own (each side at most 2³¹ − 1 pixels for PNG).
An invalid SIZE SHALL be a usage error with exit code 2.

#### Scenario: Default limit exceeded
- **WHEN** a static panel of 20000 x 20000 pixels (1.49 GiB as RGBA) is rendered without `--max-image-size`
- **THEN** rendering fails with an error pointing to the panel that mentions the 1 GiB limit and `--max-image-size`, no file is written and the exit code is 1

#### Scenario: Limit applies after scale
- **WHEN** a 4096 x 4096 panel (64 MiB) is rendered with `--scale 8` (4 GiB)
- **THEN** rendering fails because the final image exceeds the default limit

#### Scenario: Intermediate image
- **WHEN** an element has a `scale` transform to 30000 x 30000 inside a 16 x 16 panel
- **THEN** rendering fails with an error pointing to that element's transform, even though the panel itself is small

#### Scenario: Raised limit
- **WHEN** the 20000 x 20000 panel is rendered with `--max-image-size 2GiB`
- **THEN** the plan accepts it

#### Scenario: No limit
- **WHEN** `--max-image-size none` is given
- **THEN** no size limit is applied besides the PNG maximum side

#### Scenario: Invalid size
- **WHEN** `--max-image-size 1GB`, `--max-image-size 0` or `--max-image-size big` is given
- **THEN** a usage error is printed and the exit code is 2
