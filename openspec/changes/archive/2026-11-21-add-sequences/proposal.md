## Why

Scenes can stack animated layers, but the layers themselves never move,
appear or change order: everything that changes over time lives inside panels
(timelines) or sprites (flipbooks). Two common effects therefore cannot be
expressed: **parallax**, where whole planes slide at different speeds, and
**effect layers** such as lights, which switch on and off or move above or
below other planes. Doing them today means flattening the planes into one
timeline panel, which loses the scene's layering and its per-layer
flattening.

A scene-level animation, called a *sequence*, gives the scene the same
treatment the timeline gives the panel. The schema blocks are already drafted
in `schema/worksheet.schema.json`; this change specifies them and implements
them in the engine.

## What Changes

- New top-level collection `sequences`. A sequence has `id`, `_scene`, `fps`,
  `duration` and `actions`, and optionally a `description`. It starts from
  the scene's state and applies its actions to the scene's layers over
  `duration` frame-units, looping.
- Three action kinds target a layer by `_layer`: `moveTo` (linear movement,
  for parallax), `setVisible` and `moveToZ` (instant, for effect layers).
  They follow the timing and rounding rules of the timeline transitions with
  the same names.
- Scene layers gain an optional `id`, unique inside the scene, which `_layer`
  refers to.
- Scene layers gain `visible` (default `true`). The schema already accepts it,
  but the engine ignores it today. With this change a hidden layer is not
  drawn, in static scenes too. It is the initial state that `setVisible`
  changes.
- Time is global. Animated layers (timelines, animated panels) keep looping
  on their own while the sequence runs, exactly as in a scene. The file loops
  on the least common multiple of the sequence's `duration` and the loops of
  those layers. When that is longer than the sequence's own `duration`,
  `render` prints a notice.
- Sequences are always animated: they are written as APNG with
  `--format png` and as animated WebP with `--format webp`, to
  `<output-path>/sequences/<id>.<ext>`. They are listed in the manifest and
  exported by default when `exports` is absent.
- Validation:
  - `_scene` resolves;
  - `_layer` matches a layer `id` of that scene;
  - layer ids are unique inside each scene;
  - `at` and `start` are less than `duration`;
  - a `moveTo` ends within the sequence;
  - `moveTo` actions on the same layer do not overlap.
- **BREAKING** (rendering): a scene layer with `visible: false` is no longer
  drawn. Before this change the flag passed the schema but was ignored. No
  example or fixture uses it.

## Capabilities

### New Capabilities
- `sequences`: evaluation of scene-level animations. Covers layer state over
  time (instant actions, linear movement), the clock and loop with nested
  animated layers, the frames to write, and the loop-length notice.

### Modified Capabilities
- `composition`: scene layers honor `visible`; a scene is animated only
  through layers that are visible.
- `animation`: loop lengths and animated scenes ignore hidden layers, and the
  common clock covers sequences.
- `worksheet-loading`: unique layer ids, sequence references, and consistent
  sequence actions.
- `png-export`: export selection, output paths and the manifest include
  `sequences`.

## Impact

- Schema: `schema/worksheet.schema.json` (already drafted). The `sequences`
  collection, the `sequence` and `action` definitions, layer `id` and
  `visible`, and `exports` accepting `/sequences/<id>`.
- Model: `Sequence`, `Action`, and `Layer.id`/`Layer.visible` in `model.rs`.
- Semantic checks in `semantic.rs`: references, layer ids, action
  consistency.
- Animation in `animate.rs`: a new `Kind::Sequence`. Its layer-state
  evaluation shares its logic with timelines (`position`, instant ordering).
  It also contributes change ticks, a loop length and rendering.
- Composition in `compose.rs`: the static scene path skips hidden layers.
- Export in `export.rs`: the `sequences` collection, its sizes, the default
  export list and the loop-length notice.
- Docs: README (Concepts, Animation) and a new example in `examples/`.
- No new dependencies.
