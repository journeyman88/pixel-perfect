## Context

After `add-composition` the engine renders sprites, static panels and static
scenes to PNG. Flipbooks, timelines and animated panels and scenes are loaded
and validated (references, element ids, `crop` areas on declared chains,
`seekFlipbook` targets) but rejected or skipped at export time.

`compose::panel` and `compose::scene` render one static image; `encode::png`
writes one image; `export::plan` computes every output and the size guard
before anything is rendered. The `png` crate (0.18) already supports APNG
(`acTL`, `fcTL`, `fdAT`).

## Goals / Non-Goals

**Goals:**
- An exact time model: every frame boundary and every delay is a rational
  number computed in integer arithmetic, never a float.
- Flipbooks, timelines, animated panels and animated scenes exported as APNG
  through the existing pipeline (output paths, safe build, manifest, scale,
  size guard, deterministic bytes).
- Every error that can be found without rendering pixels is found before
  anything is written.

**Non-Goals:**
- Easing curves other than linear for `moveTo`.
- A finite loop count (`--loop-count`): every animation loops forever.
- Frame-difference optimization (sub-rectangle frames, `APNG_DISPOSE_OP_PREVIOUS`).
  Every frame is a full canvas; this can be added later without changing the
  decoded result.
- A static poster frame of animated content, beyond the APNG default image.
- Animated WebP (`add-webp-export`).

## Decisions

### Time is counted in ticks of a common clock
Every animated resource gets a clock `F` (ticks per second): the least common
multiple of the `fps` of every flipbook and timeline it involves, transitively
(a scene involves its timelines, their panels' flipbook elements, and the
flipbook elements of its animated panels). Since `fps ≤ 100`, one frame-unit of
any component is a whole number of ticks: `F / fps`.

