# composition Specification

## Purpose

Compose sprites into panels and panels into scenes with exact, platform-independent
rules: integer source-over alpha blending, `zIndex` ordering with ties by array
order, visibility, placement with clipping, panels flattened as groups in
scenes, and which panels and scenes count as animated.

## Requirements

### Requirement: Alpha compositing
The engine SHALL combine a source pixel `(sr, sg, sb, sa)` over a destination
pixel `(dr, dg, db, da)` (8-bit channels, non-premultiplied alpha) with
source-over computed in integer arithmetic:

- `A = sa·255 + da·(255 − sa)`
- if `A = 0`, the result is `(0, 0, 0, 0)`;
- otherwise the result alpha is `round(A, 255)` and each color channel `c` is
  `round(sc·sa·255 + dc·da·(255 − sa), A)`,

where `round(n, d) = floor((2n + d) / (2d))` (round half up). The result SHALL
be the same on every platform, and no floating-point arithmetic SHALL be used.

#### Scenario: Opaque source
- **WHEN** an opaque pixel is drawn over any pixel
- **THEN** the result is the source pixel

#### Scenario: Transparent source
- **WHEN** a pixel with alpha 0 is drawn over any pixel
- **THEN** the result is the destination pixel

#### Scenario: Source over a transparent destination
- **WHEN** `(10, 20, 30, 77)` is drawn over `(0, 0, 0, 0)`
- **THEN** the result is `(10, 20, 30, 77)`

#### Scenario: Semi-transparent over opaque
- **WHEN** `(255, 0, 0, 128)` is drawn over `(0, 0, 255, 255)`
- **THEN** the result is `(128, 0, 127, 255)`

#### Scenario: Semi-transparent over semi-transparent
- **WHEN** `(255, 255, 255, 128)` is drawn over `(0, 0, 0, 128)`
- **THEN** the result is `(170, 170, 170, 192)`

### Requirement: Panel rendering
A panel SHALL be rendered on a canvas of its `width` x `height`, initially
fully transparent. Each element with `visible` true (the default) SHALL be
drawn in ascending `zIndex` order (default 0); elements with the same `zIndex`
SHALL be drawn in array order, later entries on top. Elements with
`visible: false` SHALL NOT be drawn.

#### Scenario: Higher zIndex on top
- **WHEN** element A (`zIndex` 2) is first in `elements` and element B (`zIndex` 0) is second, and both cover pixel (0, 0) with opaque colors
- **THEN** pixel (0, 0) has A's color

#### Scenario: Tie broken by array order
- **WHEN** two elements have the same `zIndex` and cover the same pixel with opaque colors
- **THEN** the pixel has the color of the element that comes later in `elements`

#### Scenario: Hidden element
- **WHEN** an element has `visible: false`
- **THEN** the panel is identical to the same panel without that element

#### Scenario: Uncovered pixels
- **WHEN** no element covers a pixel of the panel
- **THEN** that pixel is `(0, 0, 0, 0)`

### Requirement: Element placement and clipping
An element SHALL be drawn by taking its source image (the rendered sprite of
`_sprite`), applying its `transforms` in array order, and drawing pixel (i, j)
of the result at canvas position (x + i, y + j), where `[x, y]` are its
`coordinates`. Positions outside the canvas SHALL be discarded. `coordinates`
MAY be negative or beyond the canvas size.

#### Scenario: Placement
- **WHEN** a 2x2 opaque sprite is placed at `[3, 1]` in a 8x4 panel
- **THEN** pixels (3, 1), (4, 1), (3, 2) and (4, 2) have the sprite's colors and all other pixels are transparent

#### Scenario: Negative coordinates
- **WHEN** a 4x4 sprite is placed at `[-2, -1]`
- **THEN** only its pixels with i ≥ 2 and j ≥ 1 are drawn, at (i − 2, j − 1)

#### Scenario: Completely outside
- **WHEN** an element is placed at `[100, 0]` in a 32x16 panel
- **THEN** the panel is fully transparent where no other element is drawn and no error is raised

#### Scenario: Transforms before placement
- **WHEN** an element has `transforms` `[{ "kind": "rotate", "degrees": 90 }]` on a 3x2 sprite at `[0, 0]`
- **THEN** the drawn area is 2x3 and holds the rotated sprite

### Requirement: Scene rendering
A scene SHALL be rendered on a canvas of its `width` x `height`, initially
fully transparent. Each `_panel` layer SHALL first be rendered as a panel on
its own canvas, then drawn onto the scene canvas at its `coordinates` with the
ordering rule of panels (ascending `zIndex`, ties by array order) and the
clipping rule of elements. Drawing SHALL use the alpha compositing rule pixel by
pixel.

#### Scenario: Layer order
- **WHEN** a scene has layer A (`zIndex` 1) then layer B (`zIndex` 0) covering the same pixel with opaque colors
- **THEN** the pixel has layer A's color

#### Scenario: Layer clipped to the scene
- **WHEN** a 32x16 panel is placed at `[48, 0]` in a 64x32 scene
- **THEN** only its left 16 columns are visible

#### Scenario: Panel rendered as a group
- **WHEN** a panel stacks two semi-transparent elements and is used as a scene layer over an opaque background layer
- **THEN** the panel is first flattened on its own transparent canvas, and the flattened result is composited once over the background

### Requirement: Animated compositions
A panel SHALL be animated if at least one of its elements with `visible` true
has `_flipbook`. A scene SHALL be animated if at least one layer has
`_timeline` or refers with `_panel` to an animated panel. Animated panels and
scenes SHALL NOT be rendered as static images; they are rendered by the
animation capability.

#### Scenario: Visible flipbook element
- **WHEN** a panel has an element with `_flipbook` and `visible` true
- **THEN** the panel is animated

#### Scenario: Hidden flipbook element
- **WHEN** the only `_flipbook` element of a panel has `visible: false`
- **THEN** the panel is static and the flipbook is not drawn

#### Scenario: Timeline layer
- **WHEN** a scene has a layer with `_timeline`
- **THEN** the scene is animated
