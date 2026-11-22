## MODIFIED Requirements

### Requirement: Render on start and on change
`watch` SHALL render once at start. It SHALL then watch the worksheet and
render it again after each change. Changes SHALL be detected whether an
editor writes the file in place or replaces it by renaming a temporary file.

Changes SHALL be debounced:
- after a change, `watch` SHALL wait until no further change arrives for the
  quiet interval, 150 ms, and then render once;
- while changes keep arriving with less than the quiet interval between them,
  `watch` SHALL still render no later than the maximum wait, 2 s, after the
  first change of the burst, and then start waiting again for the next
  changes.

A change that arrives during a render SHALL be rendered by a later render,
which follows the same rules. An interrupt that arrives while `watch` waits
SHALL end it at once, without rendering.

If the file is deleted, `watch` SHALL report it and keep waiting until the
file exists again.

#### Scenario: Edit triggers a render
- **WHEN** `watch` is running on `hero.json` and a line of sprite `hero` is changed and saved
- **THEN** `sprites/hero.png` is rendered again with the new pixels, without restarting the command

#### Scenario: Save by rename
- **WHEN** an editor saves `hero.json` by writing `hero.json.tmp` and renaming it over `hero.json`
- **THEN** the change is detected and rendered, and later saves are still detected

#### Scenario: Burst of writes
- **WHEN** the worksheet is written three times, 40 ms apart
- **THEN** it is rendered once, about 150 ms after the last write

#### Scenario: Long burst of close writes
- **WHEN** the worksheet is written ten times, 100 ms apart
- **THEN** it is rendered once, about 150 ms after the last write

#### Scenario: Changes that never pause
- **WHEN** the worksheet is written every 50 ms for 5 s
- **THEN** it is rendered about every 2 s while the writes go on, and once more about 150 ms after the last one

#### Scenario: Change during a render
- **WHEN** the worksheet is saved again while a render is in progress
- **THEN** another render follows, so the output ends up matching the last save

#### Scenario: Interrupt while waiting
- **WHEN** an interrupt arrives during the quiet interval after a change
- **THEN** `watch` exits with code 0 without rendering that change

#### Scenario: File removed and restored
- **WHEN** the worksheet is deleted while `watch` runs, and later created again
- **THEN** `watch` reports that the file is missing, does not exit, and renders the new file when it appears
