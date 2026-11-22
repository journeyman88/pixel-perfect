## MODIFIED Requirements

### Requirement: Watch command
The CLI SHALL provide `pixel-perfect watch <worksheet>` with the same render
options as `render`: `--output-path` (`-o`), `--format`, `--scale`,
`--safe-build`, `--single-frames`, `--max-image-size` and `--threads`, with the same
defaults and validation. Any option added to `render` SHALL also be accepted by
`watch`. Each render made by `watch` SHALL write exactly what `render` would
write with the same worksheet and options.

#### Scenario: Same options as render
- **WHEN** `pixel-perfect watch hero.json -o out --scale 4 --format webp` starts on a valid worksheet
- **THEN** `out/` gets the same files and `manifest.json` as `pixel-perfect render hero.json -o out --scale 4 --format webp`

#### Scenario: Usage error
- **WHEN** `pixel-perfect watch hero.json --scale 0` runs
- **THEN** a usage error is printed and the exit code is 2

#### Scenario: Thread count while watching
- **WHEN** `pixel-perfect watch hero.json --threads 2` starts
- **THEN** every render it makes uses 2 worker threads and writes what `render` would
