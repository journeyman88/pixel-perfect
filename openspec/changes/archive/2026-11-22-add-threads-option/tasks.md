## 1. Implementation

- [x] 1.1 Add `--threads <N>` to `RenderArgs` in `src/main.rs`, accepting 1 to 65535.
- [x] 1.2 Configure the global rayon pool at startup: `--threads`, then `RAYON_NUM_THREADS`, then the physical cores capped by `available_parallelism`. Add the `num_cpus` dependency.

## 2. Tests

- [x] 2.1 `tests/cli.rs`: the output is byte-identical with `--threads 1` and `--threads 4`, for PNG, WebP and `--single-frames`.
- [x] 2.2 `tests/cli.rs`: `--threads 0` is a usage error with exit code 2.
- [x] 2.3 Check by hand that the process runs one thread per worker plus the main thread, for the default, `--threads`, `RAYON_NUM_THREADS`, and both set together.

## 3. Documentation

- [x] 3.1 README: add `--threads` to the options table, with a note on the default and the precedence.
