## MODIFIED Requirements

### Requirement: Render command
The CLI SHALL provide `pixel-perfect render <worksheet> [--output-path <dir>]
[--format <format>] [--scale <N>] [--safe-build] [--single-frames]
[--max-image-size <SIZE>] [--threads <N>]`, with `-o` as a short alias of
`--output-path`. It
SHALL load and validate the worksheet as `validate` does and write nothing if
the worksheet is invalid. `--output-path` SHALL default to the current
directory. `--format` SHALL accept `png` and `webp` and SHALL default to `png`.
`--single-frames` is described in the single-frame-export capability.

`--threads` SHALL set the number of worker threads used for rendering and
encoding, an integer from 1 to 65535. Without it, the `RAYON_NUM_THREADS`
environment variable SHALL be used when it is set to a positive integer.
Otherwise the default SHALL be the number of physical CPU cores, at most the
number of CPUs the process may use. The written files SHALL be byte-identical
whatever the number of threads.

#### Scenario: Render a valid worksheet
- **WHEN** `pixel-perfect render hero.json --output-path out` runs on a valid worksheet with sprite `hero`
- **THEN** `out/sprites/hero.png` and `out/manifest.json` are written and the exit code is 0

#### Scenario: Render as WebP
- **WHEN** `pixel-perfect render hero.json -o out --format webp` runs on the same worksheet
- **THEN** `out/sprites/hero.webp` and `out/manifest.json` (with `options.format` `"webp"`) are written and the exit code is 0

#### Scenario: Render single frames
- **WHEN** `pixel-perfect render spin.json -o out --single-frames` runs on a worksheet with flipbook `spin`
- **THEN** the frames are written under `out/flipbooks/spin/`, `out/manifest.json` has `options.singleFrames` true, and the exit code is 0

#### Scenario: Invalid worksheet
- **WHEN** `pixel-perfect render broken.json -o out` runs on an invalid worksheet
- **THEN** the diagnostics are printed as by `validate`, no file is written and the exit code is 1

#### Scenario: Unknown format
- **WHEN** `pixel-perfect render hero.json --format gif` runs
- **THEN** a usage error is printed and the exit code is 2

#### Scenario: Thread count does not change the output
- **WHEN** an animated worksheet is rendered once with `--threads 1` and once with `--threads 4`, as PNG, as WebP and with `--single-frames`
- **THEN** each pair of renders writes byte-identical files

#### Scenario: Default thread count
- **WHEN** `pixel-perfect render hero.json` runs without `--threads` and without `RAYON_NUM_THREADS` on a machine with 8 physical cores and 16 logical CPUs
- **THEN** 8 worker threads are used

#### Scenario: Option over environment
- **WHEN** `RAYON_NUM_THREADS=5 pixel-perfect render hero.json --threads 2` runs
- **THEN** 2 worker threads are used

#### Scenario: Zero threads
- **WHEN** `pixel-perfect render hero.json --threads 0` runs
- **THEN** a usage error is printed and the exit code is 2
