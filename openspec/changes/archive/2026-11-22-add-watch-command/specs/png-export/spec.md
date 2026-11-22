## MODIFIED Requirements

### Requirement: Output paths
By default, each exported resource SHALL be written to
`<output-path>/<collection>/<id>.<format>`, where `<collection>` is the worksheet
collection name (`sprites`, `flipbooks`, `panels`, `timelines`, `scenes`,
`sequences`). With `--single-frames`, an animated resource SHALL instead be
written as a directory of frames, `<output-path>/<collection>/<id>/`. The
single-frame-export capability describes its layout. Missing directories SHALL
be created. An existing file SHALL be overwritten when its content differs from
the new one, and left untouched (content and modification time) when it is
already byte-identical. This also applies to `manifest.json`. A file that is
written SHALL be written in place, keeping its identity on disk, so that tools
watching it see a modification and not a replacement. Files not produced by
the current render SHALL NOT be deleted.

#### Scenario: Nested output
- **WHEN** sprites `hero` and `coin` are rendered with `--output-path build/art`
- **THEN** the files are `build/art/sprites/hero.png` and `build/art/sprites/coin.png`

#### Scenario: Same id in different collections
- **WHEN** a sprite and a panel are both called `hero` and both are exported
- **THEN** they are written to `sprites/hero.png` and `panels/hero.png` without conflict

#### Scenario: Sequence output
- **WHEN** sequence `parallax` is exported
- **THEN** it is written to `sequences/parallax.png` with the size of its scene

#### Scenario: Sequence output as single frames
- **WHEN** sequence `parallax` is exported with `--single-frames`
- **THEN** its frames are written under `sequences/parallax/`, starting with `sequences/parallax/0000.png`

#### Scenario: Unchanged files are not rewritten
- **WHEN** a worksheet with sprites `hero` and `coin` is rendered, only `coin` is then changed, and it is rendered again to the same output path
- **THEN** `sprites/coin.png` is rewritten, while `sprites/hero.png` keeps its previous modification time

#### Scenario: Changed file is rewritten
- **WHEN** an output file exists with different content, for example from another worksheet
- **THEN** it is overwritten with the new content
