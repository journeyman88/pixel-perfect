## Context

Animated resources go through two phases.

- **Planning.** `plan_animation` in `src/export.rs` builds an `Animation`. It
  bounds the frame count by `--max-image-size`, then checks the format's own
  timing limits: for WebP the frame bound in milliseconds, for APNG
  `delay_overflow`.
- **Writing.** `animation_bytes` encodes either `Animation::frames()` (APNG: ticks,
  merged only while the merged delay still fits an APNG delay) or
  `Animation::ms_frames()` (WebP: rounded to milliseconds, 0 ms segments
  dropped).

Both lists come from `Animation::merged`. It renders the frame at each change
tick and merges consecutive identical images, using a merge rule that depends
on the format.

The manifest (`stable-manifest`) has `ManifestEntry { id, kind, path, width,
height, frame_count, loop_seconds }` and `EntryKind { Static, Animation }`,
with entries sorted by `id`. The schema already defines `frameCollection` as
`frames: [{ index, path, duration }]` plus `loop`, with no `path` and no
`frameCount`.

## Goals / Non-Goals

**Goals:**
- `--single-frames` writes animated resources as frame collections, in the
  layout and manifest form that the schema already defines.
- Durations are exact: rational seconds, and their sum is the loop length.
- Output stays deterministic, and there are no new dependencies.

**Non-Goals:**
- Sprite sheets, atlases or any packing. That is left to third-party tools.
- Streaming frames to disk to save memory. Frames are rendered into memory as
  they are today.
- Deleting stale frame files from earlier renders. This follows the existing
  rule that files are never deleted.
- Frame names chosen by the user.

## Decisions

### 1. A third frame list: exact durations, unbounded merging

Add `Animation::exact_frames() -> Vec<(Image, u64)>`. It is
`merged(ticks, segment lengths, |a, b| Some(a + b))`: the same frames as
`frames()`, but with no APNG cap on the merged delay. The manifest gets
`duration = time::reduced(ticks, clock)`. The ticks of all segments add up to
`loop_len`, so the durations add up to `loop` exactly. No new arithmetic is
needed.

*Alternative considered:* reuse `frames()`. Its merging stops at the APNG delay
limit, so a long hold would be split into identical frames that do not need to
be separate. That is a constraint of the APNG container and has no reason to
leak into a mode that writes no container. Rejected.

### 2. The kind of each output is decided at planning

`PlannedOutput` gains `kind: EntryKind`, where `EntryKind` now has `Static`,
`Animation` and `FrameCollection`. Planning sets it from `is_animated` and
`options.single_frames`. For a frame collection, `path` holds the frames
directory, `<collection>/<name>`, without an extension.
`ManifestEntry::new` copies `kind` from the output instead of guessing it from
`loop_seconds`.

*Alternative considered:* a `Layout::File | Layout::Frames` enum in place of
`path`. It is more precise, but it touches every place that reads
`outputs[i].path` today, which is mostly tests, for little gain. Rejected.

### 3. Planning drops the container timing checks in this mode

`plan_animation` keeps the clock and loop checks and the frame bound by size,
so `--max-image-size` keeps its documented meaning: the frames before merging
times the size of one frame. It skips the WebP millisecond bound and
`delay_overflow` when `single_frames` is set, because those limits come from
the container fields, which are not written in this mode.

### 4. Each frame is written as a static image

`write()` handles a `FrameCollection` output like this:
- it calls `exact_frames()` and upscales each frame;
- for PNG, it chooses the encoding once with `encode::choose_all` over all the
  frames, as APNG does, and encodes every frame with that encoding
  (`encode::png_with`). Every frame then has the same color type and, when
  indexed, the same `PLTE`, `tRNS` and bit depth: one color space for the whole
  collection, which is what third-party tools expect when they import the
  frames as one sequence or atlas. Choosing per frame would give smaller
  files, but the palettes and color types would differ from frame to frame.
  The colors are counted on the frames before `--scale`, since upscaling does
  not add colors;
- for WebP, it encodes each frame with `webp::still`. Lossless WebP always
  decodes to RGBA, so the frames already share a color space;
- it writes frame `i` to `<dir>/<i>.<ext>`, with `i` zero-padded to the width
  described below;
- it fills `entry.frames` in playback order.

File names are the frame index, zero-padded to
`width = max(4, number of digits of (frame count − 1))`. For example, `0000` to
`0011` for 12 frames, and `00000` to `12344` for 12345 frames. Because every
name in a collection has the same width, sorting the names alphabetically gives
playback order, which is what many tools that import image sequences expect.
The width is fixed at 4 up to 10000 frames, so names only change for very long
animations. The width depends on the frame count after merging, which is known
only once the frames have been rendered, so it is computed in `write()` and not
during planning. The manifest stays the authoritative list of frames.

*Alternative considered:* no padding. Names are then stable at any count, but
alphabetical order breaks from 10 frames on (`10` sorts before `2`). Rejected
at the user's request.

### 5. Manifest types

- `EntryKind` gains `FrameCollection`, serialized as `"frameCollection"`.
- `ManifestEntry.path` becomes `Option<String>`, skipped when it is `None`.
- `ManifestEntry` gains `frames: Option<Vec<ManifestFrame { index, path,
  duration }>>`.
- Field order is `id, kind, path, width, height, frameCount, frames, loop`.
- `ManifestOptions` gains `single_frames`. It is always serialized, following
  the `stable-manifest` rule that the engine writes every option it supports.

Every existing golden manifest gains `"singleFrames": false`.

### 6. File-safe names already cover directories

The frames directory uses the same `<name>` as the file name would: the id, or
its hash with `--safe-build`. The existing checks therefore apply unchanged:
portable names and case collisions without safe build, hash collisions with
it. Ids are unique within a collection, so a directory `panels/a/` can never
coexist with a file `panels/a.png` written by the same render.

## Risks / Trade-offs

- [A long animation can produce thousands of small files] → The count is
  already bounded by `--max-image-size`, and merging keeps only the distinct
  frames. Nothing more is added. The README warns about it.
- [Stale frames: re-rendering an animation that has fewer frames leaves the old
  higher-numbered files on disk] → This is consistent with the "never delete"
  rule. The manifest lists the authoritative frames, and the README says to
  read them from the manifest, not from the directory listing.
- [Golden churn: every manifest changes] → It is a single field. Regenerate the
  golden files with `UPDATE_GOLDEN` and check that only `manifest.json` files
  change among the existing golden directories.
