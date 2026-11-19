# animation Specification

## Purpose

Evaluate animated resources exactly: the common clock of a resource, flipbook
playback, loop lengths, timeline state (instant transitions, linear `moveTo`,
flipbook phase and `seekFlipbook`), animated scenes, and the merged sequence
of frames covering one loop.

## Requirements

### Requirement: Common clock
The engine SHALL time each animated resource (flipbook, timeline, animated
panel or animated scene) with a clock of `F` ticks per second, where `F` is the
least common multiple of the `fps` of every flipbook and timeline the resource
involves, directly or through its panels, elements and layers. All times SHALL
be whole numbers of ticks and SHALL be computed in integer arithmetic. If `F` or
a loop length does not fit the engine's integer range, rendering SHALL fail
before anything is written, with an error pointing to the resource.

#### Scenario: Two rates
- **WHEN** a scene has a timeline layer with `fps` 12 whose panel contains a flipbook element with `fps` 8
- **THEN** the scene's clock is 24 ticks per second, a timeline frame-unit lasts 2 ticks and a flipbook frame-unit lasts 3 ticks

#### Scenario: Single rate
- **WHEN** a flipbook with `fps` 10 is exported
- **THEN** its clock is 10 ticks per second

### Requirement: Flipbook playback
A flipbook SHALL show each entry of `frames` for `duration` frame-units at the
flipbook's `fps`, in array order, then loop. A frame SHALL be rendered on a
transparent canvas of the flipbook's `width` x `height` by drawing the frame's
`_sprite`, after its `transforms`, at its `coordinates`, with the placement and
clipping rules of panel elements. The loop length SHALL be the sum of the
frames' `duration`.

#### Scenario: Frame durations
- **WHEN** a flipbook at `fps` 10 has frames A (`duration` 2) and B (`duration` 1)
- **THEN** A is shown for 0.2 s, B for 0.1 s, and the loop lasts 0.3 s

#### Scenario: Frame clipped to the canvas
- **WHEN** a frame places an 8x8 sprite at `[-2, 3]` on an 8x8 flipbook canvas
- **THEN** only the part of the sprite inside the canvas is visible in that frame

### Requirement: Flipbook elements
A panel element with `_flipbook` SHALL draw, at each instant, the flipbook frame
current at that instant, as an image of the flipbook's canvas size, then apply
the element's `transforms` and place it as any element. Outside a timeline, the
flipbook's time SHALL start at time 0 of the resource being rendered. A hidden
element's flipbook SHALL keep running.

#### Scenario: Element plays its own rate
- **WHEN** an animated panel has a flipbook element at `fps` 4 with two frames of `duration` 1
- **THEN** the panel shows the first frame for 0.25 s and the second for 0.25 s, looping every 0.5 s

#### Scenario: Element transforms apply to the canvas
- **WHEN** a flipbook element on an 8x8 canvas has a `crop` with area `[1, 1, 6, 6]`
- **THEN** every frame is cropped to 6x6 before being placed

### Requirement: Loop lengths
The loop of a flipbook SHALL be the sum of its frames' `duration` at its `fps`.
The loop of a timeline SHALL be its `duration` at its `fps`. The loop of an
animated panel SHALL be the least common multiple of the loops of its elements
that have `_flipbook` and `visible` true. The loop of an animated scene SHALL be
the least common multiple of the loops of its animated layers; static layers
SHALL NOT affect it. Each component SHALL loop independently within the
resource's loop.

#### Scenario: Panel with two flipbooks
- **WHEN** an animated panel has visible flipbook elements with loops of 0.4 s and 0.6 s
- **THEN** the panel's loop is 1.2 s

#### Scenario: Scene with a timeline and an animated panel
- **WHEN** a scene has a timeline layer with a 2 s loop and an animated panel layer with a 0.75 s loop
- **THEN** the scene's loop is 6 s

#### Scenario: Static layer
- **WHEN** a scene has a static panel layer and a timeline layer with a 2 s loop
- **THEN** the scene's loop is 2 s

### Requirement: Timeline evaluation
A timeline SHALL render, at each frame-unit `t` of its loop (`0 ≤ t <
duration`), its `_panel` with every element in the state given by the panel
and the transitions, using the panel rendering rules of the composition
capability. Every loop SHALL start again from the panel's state. Within a
frame-unit the timeline state SHALL be constant, while flipbook elements SHALL
advance at their own `fps`.

#### Scenario: Loop restarts from the panel
- **WHEN** a timeline with `duration` 10 has `setVisible` with `value` false on element `hero` at 5
- **THEN** `hero` is visible during frame-units 0–4, hidden during 5–9, and visible again at the start of the next loop

#### Scenario: Flipbook faster than the timeline
- **WHEN** a timeline at `fps` 5 contains a flipbook element at `fps` 10
- **THEN** the flipbook changes frame twice per timeline frame-unit

### Requirement: Instant transitions
Instant transitions SHALL take effect at frame-unit `at`: `setVisible`,
`moveToZ`, `appendTransforms`, `resetTransforms` and `seekFlipbook`. The change is already
visible in that frame-unit. Transitions SHALL be applied in order of `at`, and
transitions with the same `at` in array order. `setVisible` SHALL set
`visible`; `moveToZ` SHALL set `zIndex`; `appendTransforms` SHALL append its
`transforms` after the element's current chain; `resetTransforms` SHALL remove
every transformation added by `appendTransforms` and keep the element's own
`transforms`.

#### Scenario: Visible from `at`
- **WHEN** element `spinner` starts with `visible: false` and `setVisible` with `value` true has `at` 10
- **THEN** `spinner` is drawn from frame-unit 10, not from 11

