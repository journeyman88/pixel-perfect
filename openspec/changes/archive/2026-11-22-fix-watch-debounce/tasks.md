## 1. Watch loop

- [x] 1.1 Change `watch::run` in `src/watch.rs` to take `quiet` and `max_wait`, and wait with `recv_timeout(min(quiet, deadline - now))`:
  - every change restarts the quiet interval, but not the deadline;
  - it renders when the quiet interval passes or the deadline is reached;
  - a `Stop` while waiting returns at once.

  Update its documentation.
- [x] 1.2 Unit tests in `src/watch.rs`, with a 50 ms quiet interval and a 300 ms maximum wait:
  - a burst of signals 10 ms apart gives one render;
  - signals 20 ms apart for 1 s give a render at least every 300 ms, and one after the last;
  - a `Stop` during the quiet interval gives no render;
  - a change sent during a render gives one more render.

  Update the existing tests to the new signature.

## 2. Watcher

- [x] 2.1 In `src/main.rs`, replace `new_debouncer` with `notify::recommended_watcher` on the worksheet's directory, keeping the filter on access events and on the file name. Pass `QUIET = 150 ms` and `MAX_WAIT = 2 s` to `run`.
- [x] 2.2 Remove `notify-debouncer-full` from `Cargo.toml`.

## 3. End-to-end tests (`tests/watch.rs`)

- [x] 3.1 3 saves 40 ms apart give exactly one more render; 10 saves 100 ms apart also give exactly one.
- [x] 3.2 Saves every 50 ms for 5 s give at least 2 renders while they go on, and the last output matches the last save.
- [x] 3.3 The existing tests still pass: editing, saving by rename, errors, a missing worksheet, the idle check and SIGINT.

## 4. Check and cleanup

- [x] 4.1 Measure with the burst and latency scripts in the scratchpad: one render per burst, a single-save latency close to 176 ms, and renders during continuous saves.
- [x] 4.2 Run `cargo test`, `cargo clippy --all-targets` and `cargo fmt --check`. Delete `watch-debounce.patch` from the repository root.
