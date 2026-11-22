## Context

`src/main.rs` installs a `ctrlc` handler. The first call sends
`Signal::Stop` to the watch loop, which ends after the render in progress.
Later calls run `std::process::exit(130)`. Images are written in place, and
`manifest.json` is written last.

## Goals / Non-Goals

**Goals:**
- The spec describes the behavior that already exists, including what a
  second interrupt can leave on disk.

**Non-Goals:**
- Changing the behavior, for example making the forced exit leave a
  consistent output. That would need writing to temporary files and
  renaming them, which was rejected in `add-watch-command` because tools
  watching the files expect modifications in place.

## Decisions

### 1. Document the inconsistency instead of preventing it

The second interrupt is an escape hatch for long renders: it is meant to stop
at once. A clean stop is still available with a single interrupt. Rendering
again repairs the output, since identical files are skipped and the others
are rewritten.

### 2. Exit code 130

130 is the usual exit code of a process ended by SIGINT (128 + 2). Scripts
can tell a forced stop from a clean one (0).

## Risks / Trade-offs

- [A tool can read the incomplete image or the stale manifest] → This only
  happens after the user forces the stop. The spec says how to recover.
