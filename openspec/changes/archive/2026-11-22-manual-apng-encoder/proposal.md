## Why

After the earlier optimizations, writing APNG files is the main remaining
bottleneck: about 70% of the wall time for `train` and half for `campfire`.
The `png` crate's writer compresses the frames one after another, on a single
thread, while the other threads wait. When the encoder tries both ways of
writing a frame, it also compresses the winner a third time.

An experiment showed the way out. The writer compresses each frame with an
independent zlib stream, exactly as when that frame is encoded alone as a
PNG with the same settings. If those standalone encodings run in parallel and
the engine writes the APNG chunks itself, every frame is compressed once, on
all cores, and the files stay byte-identical. These were the results with 8
threads, checked on 60 renders, 84 APNG files and 3075 frames, every one
identical:

| Case | Current | Hand-written |
|---|---|---|
| train, PNG | 106 ms | 45 ms |
| merged, PNG | 147 ms | 71 ms |
| campfire, PNG | 50 ms | 30 ms |
| train, `--scale 4` | 422 ms | 261 ms |

## What Changes

- The APNG encoder writes `fcTL`, `IDAT`, `fdAT` and `IEND` itself. The
  header (signature, `IHDR`, `acTL`, `PLTE`, `tRNS`) still comes from the
  `png` crate.
- Each frame's data is the zlib stream of a standalone PNG of that frame,
  computed for all frames in parallel. When both ways of writing a frame
  (changed area and masked) are tried, the smaller standalone PNG is kept and
  its data used as is: one compression fewer per tried frame.
- The written files are byte-identical to those of the current encoder. The
  golden files must not change.
- The chunk order and sequence numbering, which were left to the `png` crate,
  are now stated in the apng-export spec.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `apng-export`: *Lossless and deterministic APNG* states the chunk order,
  the `fcTL`/`fdAT` sequence numbers, the splitting of very large frame data,
  and that each frame's data is a complete zlib stream of its own.

## Impact

- `src/encode.rs`:
  - `apng` is rewritten;
  - `frame_part` and `encoded_len` become one function that returns the
    chosen frame already compressed;
  - the batched preparation with `rayon::join` is removed.
- New direct dependency `crc32fast` for chunk CRCs. It is pure Rust and was
  already an indirect dependency through `png`.
- Tests:
  - the existing golden files are the reference: they were written by the
    current encoder and must stay byte-identical;
  - new unit tests cover the chunk layout and sequence numbers, and the split
    of oversized frame data.
