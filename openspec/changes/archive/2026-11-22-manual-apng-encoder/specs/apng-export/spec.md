## MODIFIED Requirements

### Requirement: Lossless and deterministic APNG
Decoding each frame of a written APNG SHALL give exactly the corresponding
rendered frame, after `--scale`. Rendering the same worksheet with the same
options and the same engine version SHALL produce byte-identical files. The
file SHALL contain only `IHDR`, `acTL`, `PLTE` and `tRNS` when indexed, `fcTL`,
`IDAT`, `fdAT` and `IEND`, in this order:

- the PNG signature, `IHDR`, `acTL`, then `PLTE` and `tRNS` when indexed;
- for each frame, in playback order, its `fcTL`, then its data: in `IDAT`
  chunks for the first frame, in `fdAT` chunks for the others;
- `IEND`.

The `fcTL` and `fdAT` chunks SHALL carry consecutive sequence numbers starting
at 0, in file order. `IDAT` chunks have no sequence number. Each frame's data
SHALL be a complete zlib stream of its own, holding the filtered rows of that
frame's rectangle. It SHALL be split into several chunks only when it exceeds
the largest chunk payload: 2^31 − 1 bytes for `IDAT`, 2^31 − 5 bytes of data
after the sequence number for `fdAT`.

#### Scenario: Round trip
- **WHEN** a written APNG is decoded frame by frame to RGBA8
- **THEN** every frame equals the rendered frame and every delay equals the frame's duration

#### Scenario: Repeated render
- **WHEN** a worksheet with a timeline is rendered twice
- **THEN** the two APNG files are byte-identical

#### Scenario: Single frame
- **WHEN** a timeline's frames all merge into one
- **THEN** the file is still an APNG with one frame whose delay is the whole loop (split as above if its numerator exceeds 65535)

#### Scenario: Chunk order and sequence numbers
- **WHEN** an animation with three frames is written
- **THEN** the chunks after the header are `fcTL` (0), `IDAT`, `fcTL` (1), `fdAT` (2), `fcTL` (3), `fdAT` (4), `IEND`

#### Scenario: Frames are independent zlib streams
- **WHEN** the data of any frame of a written APNG is taken alone
- **THEN** it inflates by itself to that frame's filtered rows

#### Scenario: Same bytes as before
- **WHEN** the golden fixtures are rendered after the encoder is rewritten
- **THEN** every APNG file is byte-identical to the committed golden file

