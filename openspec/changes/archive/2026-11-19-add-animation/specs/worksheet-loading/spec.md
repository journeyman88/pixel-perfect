## RENAMED Requirements

- FROM: `### Requirement: Static transforms stay within bounds`
- TO: `### Requirement: Transforms stay within bounds`

## MODIFIED Requirements

### Requirement: Transforms stay within bounds
Each `crop` area SHALL lie inside the image as it is at that point of the
`transforms` chain, taking previous `rotate`, `scale` and `crop` steps into
account. This SHALL hold for the chains declared on flipbook frames and panel
elements, and for every chain an element can have during a timeline: its own
`transforms` followed by the `transforms` of the `appendTransforms` applied
since the last `resetTransforms`, in the order in which the transitions are
applied (by `at`, ties in array order). An error in an appended `crop` SHALL
point to the `area` inside that `appendTransforms` transition.

#### Scenario: Crop after rotation
- **WHEN** a 4x8 sprite is rotated by 90 degrees and then cropped with area `[0, 0, 8, 4]`
- **THEN** validation succeeds

#### Scenario: Crop out of bounds
- **WHEN** a 4x8 sprite is cropped with area `[2, 0, 4, 8]`
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

## ADDED Requirements

### Requirement: Timeline transitions are consistent
The engine SHALL check the transitions of every timeline:
- `at` and `start` SHALL be less than the timeline's `duration`, and
  `start + duration` of a `moveTo` SHALL be at most the timeline's `duration`;
- two `moveTo` on the same element SHALL NOT overlap: when ordered by `start`,
  each SHALL start at or after `start + duration` of the previous one;
- a `seekFlipbook` SHALL target an element that has `_flipbook`, and its
  `frameIndex` SHALL be less than the number of `frames` of that flipbook.

Each error SHALL point to the offending field of the transition.

#### Scenario: Instant transition after the end
- **WHEN** a timeline with `duration` 40 has a `setVisible` with `at` 40
- **THEN** validation fails pointing to that `at`

#### Scenario: Movement ending at the loop boundary
- **WHEN** a timeline with `duration` 40 has a `moveTo` with `start` 30 and `duration` 10
- **THEN** validation succeeds

#### Scenario: Movement past the end
- **WHEN** a timeline with `duration` 40 has a `moveTo` with `start` 35 and `duration` 10
- **THEN** validation fails pointing to that transition's `duration`

#### Scenario: Overlapping movements
- **WHEN** element `hero` has `moveTo` with `start` 0 and `duration` 10, and another with `start` 5
- **THEN** validation fails pointing to the `start` of the second one

#### Scenario: Back-to-back movements
- **WHEN** element `hero` has `moveTo` with `start` 0 and `duration` 10, and another with `start` 10
- **THEN** validation succeeds

#### Scenario: Seek on a sprite element
- **WHEN** a `seekFlipbook` targets element `hero`, which has `_sprite`
- **THEN** validation fails pointing to that `_element`

#### Scenario: Frame index out of range
- **WHEN** a `seekFlipbook` has `frameIndex` 4 and its element's flipbook has 4 frames
- **THEN** validation fails pointing to that `frameIndex`
