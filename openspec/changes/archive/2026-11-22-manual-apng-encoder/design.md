## Context

`encode::apng(frames)` has three steps today:
- it picks one encoding for all frames with `choose_all`;
- it opens an animated `png::Writer`;
- it writes the frames in batches of one frame per thread.

For each frame, `frame_part` picks the rectangle and, when masking is
allowed, compresses both candidates (`encoded_len`) to keep the smaller one.
The prepared batches overlap with the writer through `rayon::join`, but the
writer itself compresses on one thread.

In the `png` crate 0.18, `Writer::write_image_data` filters the rows of the
frame's rectangle, starting from a row of zeros, with the configured filter.
It compresses them with a fresh `flate2` zlib encoder at the configured level,
writes the `fcTL`, and then the stream as `IDAT` for the first image or as
`fdAT` chunks numbered from the `fcTL`'s sequence number plus one.
`write_header` writes the signature, `IHDR`, `acTL`, `PLTE` and `tRNS`.
Dropping a writer that was not finished appends `IEND`.

An experiment in a separate worktree implemented the design below. It
produced byte-identical files on every example and valid fixture, at scales 1
and 3 and with `--safe-build`, and the whole test suite passed with it.

## Goals / Non-Goals

**Goals:**
- Compress every APNG frame exactly once, in parallel.
- Keep the files byte-identical. The golden files are the reference.

**Non-Goals:**
- Changing the compression level, the filters or the frame selection. Any of
  these would change the bytes.
- Replacing the `png` crate for still PNG images, or for the APNG header.
- Animated WebP. Its encoder is already parallel per frame.

## Decisions

### 1. Frame data from standalone PNGs

Each chosen frame part is encoded as a standalone PNG with the same
`configure(encoder, encoding)` as the animated writer: same color type, bit
depth, palette, `tRNS`, compression and filter. Its `IDAT` payloads,
concatenated, are that frame's zlib stream. This is the same function the
trial compressions already use, so a tried frame keeps the winner's
standalone PNG and is not compressed again.

*Alternative considered:* calling `flate2` and the PNG filters directly. It
would skip a little overhead (the `IHDR` and the copy), but it would have to
reproduce the `png` crate's adaptive filter selection exactly. Through the
crate's own encoder that is guaranteed. Rejected.

### 2. The header comes from the `png` crate

An animated `png::Encoder` configured as today is asked for its header, and
the writer is then dropped. Its trailing `IEND`, 12 fixed bytes, is checked
and removed. This keeps every header detail, such as chunk order and `PLTE`
and `tRNS` contents, identical by construction.

### 3. Chunks written by hand

`write_chunk(out, kind, data)` writes the length, the type, the data and the
CRC-32 over the type and data, using `crc32fast`. Frames are written in order:
- an `fcTL` with the next sequence number, holding the rectangle, the delay,
  `dispose_op` NONE and the blend;
- the first frame's data as `IDAT`;
- every other frame's data as `fdAT`, each prefixed by the next sequence
  number.

Data larger than the largest chunk payload is split as the `png` crate does:
2^31 − 1 bytes for `IDAT`, 2^31 − 5 for `fdAT`. No real render reaches these
sizes, but a unit test of the splitting function covers them.

### 4. All frames in one parallel pass

Every frame's part is computed and compressed in one `par_iter` over all the
frames, and the results are collected in order. Only the compressed data is
kept, so memory grows by the size of the output file and not by raw frames.
The batches and the `rayon::join` pipeline, which existed only to overlap the
writer's sequential compression, are removed.

## Risks / Trade-offs

- [A future `png` crate could compress frames differently in its animated
  writer than in its still writer] → `Cargo.lock` pins the version, and the
  golden byte comparisons fail at once if the bytes change. Upgrading `png`
  then means checking the goldens, as with any other encoder change.
- [Hand-written chunks could be malformed] → The golden files compare every
  byte. A unit test checks the chunk layout and sequence numbers, and the
  existing decoding tests check every frame.

## Measurements

These are the results after implementation. The previous build is `f125f0c`
and the new one has the hand-written encoder. Both were built in release
mode and run with the default 8 threads on an AMD Ryzen AI 7 350 (8 cores,
16 threads). Each figure is the median of 7 renders.

| Case | Before | After | Speed-up | Peak memory |
|---|---|---|---|---|
| train, PNG | 106 ms, 204 ms CPU | 45 ms, 196 ms CPU | 2.4× | 53 → 54 MB |
| merged, PNG | 150 ms, 316 ms CPU | 69 ms, 288 ms CPU | 2.2× | 57 → 56 MB |
| campfire, PNG | 56 ms, 141 ms CPU | 32 ms, 125 ms CPU | 1.7× | 25 → 25 MB |
| campfire, `--scale 4` | 141 ms | 89 ms | 1.6× | 146 → 132 MB |
| train, `--scale 4` | 414 ms | 265 ms | 1.6× | 586 → 553 MB |

WebP and `--single-frames` output do not go through the APNG encoder, and
they are unchanged within noise.

The new files are byte-identical to the old ones in all 12 benchmarked cases
and in 60 further renders. Those renders cover every example and valid
fixture, at scales 1 and 3 and with `--safe-build`, for 84 APNG files and
3075 frames. The golden files are unchanged.
