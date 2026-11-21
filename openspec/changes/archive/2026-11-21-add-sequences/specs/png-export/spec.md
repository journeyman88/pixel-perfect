## MODIFIED Requirements

### Requirement: Output paths
By default, each exported resource SHALL be written to
`<output-path>/<collection>/<id>.<format>`, where `<collection>` is the worksheet
collection name (`sprites`, `flipbooks`, `panels`, `timelines`, `scenes`,
`sequences`). Missing directories SHALL be created and existing files SHALL be
overwritten. Files not produced by the current render SHALL NOT be deleted.

#### Scenario: Nested output
- **WHEN** sprites `hero` and `coin` are rendered with `--output-path build/art`
- **THEN** the files are `build/art/sprites/hero.png` and `build/art/sprites/coin.png`

#### Scenario: Same id in different collections
- **WHEN** a sprite and a panel are both called `hero` and both are exported
- **THEN** they are written to `sprites/hero.png` and `panels/hero.png` without conflict

#### Scenario: Sequence output
- **WHEN** sequence `parallax` is exported
- **THEN** it is written to `sequences/parallax.png` with the size of its scene

### Requirement: Export selection
The engine SHALL render the resources listed in `exports`, or, when `exports` is
absent, every resource of every collection except palettes. With `--format png`,
sprites, static panels and static scenes SHALL be written as PNG, and
flipbooks, timelines, sequences and animated panels and scenes (see the
composition capability) SHALL be written as APNG (see the apng-export
capability), with the `.png` extension and the same output paths, even when the
animation has a single frame. With `--format webp`, the same resources SHALL be
written as static and animated WebP respectively (see the webp-export
capability), with the `.webp` extension and the same output paths.

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

#### Scenario: Implicit export of a sequence
- **WHEN** `exports` is absent and the worksheet defines scene `night` and sequence `flicker` over it
- **THEN** `scenes/night.png` and `sequences/flicker.png` are both written, the sequence as APNG
