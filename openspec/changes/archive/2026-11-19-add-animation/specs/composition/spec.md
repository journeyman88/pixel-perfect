## MODIFIED Requirements

### Requirement: Animated compositions
A panel SHALL be animated if at least one of its elements with `visible` true
has `_flipbook`. A scene SHALL be animated if at least one layer has
`_timeline` or refers with `_panel` to an animated panel. Flipbooks and
timelines SHALL always be animated. Animated resources SHALL be rendered by the
animation capability as a sequence of frames, each composed with the same rules
as a static panel or scene (alpha compositing, `zIndex` ordering, visibility,
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

#### Scenario: Timeline over a static panel
- **WHEN** a timeline's `_panel` has no flipbook elements
- **THEN** the timeline is still animated and is rendered as an animation

#### Scenario: Frames use the static rules
- **WHEN** a frame of an animated panel is compared with the static rendering of the same panel whose elements are set to the frame's state, with each flipbook element replaced by a sprite showing its current frame
- **THEN** the two images are identical

#### Scenario: Timeline layer flattened as a group
- **WHEN** a scene has a timeline layer over an opaque background layer
- **THEN** each frame of the timeline is first rendered on its own transparent canvas of the panel's size, and that image is composited once over the background
