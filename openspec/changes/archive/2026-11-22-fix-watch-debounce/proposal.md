## Why

`watch` renders once for every save, even when the saves come a few
milliseconds apart. Measured on `tests/fixtures/valid/watch.json`, 3 writes
40 ms apart give 3 renders, and 10 writes 100 ms apart give 10. The spec asks
for one render after the last of them.

The cause is the debouncer. `notify-debouncer-full` delays each event by
150 ms, but it does not wait for a burst to end: writes 40 ms apart reach
`watch` as separate batches. The watch loop only drains what is already
queued when it wakes up. A render takes about 1 ms, so the next batch always
arrives after it, and triggers its own render.

A patch from another agent moved the debounce into the watch loop. That fixed
the bursts, but it showed two problems:
- the latency doubled, from 176 to 326 ms, because two debounces ran one
  after the other;
- a file written without pause never rendered: saves every 100 ms for 3 s
  gave a single render, only after the saves stopped.

This change starts again from the committed code and addresses all three
problems.

## What Changes

- The debounce moves into the watch loop (`src/watch.rs`). After a change,
  the loop waits until no further change arrives for the quiet interval
  (150 ms), then renders once.
- While changes keep arriving with no quiet interval between them, the loop
  renders anyway 2 s after the first change of the burst. A file that never
  stops changing still updates the output at least every 2 s.
- The file watcher is `notify`'s own watcher, used directly.
  `notify-debouncer-full` is removed: its debounce would add to the loop's,
  and its rename tracking is not needed, since `watch` watches the directory
  and filters by file name.
- Changes that arrive during a render start a new debounce after it, as
  today.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `watch-command`: *Render on start and on change* states the quiet interval
  and the maximum wait, with new scenarios for bursts, continuous changes and
  changes during a render.

## Impact

- `src/watch.rs`: `run` takes the quiet interval and the maximum wait, and
  waits with `recv_timeout`.
- `src/main.rs`: `notify::recommended_watcher` replaces
  `notify_debouncer_full::new_debouncer`. Access events are still ignored.
- `Cargo.toml`: `notify-debouncer-full` is removed.
- Tests:
  - unit tests of the loop with a channel and short intervals;
  - end-to-end tests for bursts and for continuous saves.
- `watch-debounce.patch`, the other agent's patch, is no longer needed once
  this change is in.
