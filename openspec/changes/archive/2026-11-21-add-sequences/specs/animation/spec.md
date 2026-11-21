## MODIFIED Requirements

### Requirement: Common clock
The engine SHALL time each animated resource (flipbook, timeline, animated
panel, animated scene or sequence) with a clock of `F` ticks per second, where
`F` is the least common multiple of the `fps` of every flipbook, timeline and
sequence the resource involves, directly or through its panels, elements and
layers. All times SHALL be whole numbers of ticks and SHALL be computed in
integer arithmetic. If `F` or a loop length does not fit the engine's integer
range, rendering SHALL fail before anything is written, with an error pointing
to the resource.

#### Scenario: Two rates
- **WHEN** a scene has a timeline layer with `fps` 12 whose panel contains a flipbook element with `fps` 8
- **THEN** the scene's clock is 24 ticks per second, a timeline frame-unit lasts 2 ticks and a flipbook frame-unit lasts 3 ticks

#### Scenario: Single rate
- **WHEN** a flipbook with `fps` 10 is exported
- **THEN** its clock is 10 ticks per second

#### Scenario: Sequence over a timeline layer
- **WHEN** a sequence at `fps` 4 has a scene with a timeline layer at `fps` 6
- **THEN** the sequence's clock is 12 ticks per second

### Requirement: Loop lengths
The loop of a flipbook SHALL be the sum of its frames' `duration` at its `fps`.
The loop of a timeline SHALL be its `duration` at its `fps`. The loop of an
animated panel SHALL be the least common multiple of the loops of its elements
that have `_flipbook` and `visible` true. The loop of an animated scene SHALL be
the least common multiple of the loops of its animated layers that have
`visible` true; static layers and hidden layers SHALL NOT affect it. The loop of
a sequence is defined by the sequences capability. Each component SHALL loop
independently within the resource's loop.

#### Scenario: Panel with two flipbooks
- **WHEN** an animated panel has visible flipbook elements with loops of 0.4 s and 0.6 s
- **THEN** the panel's loop is 1.2 s

#### Scenario: Scene with a timeline and an animated panel
- **WHEN** a scene has a timeline layer with a 2 s loop and an animated panel layer with a 0.75 s loop
- **THEN** the scene's loop is 6 s

#### Scenario: Static layer
- **WHEN** a scene has a static panel layer and a timeline layer with a 2 s loop
- **THEN** the scene's loop is 2 s

#### Scenario: Hidden layer
- **WHEN** a scene has a visible timeline layer with a 2 s loop and a hidden one with a 3 s loop
- **THEN** the scene's loop is 2 s

### Requirement: Animated scenes
An animated scene SHALL be rendered at each instant by rendering each layer
with `visible` true at that instant (a timeline layer at its own time within
its loop, an animated panel layer at its own time within its loop, a static
layer as a static image) and compositing the layers with the scene rendering
rules of the composition capability. Layers with `visible` false SHALL NOT be
drawn.

#### Scenario: Layers loop independently
- **WHEN** a scene has a timeline layer with a 2 s loop and another with a 3 s loop
- **THEN** at 4.5 s the first layer shows its state at 0.5 s and the second its state at 1.5 s
