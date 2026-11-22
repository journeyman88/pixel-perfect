## Why

A worksheet is plain text and is edited by trial: change a line, look at the
result, fix it. Today every round needs a manual `render`. A `watch` command
that renders again on every save closes that loop. Paired with a game engine
or image viewer that reloads changed files, it gives a live preview without
pixel-perfect drawing any window, so the project still does one thing: it
renders.

## What Changes

- New `pixel-perfect watch <worksheet>` subcommand. It does the following:
  - renders once at start;
  - watches the worksheet and renders it again after every change, until
    interrupted;
  - accepts the same render options as `render`;
  - on an invalid worksheet or export request, prints the diagnostics and
    keeps the last good output on disk;
  - prints one summary line per render.
- `watch` is a separate subcommand, not a `render --watch` flag. This leaves
  room for options that only make sense while watching. Its render options are
  shared with `render` through one argument structure, so the two commands
  cannot drift apart.
- Files whose content would not change are no longer rewritten, by `render` as
  well as by `watch`. Output is deterministic, so a file whose bytes are
  already identical is left untouched, together with its modification time. A
  tool that reloads changed files then reloads only the images that really
  changed.
- File watching uses the `notify` crate, which is pure Rust and supports Linux,
  macOS and Windows. Changes are debounced.

## Capabilities

### New Capabilities

- `watch-command`: the `watch` subcommand. It covers the startup render, change
  detection and debouncing, re-rendering, error handling while watching, the
  summary line and how the command ends.

### Modified Capabilities

- `png-export`: in *Output paths*, an existing file whose content is identical
  is no longer rewritten.

## Impact

- `src/main.rs`:
  - the render options move into a shared `RenderArgs` structure, included in
    both `render` and `watch` with `#[command(flatten)]`;
  - new `watch` subcommand and its loop.
- `src/export.rs`:
  - `write_file` skips files whose content is identical, and still writes
    the others in place;
  - a render reports how many files it wrote and how many were unchanged.
- New dependencies, all pure Rust: `notify` and `notify-debouncer-full` for watching, and `ctrlc` to end cleanly with exit code 0 on Ctrl+C.
- Tests:
  - unit tests for skipping identical files;
  - an integration test that starts `watch`, edits the worksheet and checks the
    new output, with generous timeouts.
- `README.md`: document `watch`.