Times are `u64` ticks; `F` and loop lengths are computed with checked `u128`
arithmetic. An overflow is a plan error pointing to the resource ("loop too
long"), not a panic.

Alternatives considered: floating-point seconds (rejected: rounding makes frame
boundaries drift and breaks byte-for-byte reproducibility); a fixed clock such
as 1000 Hz (rejected: 1/30 s is not representable).

### Loop lengths
In ticks of `F`:
- **Flipbook:** `sum(frames[].duration) · F / fps`.
- **Timeline:** `duration · F / fps`. Flipbook elements inside a timeline
  restart their phase at every timeline loop (see below), so they do not
  lengthen it.
- **Animated panel:** least common multiple of the loops of its visible
  `_flipbook` elements. Hidden flipbook elements are not drawn and do not count.
- **Animated scene:** least common multiple of the loops of its animated layers
  (timeline layers and animated panel layers). Static layers do not count.

### Flipbook phase
A flipbook played directly (exported, or as an element of an animated panel)
starts at tick 0 of its resource. Inside a timeline its local time restarts at
the start of every timeline loop, and `seekFlipbook` moves it: from frame-unit
`at` onward the flipbook is at the start of `frames[frameIndex]` plus the time
elapsed since `at`. Visibility does not affect the phase: a hidden flipbook keeps
running.

Resetting at every timeline loop is what makes "every loop starts from the
panel's state" true for flipbooks too. It keeps the timeline loop equal to
`duration` and makes `seekFlipbook` behave the same on every loop. The price is
that a flipbook whose cycle does not divide the timeline's `duration` is cut at
the loop boundary, which is visible to the author and fixable with a
`seekFlipbook` or a different `duration`.

Frame lookup is integer: local time `τ` ticks gives frame-unit
`u = (τ · fps / F) mod total`, and the frame is the one whose cumulative
interval `[start, start + duration)` contains `u`.

### Timeline state
The state of every element at frame-unit `t` of a timeline (`0 ≤ t < duration`)
is computed from the panel's state:

- **Instant transitions** (`setVisible`, `moveToZ`, `appendTransforms`,
  `resetTransforms`, `seekFlipbook`) with `at ≤ t` are applied in order of `at`,
  ties broken by array order. The change is visible at frame-unit `at` itself.
- **`moveTo`** gives the coordinates. With `p0` the coordinates at `start`
  (from the previous `moveTo` of the same element, or the panel) and `p1` its
  `coordinates`, for `start ≤ t ≤ start + duration`:

  ```
  offset = round_away((p1 − p0) · (t − start), duration)
  round_away(n, d) = sign(n) · ((2|n| + d) div 2d)
  ```

  computed per axis in `i64`. After `start + duration` the element stays at
  `p1`. Half away from zero makes a movement to the left the mirror image of
  the same movement to the right, which half-up rounding would not.

A `moveTo` with `start + duration = duration` reaches `p1` exactly at the loop
boundary, so `p1` is never shown. This is the intended way to write a seamless
loop that returns to the panel's position.

Positions are evaluated per frame-unit of the timeline, not per tick: inside a
frame-unit the timeline is constant, while flipbook elements may change at
their own rate.

### Validation of timelines happens at load time
New semantic checks in `semantic.rs`, all before rendering:
- `at` and `start` are less than the timeline's `duration`;
  `start + duration` of a `moveTo` is at most the timeline's `duration`.
- Two `moveTo` on the same element must not overlap: for consecutive ones (by
  `start`), `next.start ≥ prev.start + prev.duration`.
- `crop` on extended chains. Each element's chain over time is fully determined
  by its `transforms` and its `appendTransforms` / `resetTransforms` in `at`
  order, so the engine enumerates every chain the element can have and checks
  each `crop` with the existing `check_crops` walk. The error points to the
  `appendTransforms` entry that adds the failing `crop`.

This replaces the "run-time bounds checks" in the proposal: nothing about
chains depends on pixels or on time beyond the ordering of transitions, so
checking them at load time gives earlier errors in `validate` as well.

### Frame segments, not ticks
Rendering every tick would be wasteful (`F` can be thousands per second). The
engine computes the sorted set of **change ticks** in `[0, loop)`: tick 0, every
flipbook frame boundary of every visible-or-not flipbook element (in its local
time), every timeline frame-unit boundary inside a `moveTo` interval, every
`at` of an instant transition, and every loop restart of a nested timeline or
flipbook. Between two consecutive change ticks nothing can change, so one frame
is rendered per segment with duration `next − current` ticks.

Consecutive segments with identical pixels (after scale) are merged into one
frame. Merging compares pixels, not states, so a `setVisible` on an element
that is fully clipped produces no extra frame.

### APNG delays are exact
A frame of `k` ticks lasts `k / F` seconds; the delay is that fraction reduced
by `gcd(k, F)`.

- **The denominator usually fits, but not always.** A boundary of a flipbook
  played directly lies at `n/fps` seconds, so in a single flipbook, animated
  panel or timeline loop a segment lasts `n/fps_a − m/fps_b` seconds, with a
  denominator dividing `fps_a · fps_b ≤ 10000`. Nested clocks break this: inside
  a timeline a flipbook restarts at the loop start and at every `seekFlipbook`,
  so its boundaries lie at `a/fps_timeline + n/fps_flipbook`, and a scene can
  put such a boundary next to one of another layer, giving a three-rate
  denominator (97, 89 and 83 fps give up to 716539). Common rates never get
  there (`lcm(8, 10, 12, 24, 25, 30, 60) = 600`).
- **Segments are checked at plan time.** The plan already enumerates the change
  ticks; if any segment between two consecutive change ticks has a reduced
  denominator above 65535, the plan fails with an error pointing to the
  resource and listing the `fps` values it involves. Nothing is approximated.
- **Merging never creates an overflow.** Two consecutive identical frames are
  merged only if the merged delay still has a denominator ≤ 65535; otherwise
  they stay separate frames with identical pixels, which decodes to the same
  animation.
- If the reduced numerator `n` exceeds 65535 (a long still), the frame is
  written as `n div 65535` frames of `65535/d` followed by one frame of
  `(n mod 65535)/d` (reduced) if non-zero. The decoded animation is identical.

The merging happens before splitting, so splitting never undoes it.

### APNG layout
- `acTL`: `num_frames` = number of written frames, `num_plays` = 0 (infinite).
- The default image (`IDAT`) is frame 0 and part of the animation, so viewers
  without APNG support show the first frame.
- Every `fcTL` covers the full canvas at offset (0, 0), `dispose_op = NONE`,
  `blend_op = SOURCE`. `SOURCE` makes each frame replace the canvas, which is
  required for transparent pixels to be exact.
- Encoding: one color count over all frames. At most 256 distinct colors give
  an indexed APNG with one shared `PLTE`/`tRNS`, ordered as for PNG (non-opaque
  first, then opaque, each by first appearance) scanning frames in order and
  each frame in row-major order. Otherwise RGBA. Bit depth as for PNG.
- Same compression settings as `encode::png`; no other chunks.

### Size guard and frame count
The plan computes, without rendering, the number of segments `S` (before
merging) of each animated export. The guard compares `S × width × height × 4`
(after `--scale`) with `--max-image-size`, because all frames are kept in
memory until the shared palette is known. The enumeration of change ticks stops
as soon as the bound is exceeded, so a huge loop fails fast instead of
allocating. More than 2³¹ − 1 frames (APNG's `num_frames`) is an error even
with `--max-image-size none`.

Alternative considered: two rendering passes (count colors, then encode) with
constant memory. Rejected for now: it doubles rendering time, and the guard
already bounds memory; it can be revisited if large animations become common.

### Exports and manifest
All collections become exportable. Sprites, static panels and static scenes
stay PNG; flipbooks, timelines, animated panels and animated scenes are written
as APNG with the `.png` extension at the usual path, even when merging leaves a
single frame, so the file type depends on the resource and not on its content.
The "not supported yet" error and notice are removed.

Manifest entries of animated resources gain `frames` (written frames, after
merging and splitting) and `loop` (`{ "num": n, "den": d }`, seconds, reduced).
Static entries are unchanged.

### Module layout
- `time.rs`: clock, loop lengths, change ticks, delay fractions and splitting.
- `animate.rs`: flipbook frame lookup, timeline state at a frame-unit,
  `frames(ws, collection, id) -> Vec<(Image, u64)>` (pixels and ticks, merged).
- `compose.rs`: `panel_at` / `scene_at` take a state or time instead of always
  the static state; `panel` and `scene` call them at time 0 of a static
  resource.
- `encode.rs`: `apng(frames, clock)`; `choose` takes an iterator of images so
  PNG and APNG share the palette rule.
- `semantic.rs`: the timeline checks above.
- `export.rs`: the per-resource support check becomes "static → PNG,
  animated → APNG"; the plan computes `S` and the delay denominators; the
  manifest gains `frames` and `loop`.

## Risks / Trade-offs

- **Full-canvas frames make files larger** than an optimizing encoder would.
  Accepted for simplicity and determinism; frame-difference optimization is a
  non-goal that keeps the decoded result unchanged.
- **Coprime `fps` values can make loops very long** (e.g. layers at 7 and 11
  fps with long durations). The size guard and the u128 overflow check turn this
  into a clear error instead of a hang.
- **Unusual `fps` mixes with nested clocks can exceed APNG's 16-bit delay
  denominator** (see "APNG delays are exact"). Rare with common rates; reported
  at plan time with the rates involved, never approximated.
- **Flipbook phase reset cuts cycles** that do not divide the timeline
  `duration`. Documented; it is the price of a predictable timeline loop.
- **All frames are held in memory.** Bounded by `--max-image-size`.
- **Golden files change:** `full.json` now exports the flipbook, the timeline
  and the animated scene, and the manifest gains fields for them.
