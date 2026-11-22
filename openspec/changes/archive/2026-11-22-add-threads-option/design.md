## Context

The rendering and encoding paths (`animate`, `encode`, `webp`, `export`) use
rayon's global pool. Until now the pool was never configured, so rayon
started one thread per logical CPU, unless `RAYON_NUM_THREADS` said
otherwise. Every parallel step collects its results in order, so the output
never depended on the thread count.

These measurements were taken on an AMD Ryzen AI 7 350 (8 cores, 16
threads), with the median of 7 renders:

| Case | 16 threads | 8 threads |
|---|---|---|
| train, PNG | 108 ms, 289 ms CPU, 60 MB | 114 ms, 234 ms CPU, 54 MB |
| train, WebP | 24 ms, 197 ms CPU, 63 MB | 28 ms, 132 ms CPU, 58 MB |
| merged, PNG | 153 ms, 447 ms CPU, 61 MB | 157 ms, 345 ms CPU, 55 MB |
| train, `--scale 4` | 452 ms, 2034 ms CPU, 634 MB | 439 ms, 1321 ms CPU, 589 MB |

## Goals / Non-Goals

**Goals:**
- A default that keeps the wall time and saves CPU time and memory.
- An explicit override on `render` and `watch`.

**Non-Goals:**
- Choosing a different thread count for each phase.
- Exposing the setting in the library API. Library users keep rayon's own
  defaults and can configure the global pool themselves.

## Decisions

### 1. Default: the physical cores, capped by the usable CPUs

The default is `num_cpus::get_physical()`, clamped between 1 and
`std::thread::available_parallelism()`. The standard library only counts
logical CPUs. The cap keeps the default within the limits of containers and
CPU affinity, which `available_parallelism` honors and the physical count
does not.

*Alternative considered:* rayon's own default, one thread per logical CPU.
It gives the same wall time for PNG output and a few milliseconds less for
WebP, at 20–35% more CPU time. Rejected.

### 2. Precedence: `--threads`, then `RAYON_NUM_THREADS`, then the default

The pool is built once, in `main`, before the command runs. An explicit
option wins. The environment variable keeps working for anyone who already
uses it with rayon.

### 3. The option lives in `RenderArgs`

`render` and `watch` share their options through `RenderArgs`, so both get
`--threads` without any code duplication. `validate` and `import` do not take
the option and use the default. `import` renders to validate its result.

## Risks / Trade-offs

- [The physical core count can be wrong on unusual systems] → It is capped by
  the usable CPUs and can always be overridden with `--threads` or
  `RAYON_NUM_THREADS`.
- [WebP output is slightly slower than with every logical thread: about 4 ms
  on `train`] → It is accepted for the CPU and memory saved, and
  `--threads 16` gets the old behavior back.
