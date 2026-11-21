## Why

Many game engines and packing tools do not read APNG or animated WebP. They
expect one image per frame, together with the timing, and build the sprite
sheet or atlas themselves. The manifest schema (`schema/manifest.schema.json`)
already reserves `kind: frameCollection` and `options.singleFrames` for this.
Adding the mode is what lets pixel-perfect stay a pure renderer and leave
packing to third-party tools.

## What Changes

- `render` gains a `--single-frames` flag. When it is set, every resource that
  would otherwise be written as one animated image (APNG or animated WebP) is
  written instead as a set of still images, one per frame. Each image is a full
  frame of the animation's canvas, not a difference from the previous frame.
  Static resources are not affected.
- Frames are the distinct frames of one loop. Consecutive identical frames are
  merged, and each frame keeps its exact duration as a fraction of a second.
  The limits on APNG delays and on WebP milliseconds do not apply, because no
  animated container is written.
- Frame files go to `<output-path>/<collection>/<name>/<index>.<format>`.
  `<name>` is the id, or its hash with `--safe-build`. `<index>` starts at 0
  and is zero-padded to 4 digits, or to the width of the last index when an
  animation has more than 10000 frames, so the names sort in playback order.
  Each frame is encoded the way a static image is in the chosen `--format`.
- Manifest:
  - Entries for these resources have `kind` `frameCollection`, with `frames`
    (`index`, `path`, `duration`) and `loop`, as the schema defines.
  - `options.singleFrames` is now always written, `true` or `false`. This
    follows the rule that the engine writes every option it supports, and it
    changes every manifest by one field.

## Capabilities

### New Capabilities

- `single-frame-export`: the `--single-frames` mode. It covers which resources
  it applies to, how frames are selected and merged, their exact durations, the
  frame file layout and the encoding of each frame.

### Modified Capabilities

- `png-export`:
  - *Render command*: accepts `--single-frames`.
  - *Output paths*: animated resources go to a directory of frames when
    `--single-frames` is set.
  - *Manifest*: `singleFrames` is always present in `options`, and
    `frameCollection` entries are written when the mode is on.
  - *Export selection*: refers to single-frame output for animated resources.

## Impact

- `src/main.rs`: the new flag.
- `RenderOptions` gains `single_frames`. Every literal construction in the
  tests must be updated (about 20 sites in `tests/`).
- `src/animate.rs`: a frame list with exact durations, where merging has no
  delay cap.
- `src/export.rs`:
  - the planning step skips the APNG and WebP timing checks in this mode;
  - frame paths are planned;
  - frames are written;
  - the manifest gets `frameCollection` entries and `singleFrames`, and
    `ManifestEntry.path` becomes optional.
- Golden files: every existing `manifest.json` gains `"singleFrames": false`.
  New golden directories cover single-frame PNG and WebP output.
- `README.md`: document the flag.
- No new dependencies.
