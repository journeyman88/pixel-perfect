## Why

Flipbooks and timelines are what set the worksheet format apart from text-based
image formats, and APNG is the lossless animated format the project targets
first. Animations mix independent clocks (each flipbook has its own `fps`, each
timeline its own `fps` and `duration`), so the engine needs a precise time model
before it can produce frames whose timing is exact.

## What Changes

- Flipbooks: frames placed on the flipbook canvas for `duration` frame-units at
  the flipbook's `fps`, looping.
- Timelines: start from the panel's state and apply transitions per frame-unit:
  - `moveTo`: linear interpolation in integer arithmetic, halves rounded away
    from zero, so mirrored movements are pixel-exact mirrors;
  - `setVisible`, `moveToZ`;
  - `appendTransforms`, `resetTransforms`; `crop` bounds on the extended chains
    are checked at load time, since every chain is determined by the
    transitions;
  - `seekFlipbook`: at frame-unit `at`, jump a flipbook element to the start of
    frame `frameIndex` (an index into `frames`, not a frame-unit; default 0 =
    restart), then keep playing at the flipbook's `fps`.
  Instant transitions take effect at `at`, ties in array order. Flipbook
  elements inside a timeline play at their own `fps` and restart their phase at
  every timeline loop.
- Load-time checks: `at`/`start` within `duration`, `moveTo` ending at most at
  `duration`, no overlapping `moveTo` on the same element.
- Scenes with timeline layers: each layer loops on its own length.
- Common time base. The frame clock is the least common multiple of the `fps`
  values involved; the loop length is the least common multiple of the loop
  durations. Consecutive identical frames are merged into one longer frame.
- APNG export with exact frame delays (num/den fractions; long stills are split
  into identical frames, never approximated), infinite looping, frame 0 as the
  default image. The `render` command writes flipbooks, timelines and animated
  panels and scenes as APNG.
- `--max-image-size` also bounds the total size of an animation's frames.
- The manifest gives `frames` and `loop` for animated resources.

## Capabilities

### New Capabilities
- `animation`: flipbook and timeline evaluation, the time model and frame merging.
- `apng-export`: APNG encoding and exact delays.

### Modified Capabilities
- `composition`: animated resources are composed frame by frame with the static
  rules.
- `png-export`: every collection becomes exportable, animated resources as APNG;
  the manifest and the size limit cover animations.
- `worksheet-loading`: timeline transitions are checked (ranges, overlapping
  `moveTo`, `seekFlipbook` targets) and `crop` bounds cover chains extended by
  `appendTransforms`.

## Impact

- New time-model and frame-sequencing modules on top of the compositing module.
- Uses the APNG support of the existing `png` dependency; nothing new.
- Golden files and the manifest of `full.json` change: its flipbook, timeline
  and animated scene are now exported.
