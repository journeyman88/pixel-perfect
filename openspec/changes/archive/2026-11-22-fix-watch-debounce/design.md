## Context

`watch` (in `src/main.rs`) runs `notify-debouncer-full` with a 150 ms
timeout on the worksheet's directory. Every batch with a non-access event
for the worksheet's file name sends `Signal::Changed` to the loop
`watch::run` (in `src/watch.rs`). `run` blocks on `recv`, drains the signals
already queued with `try_recv`, and renders, unless a `Signal::Stop` is
among them.

The debouncer delays each event by its timeout, but it does not wait for a
burst to end, so writes 40 ms apart arrive as separate batches. These are the
measurements on `tests/fixtures/valid/watch.json`:

| | Committed code | Patch (`watch-debounce.patch`) |
|---|---|---|
| 3 writes, 40 ms apart | 3 renders | 1 render |
| 10 writes, 100 ms apart | 10 renders | 1 render |
| Single save, until the render is done | 176 ms | 326 ms |
| Saves every 100 ms for 3 s | 30 renders, the first after 201 ms | 1 render, after the saves stop |

## Goals / Non-Goals

**Goals:**
- One render per burst of changes, after the burst.
- The latency of a single save about the same as today: the quiet interval
  plus the render.
- A file that never stops changing still updates the output at least every
  2 s.
- The timing logic testable without a real watcher, as `run` is today.

**Non-Goals:**
- Command-line options for the two intervals. `watch` is a separate command
  so such options can be added later, without touching `render`.
- Incremental rendering.

## Decisions

### 1. Trailing debounce in the loop, with a maximum wait

`run(signals, quiet, max_wait, session)` works like this:
- it renders at start;
- it blocks on `recv` until the first change;
- it then waits with `recv_timeout(min(quiet, deadline - now))`, where
  `deadline` is the first change plus `max_wait`;
- every change restarts the quiet interval, never the deadline;
- it renders when the quiet interval passes or the deadline is reached,
  whichever comes first;
- a `Stop` received while waiting returns at once, and a closed channel ends
  the wait.

Signals that arrive during a render stay in the channel. The next `recv`
returns them, and a new debounce starts.

*Alternative considered:* a fixed window opened by the first change,
discarding the others until it closes. It is simpler, and it handles changes
that never pause without a separate limit. But a burst longer than the
window gives several renders, possibly of a half-written file. The trailing
debounce with a maximum wait was chosen.

*Alternative considered:* keeping `notify-debouncer-full` with a short
timeout. It still adds a second delay, and a second mechanism to reason
about, for no benefit. Rejected.

### 2. `notify`'s watcher directly

`notify::recommended_watcher` watches the directory, non-recursively. Its
callback keeps the current filter: it ignores access events, which every
render causes by reading the worksheet, and events for other file names. It
then sends `Signal::Changed`. Saves made by renaming a temporary file show up
as create or rename events carrying the worksheet's name, so they are still
detected. `notify-debouncer-full` is removed from `Cargo.toml`.

### 3. Values

The quiet interval is 150 ms, the same as the current debounce: it is long
enough for an editor's save, which writes or renames within a few
milliseconds, and short for a person. The maximum wait is 2 s. It only
matters for files written continuously, less than 150 ms apart, such as
scripts or sync tools; editors do not do this. Two seconds keeps the output
at most 2 s behind without rendering every intermediate state. Both are
constants in `src/main.rs`, next to where `run` is called.

## Risks / Trade-offs

- [Raw events are many more than debounced batches] → Each one is a cheap
  channel send, and the loop collapses them.
- [Timing-based unit tests can be flaky] → They use short intervals with
  wide margins, such as a 50 ms quiet interval and signals 10 ms apart, and
  assert only what the timing guarantees.
- [Platform differences in raw `notify` events] → The end-to-end tests,
  including saving by rename, cover them. On network file systems the
  existing note in the README still applies.
