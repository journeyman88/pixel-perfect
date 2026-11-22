## Why

Rendering and encoding run on a rayon thread pool. By default rayon starts
one thread per logical CPU. On CPUs with two hardware threads per core this
was measured, on an 8-core, 16-thread machine, to give no faster PNG output
than one thread per physical core, while costing 20–35% more CPU time and
more memory. Users also need a way to choose the thread count: to leave cores
free for an editor or game engine while `watch` runs, or to compare runs.

## What Changes

- The default number of worker threads is the number of physical CPU cores,
  capped by the CPUs the process may use.
- `render` and `watch` gain `--threads <N>`, an integer from 1 to 65535, to
  override the default. Without it, the `RAYON_NUM_THREADS` environment
  variable is honored when set.
- The output does not depend on the thread count: it is byte-identical for
  any value.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `png-export`: *Render command* lists and describes `--threads`.
- `watch-command`: *Watch command* lists `--threads` among the options shared
  with `render`.

## Impact

- `src/main.rs`:
  - `--threads` in the shared `RenderArgs`;
  - the global rayon pool is configured at startup, for every command.
- New dependency `num_cpus`, pure Rust, to count the physical cores.
- `tests/cli.rs`: the output is identical with 1 and 4 threads, and
  `--threads 0` is a usage error.
- `README.md`: the option and the default.
