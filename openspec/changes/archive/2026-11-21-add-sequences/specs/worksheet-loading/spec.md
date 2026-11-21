## MODIFIED Requirements

### Requirement: Unique ids
Ids SHALL be unique among resources of the same type, element `id`s SHALL be
unique inside their panel, and layer `id`s SHALL be unique inside their scene.
The same id MAY be reused across different resource types, and layers without
`id` SHALL NOT conflict.

#### Scenario: Duplicate sprite id
- **WHEN** two sprites both have `id` `hero`
- **THEN** validation fails pointing to the second one

#### Scenario: Same id in different collections
- **WHEN** a palette and a sprite both have `id` `hero`
- **THEN** validation succeeds

#### Scenario: Duplicate layer id
- **WHEN** two layers of scene `night` both have `id` `lamp`
- **THEN** validation fails pointing to the `id` of the second one

#### Scenario: Same layer id in different scenes
- **WHEN** scenes `night` and `day` both have a layer with `id` `lamp`
- **THEN** validation succeeds

### Requirement: References resolve to the expected type
References SHALL resolve: every `_palette`, `_sprite`, `_flipbook`, `_panel`,
`_timeline` and `_scene` field SHALL name an existing resource of that type. Every transition `_element`
SHALL name an element `id` of the timeline's `_panel`. Every action `_layer`
SHALL name a layer `id` of the sequence's `_scene`.

#### Scenario: Missing palette
- **WHEN** a sprite has `_palette` set to `night` and no palette `night` exists
- **THEN** validation fails pointing to that `_palette`

#### Scenario: Unknown element in a transition
- **WHEN** a timeline transition has `_element` set to `door` and its panel has no element with `id` `door`
- **THEN** validation fails pointing to that `_element`

#### Scenario: Unknown scene
- **WHEN** a sequence has `_scene` set to `night` and no scene `night` exists
- **THEN** validation fails pointing to that `_scene`

#### Scenario: Unknown layer in an action
- **WHEN** a sequence action has `_layer` set to `lamp` and its scene has no layer with `id` `lamp`
- **THEN** validation fails pointing to that `_layer`

## ADDED Requirements

### Requirement: Sequence actions are consistent
The engine SHALL check the actions of every sequence:
- `at` and `start` SHALL be less than the sequence's `duration`, and
  `start + duration` of a `moveTo` SHALL be at most the sequence's `duration`;
- two `moveTo` on the same layer SHALL NOT overlap: when ordered by `start`,
  each SHALL start at or after `start + duration` of the previous one.

Each error SHALL point to the offending field of the action.

#### Scenario: Instant action after the end
- **WHEN** a sequence with `duration` 16 has a `setVisible` with `at` 16
- **THEN** validation fails pointing to that `at`

#### Scenario: Movement ending at the loop boundary
- **WHEN** a sequence with `duration` 16 has a `moveTo` with `start` 8 and `duration` 8
- **THEN** validation succeeds

#### Scenario: Movement past the end
- **WHEN** a sequence with `duration` 16 has a `moveTo` with `start` 10 and `duration` 8
- **THEN** validation fails pointing to that action's `duration`

#### Scenario: Overlapping movements
- **WHEN** layer `far` has `moveTo` with `start` 0 and `duration` 8, and another with `start` 4
- **THEN** validation fails pointing to the `start` of the second one

#### Scenario: Removed action kinds
- **WHEN** a sequence action has `kind` `appendTransforms`
- **THEN** validation fails pointing to that `kind`