#### Scenario: Same `at`, array order
- **WHEN** two `moveToZ` on the same element have `at` 3 and `value` 5 and 1, in that order
- **THEN** from frame-unit 3 the element has `zIndex` 1

#### Scenario: Append twice, then reset
- **WHEN** an element with `transforms` `[flip x]` gets `appendTransforms` `[rotate 90]` at 2, `appendTransforms` `[flip y]` at 4 and `resetTransforms` at 6
- **THEN** its chain is `[flip x]` before 2, `[flip x, rotate 90]` from 2, `[flip x, rotate 90, flip y]` from 4 and `[flip x]` from 6

### Requirement: Linear movement
A `moveTo` SHALL move its element from `p0`, its coordinates at frame-unit
`start` (given by the previous `moveTo` of the same element, or else by the
panel), to `p1`, its `coordinates`. For `start ≤ t ≤ start + duration` the
coordinates SHALL be `p0 + offset` on each axis, with

```
offset = round_away((p1 − p0) · (t − start), duration)
round_away(n, d) = sign(n) · floor((2·|n| + d) / (2·d))
```

computed in integer arithmetic (halves rounded away from zero). After
`start + duration` the element SHALL stay at `p1`.

#### Scenario: Exact steps
- **WHEN** an element moves from `[0, 0]` to `[10, 0]` with `start` 0 and `duration` 5
- **THEN** its x coordinate is 0, 2, 4, 6, 8 at frame-units 0–4 and 10 from frame-unit 5

#### Scenario: Halves round away from zero
- **WHEN** an element moves from `[0, 0]` to `[1, 0]` with `start` 0 and `duration` 2
- **THEN** its x coordinate is 0 at frame-unit 0 and 1 at frame-units 1 and 2

#### Scenario: Mirrored movement
- **WHEN** one element moves from `[0, 0]` to `[1, 0]` and another from `[0, 0]` to `[-1, 0]`, both with `start` 0 and `duration` 2
- **THEN** at frame-unit 1 their x coordinates are 1 and −1

#### Scenario: Chained movements
- **WHEN** an element at `[0, 0]` has `moveTo` `[4, 0]` (`start` 0, `duration` 4) and `moveTo` `[4, 8]` (`start` 6, `duration` 2)
- **THEN** it is at `[4, 0]` during frame-units 4–6, at `[4, 4]` at 7 and at `[4, 8]` from 8

#### Scenario: Seamless loop
- **WHEN** a timeline with `duration` 8 moves an element from its panel position `[0, 0]` to `[8, 0]` (`start` 0, `duration` 4) and back to `[0, 0]` (`start` 4, `duration` 4)
- **THEN** frame-unit 7 shows the element at `[2, 0]` and the next loop starts at `[0, 0]` without a repeated position

### Requirement: Flipbook phase in timelines
Inside a timeline, the local time of every flipbook element SHALL restart at the
start of every timeline loop. A `seekFlipbook` SHALL set its element's flipbook,
at frame-unit `at`, to the start of `frames[frameIndex]` (`frameIndex` 0 when
absent); playback SHALL continue from there at the flipbook's `fps`. The
timeline's loop SHALL NOT be lengthened by its flipbook elements.

#### Scenario: Phase reset at loop
- **WHEN** a timeline at `fps` 10 with `duration` 5 contains a flipbook at `fps` 10 with three frames of `duration` 2
- **THEN** each timeline loop shows the first frame for 2 frame-units, the second for 2 and the third for 1, and the next loop starts again from the first frame

#### Scenario: Seek to a frame
- **WHEN** a timeline at `fps` 10 has `seekFlipbook` with `frameIndex` 2 and `at` 15 on a flipbook element at `fps` 10
- **THEN** at frame-unit 15 the element shows `frames[2]` from its first frame-unit

#### Scenario: Restart
- **WHEN** a `seekFlipbook` has no `frameIndex`
- **THEN** the flipbook restarts from `frames[0]` at `at`

#### Scenario: Seek while hidden
- **WHEN** a flipbook element is hidden, gets `seekFlipbook` at 4 and becomes visible at 6
- **THEN** at frame-unit 6 it shows the frame reached 2 frame-units after the seek

### Requirement: Animated scenes
An animated scene SHALL be rendered at each instant by rendering each layer at
that instant (a timeline layer at its own time within its loop, an animated
panel layer at its own time within its loop, a static layer as a static image)
and compositing the layers with the scene rendering rules of the composition
capability.

#### Scenario: Layers loop independently
- **WHEN** a scene has a timeline layer with a 2 s loop and another with a 3 s loop
- **THEN** at 4.5 s the first layer shows its state at 0.5 s and the second its state at 1.5 s

### Requirement: Frame sequence
The engine SHALL turn each animated resource into a sequence of frames covering
exactly one loop, each with a whole number of ticks. A new frame SHALL start
only at a tick where some component can change (a flipbook frame boundary, a
timeline frame-unit boundary inside a `moveTo`, the `at` of an instant
transition, or a loop restart of a component), and consecutive frames with
identical pixels SHALL be merged into one frame whose duration is the sum of
theirs.

#### Scenario: Merged still frames
- **WHEN** a timeline with `duration` 10 at `fps` 10 has a single `setVisible` at 5 and no flipbook elements
- **THEN** the sequence has two frames of 5 ticks each

#### Scenario: Change hidden by clipping
- **WHEN** the only transition makes visible an element placed entirely outside the panel
- **THEN** the sequence has a single frame covering the whole loop

#### Scenario: Durations sum to the loop
- **WHEN** any animated resource is rendered
- **THEN** the durations of its frames add up to its loop length in ticks
