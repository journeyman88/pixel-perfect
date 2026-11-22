## 1. Encoder

- [x] 1.1 Add `crc32fast` as a direct dependency.
- [x] 1.2 In `src/encode.rs`:
  - add `write_chunk`, writing the length, type, data and CRC;
  - add `split_chunks`, splitting data at a maximum payload length;
  - add `idat_data`, concatenating the `IDAT` payloads of a PNG.
- [x] 1.3 Turn `encoded_len` into `standalone_png(image, encoding) -> Vec<u8>`, and `frame_part` into a function that returns the chosen rectangle, the blend and the winner's zlib stream.
- [x] 1.4 Rewrite `apng`:
  - take the header from the `png` crate and remove its `IEND`;
  - encode every frame part in one parallel pass;
  - write `fcTL`, `IDAT`/`fdAT` with consecutive sequence numbers, then `IEND`.

  Remove the batches, `rayon::join` and `write_frame`.

## 2. Tests

- [x] 2.1 Run `cargo test` and check that the golden files compare byte-identical. Neither `UPDATE_GOLDEN` nor any change under `tests/golden/` is allowed.
- [x] 2.2 Add a unit test for the chunk order and sequence numbers of a three-frame APNG: `fcTL` 0, `IDAT`, `fcTL` 1, `fdAT` 2, `fcTL` 3, `fdAT` 4, `IEND`. It also checks that every CRC is valid.
- [x] 2.3 Add a unit test that each frame's data inflates on its own to the expected number of filtered-row bytes.
- [x] 2.4 Add a unit test for `split_chunks` at, just below and just above the limit, using a small limit so the test does not need gigabytes.
- [x] 2.5 Render every example and valid fixture at scales 1 and 3 and with `--safe-build`, with the previous build and the new one, and compare every file byte for byte.

## 3. Measurements and cleanup

- [x] 3.1 Benchmark campfire, train and merged as PNG, and at `--scale 4`, against the previous build. Record the wall time, CPU time and peak memory in `design.md` (Measurements), to be copied into the PR description when the PR is opened.
- [x] 3.2 Run `cargo clippy --all-targets` and `cargo fmt --check`. Remove the experiment's worktree from the scratchpad with `git worktree remove`.
