## MODIFIED Requirements

### Requirement: Render command
The CLI SHALL provide `pixel-perfect render <worksheet> [--output-path <dir>]
[--format <format>] [--scale <N>] [--safe-build] [--max-image-size <SIZE>]`,
with `-o` as a short alias of `--output-path`. It SHALL load and validate the
worksheet as `validate` does and write nothing if the worksheet is invalid.
`--output-path` SHALL default to the current directory. `--format` SHALL accept
`png` and `webp` and SHALL default to `png`.

#### Scenario: Render a valid worksheet
- **WHEN** `pixel-perfect render hero.json --output-path out` runs on a valid worksheet with sprite `hero`
- **THEN** `out/sprites/hero.png` and `out/manifest.json` are written and the exit code is 0

#### Scenario: Render as WebP
- **WHEN** `pixel-perfect render hero.json -o out --format webp` runs on the same worksheet
- **THEN** `out/sprites/hero.webp` and `out/manifest.json` (with `format` `"webp"`) are written and the exit code is 0

#### Scenario: Invalid worksheet
- **WHEN** `pixel-perfect render broken.json -o out` runs on an invalid worksheet
- **THEN** the diagnostics are printed as by `validate`, no file is written and the exit code is 1

#### Scenario: Unknown format
- **WHEN** `pixel-perfect render hero.json --format gif` runs
- **THEN** a usage error is printed and the exit code is 2

### Requirement: Export selection
The engine SHALL render the resources listed in `exports`, or, when `exports` is
absent, every resource of every collection except palettes. With `--format png`,
sprites, static panels and static scenes SHALL be written as PNG, and
flipbooks, timelines and animated panels and scenes (see the composition
capability) SHALL be written as APNG (see the apng-export capability), with the
`.png` extension and the same output paths, even when the animation has a
single frame. With `--format webp`, the same resources SHALL be written as
static and animated WebP respectively (see the webp-export capability), with the
`.webp` extension and the same output paths.

#### Scenario: Explicit exports
- **WHEN** `exports` is `["/sprites/hero"]` and the worksheet also defines sprite `coin`
- **THEN** only `sprites/hero.png` is written

#### Scenario: Implicit exports
- **WHEN** `exports` is absent and the worksheet defines two sprites, a static panel and a flipbook
- **THEN** both sprites and the panel are written as PNG, the flipbook is written as APNG, and no notice is printed

#### Scenario: Static panel and scene
- **WHEN** `exports` is `["/panels/stage", "/scenes/night"]` and both are static
- **THEN** `panels/stage.png` and `scenes/night.png` are written with the panel and scene sizes

#### Scenario: Animated resources
- **WHEN** `exports` contains `/flipbooks/spin`, `/timelines/walk` and `/scenes/night`, and `night` has a `_timeline` layer
- **THEN** `flipbooks/spin.png`, `timelines/walk.png` and `scenes/night.png` are written as APNG with the flipbook, panel and scene sizes

#### Scenario: Animated resources as WebP
- **WHEN** the same exports are rendered with `--format webp`
- **THEN** `flipbooks/spin.webp`, `timelines/walk.webp` and `scenes/night.webp` are written as animated WebP, and a static panel would be written as a static WebP
