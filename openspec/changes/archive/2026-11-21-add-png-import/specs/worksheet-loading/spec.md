## ADDED Requirements

### Requirement: Palette characters are visible
A palette `char` SHALL be a single character that is not `.`, `=`,
whitespace, a control character (`Cc`), a format character (`Cf`, for example
U+200B ZERO WIDTH SPACE or U+FEFF) or a combining mark (`M`: `Mn`, `Mc`,
`Me`). The schema SHALL express this as the pattern
`^[^.=\s\p{Cc}\p{Cf}\p{M}]$`, so that no character is invisible or merges
with its neighbour in `lines`.

**BREAKING**: worksheets using a format character or a combining mark as
`char` no longer validate. None of the examples or test fixtures do.

#### Scenario: Zero width space rejected
- **WHEN** a palette has `{ char: "​", rgba: [0, 0, 0] }`
- **THEN** validation fails with an error pointing to that entry's `char`

#### Scenario: Combining mark rejected
- **WHEN** a palette has `char` U+0301 COMBINING ACUTE ACCENT
- **THEN** validation fails with an error pointing to that entry's `char`

#### Scenario: Precomposed letter accepted
- **WHEN** a palette has `char` `é` (U+00E9, a single precomposed character)
- **THEN** validation succeeds

#### Scenario: Wide character still accepted
- **WHEN** a palette has `char` `山`
- **THEN** validation succeeds
