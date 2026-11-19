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
  - `moveTo`: linear interpolation, rounded to the nearest pixel at every
    frame-unit (rounding rule to be fixed in the specs);
  - `setVisible`, `moveToZ`;
  - `appendTransforms`, `resetTransforms`, including run-time bounds checks for
    `crop` on extended chains.
  Flipbook elements inside a timeline play at their own `fps`.
- Scenes with timeline layers: each layer loops on its own length.
- Common time base. The frame clock is the least common multiple of the `fps`
  values involved; the loop length is the least common multiple of the loop
  durations. Consecutive identical frames are merged into one longer frame.
- APNG export with exact frame delays (num/den fractions). The `render` command
  writes animated resources as APNG.

Open points to settle in the specs:
- What to do when the common time base does not fit APNG's 16-bit delay
  fields, or when the scene loop becomes unreasonably long (error, cap or
  approximation).
- Whether transitions with `start`/`at` beyond the timeline `duration` are
  errors or ignored.
- Loop count (always infinite, or a CLI option).

## Capabilities

### New Capabilities
- `animation`: flipbook and timeline evaluation, the time model and frame merging.
- `apng-export`: APNG encoding and exact delays.

### Modified Capabilities
- `composition`: static fallback rules extended to the animated case.
- `png-export`: the `render` command chooses APNG for animated resources.

## Impact

- New time-model and frame-sequencing modules on top of the compositing module.
- Uses the APNG support of the existing `png` dependency; nothing new.
