## Context

`add-worksheet-loading` produces a `ResolvedWorksheet`: resolved palettes
(`char` → 8-bit `Rgba`) and resolved sprites (rows of `char`s plus a palette
id). Nothing is rendered or written yet. The CLI has only `validate`, with exit
codes 0 (valid), 1 (invalid) and 2 (usage, IO, unsupported extension).

This change adds the first output. The pieces it introduces (image buffer,
transforms, PNG encoder) are reused unchanged by `add-composition` and
`add-animation`.

## Goals / Non-Goals

**Goals:**
- Pixel-exact sprite images and transform operations with fully specified
  arithmetic.
- An encoder that takes any final RGBA image and picks indexed or RGBA from its
  real color count.
- Byte-identical output for identical input.

**Non-Goals:**
- Panels, scenes, flipbooks, timelines (later changes).
- APNG and WebP.
- Other PNG optimizations (grayscale color types, RGB without alpha, filter
  heuristics, zopfli). They can be added later without changing the output
  contract, except for the exact bytes.

## Decisions

### Image buffer
`raster::Image { width: u32, height: u32, pixels: Vec<Rgba> }` in row-major
order. Every constructor and operation keeps the canonical-transparency
invariant (alpha 0 ⇒ `(0, 0, 0, 0)`), so the color count never sees two
"different" invisible colors.

### Transforms as pure functions
`raster::apply(&Image, &Transform) -> Image` and `apply_all`. Sizes are
validated at load time for declared chains (`crop` bounds), so `apply` can
assume valid input and panic on a broken invariant, not return an error. Run-time
chains (`appendTransforms`) get their own check in `add-animation`.

### Color count on the final image
The encoder receives the final image (after composition and `--scale`; scaling
does not change the set of colors) and counts distinct `Rgba` values, stopping
as soon as it passes 256. Counting characters would be correct for sprites
alone, but wrong as soon as compositing blends semi-transparent pixels into new
colors. Counting pixels is correct in every case and costs one pass.

### Indexed palette order
Non-opaque colors come first so `tRNS` can stop after them. Within each group,
order of first appearance in row-major order. This is deterministic and needs no
sorting. Bit depth is the smallest of 1/2/4/8 holding the palette size.

### PNG encoding
The `png` crate with fixed settings: `Compression::Balanced` (the crate
default, written explicitly) and a fixed adaptive filter. No `tIME`, `tEXt`,
`gAMA` or `sRGB` chunks. Byte-identity is guaranteed for a given engine build.
`Cargo.lock` pins the encoder, so a dependency update may change bytes but
never pixels.

### Output layout and CLI
- All flags are kebab-case (the CLI convention and `clap`'s default).
- `--output-path <dir>`, with short alias `-o`. Default is the current
  directory.
- `--format png`: a `clap::ValueEnum` with one value for now, so adding `webp`
  later does not change the CLI.
- `--scale <N>`: 1 to 64. The upper bound guards against accidental huge
  images; it is a usage error (exit 2) when outside the range.
- Paths: `<output-path>/<collection>/<id>.<format>`, or
  `<output-path>/<collection>/<hash>.<format>` with `--safe-build`.

### Safe build: hash of the id
Default file names are the ids, which is readable but fails for ids that are
not portable file names. `--safe-build` replaces every name with a hash, so that
all names follow one rule and no id needs special treatment.

- **Input:** the UTF-8 bytes of the id only. The name stays the same while the
  sprite is edited. A content hash would rename the file on every edit, which is
  a different feature (cache busting).
- **Algorithm:** BLAKE2b with an 8-byte digest length parameter (unkeyed). This
  is not BLAKE2b-512 truncated: the digest length is part of the parameter
  block, so the values differ. For example, `hero` gives `8737b11b474b218b`,
  while the first 16 hex digits of BLAKE2b-512 give `6b4e87bc8d043325`. Anyone
  can recompute a name with `printf '%s' hero | b2sum -l 64`. No security is
  needed; BLAKE2b is fast, standard and pure Rust (`blake2` crate, `Blake2b<U8>`).
- **Collisions:** 64 bits make a collision practically impossible, but it is
  still checked per collection, and a collision is a plan error.

### Manifest
`manifest.json` is written on every successful render, after all images, so a
manifest never lists files that were not written. It is the authoritative list
of current outputs. Old files are not deleted (with hashed names, an orphan
cannot be recognized without a previous manifest); cleanup can be added later
using the old manifest. Content: `format`, `scale`, `safeBuild`, and `files`
keyed by export pointer with `path` (relative, `/` separators), `width`,
`height`. Keys are sorted (`BTreeMap`) and the JSON is pretty-printed with a
trailing newline, for byte-identical output. Animated formats will add frame
data to the entries later.

### Plan, then write
Rendering runs in two phases. First it plans every output (selection, id
checks, path collisions, unsupported kinds) and collects all errors. Only if the
plan is clean does it render and write. An invalid request therefore never
leaves a half-written output directory. IO failures while writing are exit
code 2.

### File-safe ids
Only checked without `--safe-build`, and the error suggests the flag. The forbidden set is the union of the POSIX and Windows rules (separators,
reserved punctuation, control characters, `.`/`..`, trailing dot or space),
plus case-insensitive uniqueness per collection. Windows reserved device names
(`CON`, `NUL`, ...) are not checked. They are rare as sprite ids, and checking
them can be added later.

## Risks / Trade-offs

- **Ids that were valid now fail at render time.** `validate` keeps accepting
  them, because the schema allows any non-empty id. They fail only when
  exported. This is acceptable: the ids are fine as references, just not as
  file names.
- **Byte-identity across `png`/`flate2` versions** is not guaranteed. The
  contract is "same engine build". Golden-file tests will flag version bumps
  that change bytes.
- **Large `scale` × large panels** (later) could use a lot of memory. The 1–64
  bound and future size checks limit it.
