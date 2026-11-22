## Context

The CLI (`src/main.rs`, clap derive) has three subcommands: `validate`,
`render` and `import`. The `render` variant lists its options inline and builds
`RenderOptions` from them. Its `render()` helper loads the worksheet, calls
`export::render` and prints diagnostics and notices. The helper returns 0, 1 or
2: 1 for an invalid worksheet or plan, 2 for usage and I/O errors.

Rendering already has two phases. `export::plan` checks everything without
touching the disk, and `export::write` then writes the images, with
`manifest.json` last. A failed plan writes nothing. `write_file` creates the
parent directories and writes the file with `std::fs::write`, so it always
rewrites the file, even when the content is the same.

## Goals / Non-Goals

**Goals:**
- `watch` renders at start and after every change, with exactly the output of
  `render`.
- An error never ends the session and never touches the last good output.
- Only files whose content changes are rewritten, so that tools which reload
  files on change see only real changes.
- `render` and `watch` share their options, so they cannot drift apart.

**Non-Goals:**
- Incremental rendering, that is re-rendering only the resources affected by a
  change. Every change gives a full render.
- Watching several worksheets, or files other than the worksheet. A worksheet
  is self-contained.
- A preview window or a web server.
- Deleting files that are no longer produced. The "never delete" rule stays,
  and the manifest is the authoritative list.

## Decisions

### 1. Shared `RenderArgs`, separate `watch` subcommand

The worksheet path and every render option move into
`#[derive(Args)] struct RenderArgs`, with one method,
`RenderArgs::options() -> RenderOptions`. `Command::Render` holds
`#[command(flatten)] args: RenderArgs`, and so does `Command::Watch`. Options
that only make sense while watching will later be added to `Watch` alone. As
the user asked, the command is a separate subcommand and not a
`render --watch` flag.

### 2. Watch the directory and filter by file name

The watcher is `notify` with `notify-debouncer-full`. The debouncer coalesces
bursts and tracks renames. It is set to 150 ms and watches the worksheet's
parent directory, not recursively. It does not watch the file itself: when an
editor saves by writing a temporary file and renaming it over the original, a
watch on the file would keep following the old, replaced file.

Only events whose path has the worksheet's file name trigger a render. This
filter is essential: the default `--output-path` is `.`, often the worksheet's
own directory, and the render's own writes must not trigger a new render.

Access events (`EventKind::Access`) are ignored too. Every render reads the
worksheet, and on Linux that read is itself reported as an event on the file.
Without this filter each render triggers the next one, and `watch` never goes
idle. This came up while trying the command by hand.

*Alternative considered:* polling the modification time once a second. It has
no dependency and is portable, but adds up to a second of latency and makes
CPU wake-ups on idle. Rejected.

### 3. One loop, renders never overlap

The main thread runs one render at start, then blocks on the debouncer's
channel. When matching events arrive, it does the following:
- it drains everything already queued, so that a burst gives a single render;
- it renders;
- it prints the summary line.

Events that arrive during a render are queued and give exactly one more render
afterwards. A removed worksheet prints `<path>: missing, waiting for it to come
back` and does not render. A creation event renders again.

The interrupt is handled with the `ctrlc` crate (pure Rust, small). The handler
sends a stop message to the same channel, so the loop ends after any render in
progress and the process exits with 0. Renders are therefore never cut short,
and no render leaves a manifest that does not match the files it wrote.

*Alternative considered:* the default SIGINT behavior. It kills the process
with exit code 130, possibly between an image and the manifest. Rejected.

### 4. `write_file`: skip identical files, write the others in place

`write_file(path, bytes) -> Result<Written, RenderError>` works in two steps:
- If `path` exists, has the same length and has the same bytes, it returns
  `Written::Unchanged` without touching the file. The length check avoids
  reading files that obviously differ.
- Otherwise it writes the file in place with `std::fs::write`, as today, and
  returns `Written::Changed`.

The cost of the extra read is small next to rendering and encoding the image.

*Alternative considered:* atomic replacement, writing a temporary file and
renaming it over the target. It is rejected because it moves onto the reader
the very problem that `watch` has with editors. After a rename, a viewer or
engine that watches the file sees a removal and a creation, or keeps following
the old, replaced file. Tools that reload files on change expect a
modification in place, and they usually debounce writes themselves.

### 5. Render report

`export::write` counts the outcomes of `write_file`.

A new `export::render_report(ws, options) -> Result<RenderReport, RenderError>`
returns `RenderReport { plan, manifest, written, unchanged }`.
`export::render` keeps its signature and calls it, so the existing callers and
tests do not change. The CLI uses `render_report` for both commands. `render`
prints nothing new, and `watch` prints:

```
[3] rendered: 1 written, 5 unchanged, 42 ms
[4] failed: 2 diagnostics
```

The number in brackets is the render number, starting at 1 for the render at
start. The format is not a stable interface. The render number replaces a
wall-clock time, which `std` cannot produce in local time without an extra
dependency.

### 6. Exit codes

`watch` follows the `render` conventions, but only at start:
- an unusable worksheet path at start (missing file, unsupported extension) or
  a watcher that cannot start gives exit code 2;
- a worksheet that is invalid at start does not end the command: the
  diagnostics are printed and `watch` keeps waiting for a fix;
- after start, nothing ends the command except the interrupt, which gives exit
  code 0.

## Risks / Trade-offs

- [Filesystems without native change notifications, such as some network
  mounts and WSL paths on Windows drives] → notify falls back to polling on
  some platforms. The README mentions that changes may be missed on such
  filesystems.
- [The watch integration test can be flaky on slow CI] → The watch loop is
  kept as a small function over a channel of events, which unit tests drive
  directly. The single end-to-end test spawns the binary, edits the file and
  polls for the new output for up to 10 s before failing.
- [A tool can read a file while it is being written] → Writes stay in place on
  purpose (see decision 4). Tools that reload files on change debounce writes,
  and identical files are no longer written at all, so in watch mode far fewer
  files are written than before.
- [Two new dependencies, `notify`/`notify-debouncer-full` and `ctrlc`] → All
  three are pure Rust, widely used and maintained. No C dependency is added.
