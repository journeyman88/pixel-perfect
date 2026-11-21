## MODIFIED Requirements

### Requirement: Scene rendering
A scene SHALL be rendered on a canvas of its `width` x `height`, initially
fully transparent. Layers with `visible` false SHALL NOT be drawn; `visible`
SHALL default to true. Each drawn `_panel` layer SHALL first be rendered as a
panel on its own canvas, then drawn onto the scene canvas at its `coordinates`
with the ordering rule of panels (ascending `zIndex`, ties by array order) and
the clipping rule of elements. Drawing SHALL use the alpha compositing rule
pixel by pixel.

#### Scenario: Layer order
- **WHEN** a scene has layer A (`zIndex` 1) then layer B (`zIndex` 0) covering the same pixel with opaque colors
- **THEN** the pixel has layer A's color

#### Scenario: Layer clipped to the scene
- **WHEN** a 32x16 panel is placed at `[48, 0]` in a 64x32 scene
- **THEN** only its left 16 columns are visible

#### Scenario: Panel rendered as a group
- **WHEN** a panel stacks two semi-transparent elements and is used as a scene layer over an opaque background layer
- **THEN** the panel is first flattened on its own transparent canvas, and the flattened result is composited once over the background

#### Scenario: Hidden layer
- **WHEN** a scene has an opaque background layer and a layer `light` with `visible: false` above it
- **THEN** the rendered scene equals the background alone

### Requirement: Animated compositions
A panel SHALL be animated if at least one of its elements with `visible` true
has `_flipbook`. A scene SHALL be animated if at least one layer with `visible`
true has `_timeline` or refers with `_panel` to an animated panel. Flipbooks,
timelines and sequences SHALL always be animated. Animated resources SHALL be
rendered by the animation capability (and sequences by the sequences
capability) as a sequence of frames, each composed with the same rules as a
static panel or scene (alpha compositing, `zIndex` ordering, visibility,
placement and clipping, panels flattened as groups in scenes), applied to the
state of every element and layer at that frame's instant.

#### Scenario: Visible flipbook element
- **WHEN** a panel has an element with `_flipbook` and `visible` true
- **THEN** the panel is animated

#### Scenario: Hidden flipbook element
- **WHEN** the only `_flipbook` element of a panel has `visible: false`
- **THEN** the panel is static and the flipbook is not drawn

#### Scenario: Timeline layer
- **WHEN** a scene has a layer with `_timeline`
- **THEN** the scene is animated

#### Scenario: Hidden timeline layer
- **WHEN** the only `_timeline` layer of a scene has `visible: false`
- **THEN** the scene is static and the timeline is not drawn

#### Scenario: Sequence over a static scene
- **WHEN** a sequence's `_scene` has only static layers
- **THEN** the sequence is still animated and is rendered as an animation

#### Scenario: Timeline over a static panel
- **WHEN** a timeline's `_panel` has no flipbook elements
- **THEN** the timeline is still animated and is rendered as an animation

#### Scenario: Frames use the static rules
- **WHEN** a frame of an animated panel is compared with the static rendering of the same panel whose elements are set to the frame's state, with each flipbook element replaced by a sprite showing its current frame
- **THEN** the two images are identical
