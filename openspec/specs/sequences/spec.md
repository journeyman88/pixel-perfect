# sequences Specification

## Purpose

Animate the layers of a scene with a sequence: `moveTo` (linear movement, for
parallax), `setVisible` and `moveToZ` (instant, for effect layers such as
lights), evaluated on global time so that animated layers keep their own
rhythm, with a loop that is the least common multiple of the sequence and its
drawable animated layers, and a notice when that loop is longer than the
sequence itself.

## Requirements

### Requirement: Sequence evaluation
A sequence SHALL render, at each instant, its `_scene` with every layer in
the state given by the scene and the sequence's `actions`, using the scene
rendering rules of the composition capability. The state of a layer is its
`coordinates`, `zIndex` and `visible`. Time is measured in frame-units of the
sequence's `fps`. The actions SHALL be evaluated at frame-unit
`t mod duration`, so every loop of the sequence starts again from the scene's
state. Within a frame-unit the layer states SHALL be constant. Layers without
actions SHALL keep the state the scene gives them.

#### Scenario: Loop restarts from the scene
- **WHEN** a sequence with `duration` 8 has `setVisible` with `value` false on layer `lamp` at 4
- **THEN** `lamp` is drawn during frame-units 0–3, hidden during 4–7, and drawn again at the start of the next loop

#### Scenario: Untouched layers
- **WHEN** a scene has layers `sky` and `lamp` and the sequence only has actions on `lamp`
- **THEN** every frame draws `sky` at its scene `coordinates` and `zIndex`

### Requirement: Instant actions
`setVisible` and `moveToZ` SHALL take effect at frame-unit `at`, and the
change SHALL already be visible in that frame-unit. Actions SHALL be applied
in order of `at`, and actions with the same `at` in array order. `setVisible`
SHALL set the layer's `visible`; `moveToZ` SHALL set its `zIndex`. Layers SHALL
be ordered by the resulting `zIndex`, with ties broken by array order in the
scene.

#### Scenario: Light switching on
- **WHEN** layer `light` has `visible: false` in the scene and a `setVisible` with `value` true at 6
- **THEN** `light` is drawn from frame-unit 6, not from 7

#### Scenario: Same `at`, array order
- **WHEN** two `moveToZ` on layer `fog` have `at` 3 and `value` 5 and 1, in that order
- **THEN** from frame-unit 3 `fog` has `zIndex` 1

#### Scenario: Moving in front
- **WHEN** layer `light` (`zIndex` 0) gets `moveToZ` with `value` 10 at 2 and layer `tree` has `zIndex` 5
- **THEN** from frame-unit 2 `light` is drawn above `tree`

### Requirement: Linear layer movement
A `moveTo` SHALL move its layer from `p0` to `p1`. `p0` is the layer's
coordinates at frame-unit `start`, given by the previous `moveTo` of the same
layer or, if there is none, by the scene. `p1` is the action's `coordinates`.
The interpolation and rounding SHALL be those of the animation capability's
linear movement, with the layer's coordinates in place of an element's:
`p0 + round_away((p1 − p0) · (t − start), duration)` on each axis for
`start ≤ t ≤ start + duration`. After `start + duration` the layer SHALL stay
at `p1`.

#### Scenario: Parallax planes
- **WHEN** a sequence with `duration` 16 moves layer `far` from `[0, 0]` to `[-4, 0]` and layer `near` from `[0, 0]` to `[-16, 0]`, both with `start` 0 and `duration` 16
- **THEN** at frame-unit 8 `far` is at `[-2, 0]` and `near` at `[-8, 0]`

#### Scenario: Movement clipped to the scene
- **WHEN** a layer moves so that half of it lies outside the scene canvas
- **THEN** only the part inside the canvas is drawn

### Requirement: Animated layers in a sequence
Animated layers SHALL be rendered at the global time of the sequence, within
their own loop, exactly as in an animated scene. Animated layers are `_timeline`
layers and `_panel` layers whose panel is animated. Actions SHALL change only
where, in which order, and whether a layer is drawn, never its own time. A
hidden animated layer SHALL keep running.

#### Scenario: Flicker keeps its rhythm
- **WHEN** a sequence with a 4 s loop moves a `_timeline` layer `lamp` whose timeline loops every 0.75 s
- **THEN** at 4.5 s `lamp` shows its timeline at 0 s (4.5 mod 0.75) at its moved position

#### Scenario: Shown after being hidden
- **WHEN** an animated panel layer is hidden from 0 s to 1 s and shown at 1 s
- **THEN** at 1 s it shows its animation at 1 s, not at 0 s

### Requirement: Sequence clock and loop
A sequence's clock SHALL follow the common clock rule of the animation
capability. It SHALL involve the sequence's `fps` and the rates of every
animated layer of its scene that can be drawn. A layer can be drawn if it is
visible in the scene, or if some `setVisible` of the sequence sets it to true.
The sequence's loop SHALL be the least common multiple of its own length
(`duration` frame-units at its `fps`) and the loops of those layers.

#### Scenario: Loop of a sequence alone
- **WHEN** a sequence at `fps` 8 with `duration` 32 has only static layers
- **THEN** its loop is 4 s

#### Scenario: Loop with an animated layer
- **WHEN** a sequence of 4 s has a visible `_timeline` layer with a 1.5 s loop
- **THEN** its loop is 12 s and the sequence's actions repeat three times

#### Scenario: Layer that is never drawn
- **WHEN** a `_timeline` layer with a 1.5 s loop has `visible: false` and no `setVisible` with `value` true targets it
- **THEN** it does not affect the clock or the loop

### Requirement: Sequence frames
A sequence SHALL be turned into frames with the frame sequence rule of the
animation capability. Its own change points SHALL be:
- every frame-unit boundary inside a `moveTo`;
- the `at` of every instant action;
- every restart of the sequence's loop;
- the change points of its animated layers that can be drawn.

#### Scenario: Static layers, one switch
- **WHEN** a sequence at `fps` 4 with `duration` 8 has only static layers and a single `setVisible` at 4
- **THEN** it is written with two frames of 1 s each

#### Scenario: Durations sum to the loop
- **WHEN** any sequence is rendered
- **THEN** the durations of its frames add up to its loop length in ticks

### Requirement: Longer loop notice
`render` SHALL print a notice when the loop of a sequence is longer than its
own length (`duration` at `fps`), to standard error as
`<file>: note: /sequences/<id> loops every <loop> s (its own duration is
<length> s) because of its animated layers`. The notice SHALL be printed after
rendering and SHALL NOT change the exit code. Lengths SHALL be written as
exact fractions or integers, such as `12` or `3/2`.

#### Scenario: Notice
- **WHEN** sequence `night` of 4 s has a layer whose loop is 1.5 s
- **THEN** render succeeds and prints `note: /sequences/night loops every 12 s (its own duration is 4 s) because of its animated layers`

#### Scenario: No notice
- **WHEN** every animated layer's loop divides the sequence's length
- **THEN** no notice is printed
