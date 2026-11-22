## ADDED Requirements

### Requirement: Watch command
The CLI SHALL provide `pixel-perfect watch <worksheet>` with the same render
options as `render`: `--output-path` (`-o`), `--format`, `--scale`,
`--safe-build`, `--single-frames` and `--max-image-size`, with the same
defaults and validation. Any option added to `render` SHALL also be accepted by
`watch`. Each render made by `watch` SHALL write exactly what `render` would
write with the same worksheet and options.

#### Scenario: Same options as render
- **WHEN** `pixel-perfect watch hero.json -o out --scale 4 --format webp` starts on a valid worksheet
- **THEN** `out/` gets the same files and `manifest.json` as `pixel-perfect render hero.json -o out --scale 4 --format webp`

#### Scenario: Usage error
- **WHEN** `pixel-perfect watch hero.json --scale 0` runs
- **THEN** a usage error is printed and the exit code is 2

### Requirement: Render on start and on change
`watch` SHALL render once at start. It SHALL then watch the worksheet and
render it again after each change. Rapid changes SHALL be debounced: changes
less than the debounce interval apart SHALL give one render, started after the
last of them. Changes SHALL be detected whether an editor writes the file in
place or replaces it by renaming a temporary file. If the file is deleted,
`watch` SHALL report it and keep waiting until the file exists again.

#### Scenario: Edit triggers a render
- **WHEN** `watch` is running on `hero.json` and a line of sprite `hero` is changed and saved
- **THEN** `sprites/hero.png` is rendered again with the new pixels, without restarting the command

#### Scenario: Save by rename
- **WHEN** an editor saves `hero.json` by writing `hero.json.tmp` and renaming it over `hero.json`
- **THEN** the change is detected and rendered, and later saves are still detected

#### Scenario: Burst of writes
- **WHEN** the worksheet is written three times within the debounce interval
- **THEN** it is rendered once, after the last write

#### Scenario: File removed and restored
- **WHEN** the worksheet is deleted while `watch` runs, and later created again
- **THEN** `watch` reports that the file is missing, does not exit, and renders the new file when it appears

### Requirement: Errors while watching
`watch` SHALL keep running when a render fails. When the worksheet does not
parse or does not validate, or the export request fails planning, it SHALL
print the diagnostics as `render` does, write nothing, and keep watching. The output of the last successful
render SHALL stay on disk unchanged. `watch` SHALL exit only when it is
interrupted, or when it cannot watch at all at start.

#### Scenario: Syntax error keeps the last output
- **WHEN** `watch` has rendered `hero.json`, and the file is then saved with a syntax error
- **THEN** the error is printed with its line and column, the files from the previous render are untouched, and `watch` keeps running

#### Scenario: Fixed again
- **WHEN** the syntax error is then corrected and the file saved
- **THEN** the worksheet is rendered again normally

#### Scenario: Missing worksheet at start
- **WHEN** `pixel-perfect watch missing.json` runs and `missing.json` does not exist
- **THEN** an error is printed and the exit code is 2

### Requirement: Render summary
After each render attempt, `watch` SHALL print one line to standard error:
- for a successful render: the render number (1 for the render at start,
  then counting up), the number of files written, the number of unchanged
  files and the duration;
- for a failed render: the render number and the number of diagnostics.

Notices from the render SHALL be printed as `render` prints them.

#### Scenario: Summary of an edit
- **WHEN** the pixels of one sprite out of five are changed, without changing its size, and the worksheet is rendered
- **THEN** the summary line gives the next render number, and reports 1 file written and 5 unchanged (the other four sprites and `manifest.json`)

### Requirement: Ending watch
`watch` SHALL run until it is interrupted, for example with Ctrl+C. An
interrupt SHALL end the command with exit code 0. As in `render`,
`manifest.json` SHALL be written only after every image of a render has been
written. A render cut short by the interrupt therefore never leaves a manifest
that lists files it did not write.

#### Scenario: Interrupt
- **WHEN** `watch` is idle and receives an interrupt
- **THEN** it exits with code 0
