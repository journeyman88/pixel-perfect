# sprite-rendering Specification

## Purpose

Turn resolved sprites into RGBA8 images with a fixed pixel model (palette
lookup, canonical transparency) and provide the image transforms (`flip`,
`rotate`, `scale`, `crop`) that frames and elements apply in array order.

## Requirements

### Requirement: Sprite rasterization
The engine SHALL rasterize a resolved sprite into an RGBA8 image of the sprite's
width and height, where each character is replaced by its color in the sprite's
resolved palette and `.` becomes a fully transparent pixel.

#### Scenario: Palette lookup
- **WHEN** a sprite row is `r.g` with `r` = `[255, 0, 0]` and `g` = `[0, 255, 0, 0.5]`
- **THEN** the pixels are `(255, 0, 0, 255)`, `(0, 0, 0, 0)` and `(0, 255, 0, 128)`

#### Scenario: Derived sprite uses its own palette
- **WHEN** sprite `hero-dark` derives from `hero` and sets `_palette` to `dark`
- **THEN** its pixels use the colors of `dark`, not those of the base sprite's palette

### Requirement: Canonical transparency
The engine SHALL store every pixel whose alpha is 0 as `(0, 0, 0, 0)`, whatever
color channels the palette gives it.

#### Scenario: Transparent palette color
- **WHEN** a palette maps `x` to `[200, 10, 10, 0.0]` and a sprite uses `x`
- **THEN** those pixels are `(0, 0, 0, 0)`, identical to `.`

### Requirement: Flip transform
A `flip` transform SHALL mirror the image left-right for `axis` `x` and
top-bottom for `axis` `y`, keeping its size.

#### Scenario: Horizontal flip
- **WHEN** a 3x1 image `ABC` is flipped on `x`
- **THEN** it becomes `CBA`

#### Scenario: Vertical flip
- **WHEN** a 1x3 image with rows `A`, `B`, `C` is flipped on `y`
- **THEN** its rows become `C`, `B`, `A`

### Requirement: Rotate transform
A `rotate` transform SHALL rotate the image clockwise by `degrees`; 90 and 270
SHALL swap width and height.

#### Scenario: Rotate 90 degrees
- **WHEN** a 2x3 image with rows `AB`, `CD`, `EF` is rotated by 90
- **THEN** it becomes a 3x2 image with rows `ECA`, `FDB`

#### Scenario: Rotate 180 degrees
- **WHEN** a 2x2 image with rows `AB`, `CD` is rotated by 180
- **THEN** its rows become `DC`, `BA`

#### Scenario: Rotate 270 degrees
- **WHEN** a 2x3 image with rows `AB`, `CD`, `EF` is rotated by 270
- **THEN** it becomes a 3x2 image with rows `BDF`, `ACE`

### Requirement: Scale transform
A `scale` transform SHALL resize the image to exactly `width` x `height` with
nearest-neighbor sampling, where the destination pixel `(x, y)` takes the source
pixel `(floor(x * srcWidth / width), floor(y * srcHeight / height))`.

#### Scenario: Integer upscale
- **WHEN** a 2x1 image `AB` is scaled to 4x2
- **THEN** its rows become `AABB`, `AABB`

#### Scenario: Downscale
- **WHEN** a 4x1 image `ABCD` is scaled to 2x1
- **THEN** it becomes `AC`

#### Scenario: Non-integer ratio
- **WHEN** a 2x1 image `AB` is scaled to 3x1
- **THEN** it becomes `AAB`

### Requirement: Crop transform
A `crop` transform SHALL keep the area `[x, y, width, height]` of the image as it
is at that point of the chain.

#### Scenario: Crop inner area
- **WHEN** a 3x3 image with rows `ABC`, `DEF`, `GHI` is cropped with `[1, 1, 2, 2]`
- **THEN** it becomes a 2x2 image with rows `EF`, `HI`

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

### Requirement: Transform chains
The engine SHALL apply a list of transforms in array order, each one to the
result of the previous.

#### Scenario: Order matters
- **WHEN** a 2x1 image `AB` gets `rotate` 90 and then `flip` on `y`
- **THEN** the result is a 1x2 image with rows `B`, `A`
