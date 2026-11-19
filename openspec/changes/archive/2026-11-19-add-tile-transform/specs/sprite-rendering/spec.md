## ADDED Requirements

### Requirement: Tile transform
A `tile` transform SHALL turn an image of `w` x `h` pixels into an image of
exactly `width` x `height` pixels where the pixel `(x, y)` is the source pixel
`(x mod w, y mod h)`. The repetition SHALL start at the image's top-left corner;
tiles that do not fit SHALL be cut at the right and bottom edges. Transparent
pixels SHALL be repeated like any other pixel.

#### Scenario: Repeat horizontally
- **WHEN** a 2x1 image `AB` is tiled to 5x1
- **THEN** it becomes `ABABA`

#### Scenario: Repeat in both directions
- **WHEN** a 2x2 image with rows `AB`, `CD` is tiled to 3x3
- **THEN** its rows become `ABA`, `CDC`, `ABA`

#### Scenario: Smaller than the image
- **WHEN** a 3x2 image with rows `ABC`, `DEF` is tiled to 2x1
- **THEN** it becomes `AB`

#### Scenario: Same size
- **WHEN** an image is tiled to its own width and height
- **THEN** it is unchanged

#### Scenario: Tile in a chain
- **WHEN** a 2x1 image `AB` gets `rotate` 90 and then `tile` to 3x4
- **THEN** the result has rows `AAA`, `BBB`, `AAA`, `BBB`

#### Scenario: Tiled flipbook element
- **WHEN** a panel element shows a 1x1 flipbook whose frames are `A` then `B` and has a `tile` to 3x1
- **THEN** the element shows `AAA` during the first frame and `BBB` during the second
