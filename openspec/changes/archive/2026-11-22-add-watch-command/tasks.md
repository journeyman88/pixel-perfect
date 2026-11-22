## 1. Writing files: skip identical ones

- [x] 1.1 Change `write_file` in `src/export.rs` to return `Written::{Changed, Unchanged}`:
  - skip a file whose length and bytes are already identical;
  - otherwise write it in place, as today.
- [x] 1.2 Count the outcomes in `write()`. Add `RenderReport { plan, manifest, written, unchanged }` and `export::render_report`, and make `export::render` call it.
- [x] 1.3 Tests in `tests/export.rs`:
  - rendering twice reports `written: 0` the second time;
  - a file whose mtime was set to the past with `File::set_modified` keeps that mtime after an identical render;
  - after changing one sprite, only that file is rewritten and the others keep their mtime;
  - a file with different content is overwritten in place, keeping its inode on Unix.

## 2. Shared render options

- [x] 2.1 Move the worksheet path and the render options of `Command::Render` in `src/main.rs` into `#[derive(Args)] struct RenderArgs` with `options()`, and include it with `#[command(flatten)]`.
- [x] 2.2 Switch the `render` helper to `export::render_report`, with no change in its output.
- [x] 2.3 Test: the existing CLI tests pass unchanged (`tests/cli.rs`), and `pixel-perfect render --help` still lists every option.

## 3. The watch command

- [x] 3.1 Add the dependencies `notify`, `notify-debouncer-full` and `ctrlc`, using current versions, with `cargo add`.
- [x] 3.2 Add `Command::Watch { #[command(flatten)] args: RenderArgs }`, with help text.
- [x] 3.3 Implement the loop, keeping the event handling in a function that tests can drive:
  - render once at start;
  - debounce at 150 ms on the worksheet's parent directory;
  - keep only events for the worksheet's file name;
  - drain the queue before each render;
  - print the summary line `[n] rendered: W written, U unchanged, T ms` or `[n] failed: D diagnostics`;
  - on deletion, print `missing, waiting for it to come back` and keep watching.
- [x] 3.4 Handle Ctrl+C with `ctrlc`: send a stop message to the loop's channel, end after any render in progress, and exit with 0.
- [x] 3.5 Exit codes at start:
  - 2 for a missing worksheet, an unsupported extension or a watcher that cannot start;
  - an invalid worksheet at start prints the diagnostics and keeps watching.
- [x] 3.6 Unit tests of the event handling (no real watcher):
  - events for other files, including the render's own outputs, are ignored;
  - a burst gives one render;
  - removal then creation gives a "missing" message, then a render;
  - a stop message ends the loop.

## 4. End-to-end tests (`tests/watch.rs`)

- [x] 4.1 Add a fixture, `tests/fixtures/valid/watch.json`, with two small sprites.
- [x] 4.2 Edit-triggered render:
  - spawn `pixel-perfect watch` on a copy of the fixture in a temporary directory and wait for the first `manifest.json`;
  - change one sprite, then poll for up to 10 s until its PNG has the new pixels;
  - check that the other sprite kept its mtime.
- [x] 4.3 Save by rename: write `watch.json.tmp` and rename it over the worksheet, and check that the change is rendered. Save once more and check that it is rendered too.
- [x] 4.4 Error while watching:
  - save a syntax error, and check that stderr shows the line and column, the outputs are unchanged and the process is still running;
  - fix it, and check that it renders again.
- [x] 4.5 Missing worksheet at start: the exit code is 2.
- [x] 4.7 Idle: after the render at start, no further render happens while nobody touches the file (regression test for access events, which each render causes by reading the worksheet).
- [x] 4.6 On Unix: send SIGINT with `kill -INT` and check that the exit code is 0 and `manifest.json` lists only files that exist.

## 5. Documentation

- [x] 5.1 README:
  - add a `### Watch` section: usage, the same options as `render`, the summary line, errors keeping the last output, Ctrl+C, and pairing it with a tool that reloads changed files;
  - mention that identical files are no longer rewritten;
  - note that changes may be missed on network filesystems.
- [x] 5.2 Run `cargo test`, `cargo clippy --all-targets` and `cargo fmt --check`. Try `watch` by hand on `examples/campfire.yaml`, with an editor save and with a deliberate syntax error.
