## MODIFIED Requirements

### Requirement: Transforms stay within bounds
Each `crop` area SHALL lie inside the image as it is at that point of the
`transforms` chain, taking previous `rotate`, `scale`, `tile` and `crop` steps
into account. This SHALL hold for the chains declared on flipbook frames and
panel elements, and for every chain an element can have during a timeline: its
own `transforms` followed by the `transforms` of the `appendTransforms` applied
since the last `resetTransforms`, in the order in which the transitions are
applied (by `at`, ties in array order). An error in an appended `crop` SHALL
point to the `area` inside that `appendTransforms` transition.

#### Scenario: Crop after rotation
- **WHEN** a 4x8 sprite is rotated by 90 degrees and then cropped with area `[0, 0, 8, 4]`
- **THEN** validation succeeds

#### Scenario: Crop out of bounds
- **WHEN** a 4x8 sprite is cropped with area `[2, 0, 4, 8]`
- **THEN** validation fails pointing to that `area`

#### Scenario: Crop after tile
- **WHEN** a 4x8 sprite is tiled to 32x8 and then cropped with area `[20, 0, 12, 8]`
- **THEN** validation succeeds

#### Scenario: Crop outside the tiled area
- **WHEN** a 4x8 sprite is tiled to 16x8 and then cropped with area `[8, 0, 12, 8]`
- **THEN** validation fails pointing to that `area`

#### Scenario: Appended crop after an appended rotation
- **WHEN** a 4x8 sprite element gets `appendTransforms` `[rotate 90]` at 2 and `appendTransforms` `[crop [0, 0, 8, 4]]` at 4
- **THEN** validation succeeds

#### Scenario: Appended crop out of bounds
- **WHEN** a 4x8 sprite element gets `appendTransforms` `[crop [0, 0, 8, 4]]` at 4 and no rotation before it
- **THEN** validation fails pointing to the `area` of that transition's `crop`

#### Scenario: Reset restores the declared chain
- **WHEN** a 4x8 sprite element gets `appendTransforms` `[rotate 90]` at 2, `resetTransforms` at 3 and `appendTransforms` `[crop [0, 0, 8, 4]]` at 4
- **THEN** validation fails pointing to the `crop` added at 4, because the rotation was removed at 3
