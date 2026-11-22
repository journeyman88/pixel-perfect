## MODIFIED Requirements

### Requirement: Ending watch
`watch` SHALL run until it is interrupted, for example with Ctrl+C.

The first interrupt SHALL let the render in progress, if any, finish, and
then end the command with exit code 0. As in `render`, `manifest.json` SHALL
be written only after every image of a render has been written. A command
ended by a first interrupt therefore leaves the complete output of its last
render, with a manifest that matches it.

A second interrupt, received while that render is still running, SHALL end
the command at once with exit code 130, without waiting for the render. This
lets the user stop a long render. The output on disk can then be
inconsistent:
- an image that was being written can be incomplete;
- some images can come from the interrupted render and others from the
  previous one;
- `manifest.json` is the previous render's, or is missing if no render had
  finished.

Running `render`, or `watch` again, to the same output path SHALL then give
a consistent output.

#### Scenario: Interrupt while idle
- **WHEN** `watch` is idle and receives an interrupt
- **THEN** it exits with code 0

#### Scenario: Interrupt during a render
- **WHEN** `watch` receives one interrupt while a render is in progress
- **THEN** the render finishes, `manifest.json` matches the files it wrote, and the command exits with code 0

#### Scenario: Second interrupt during a render
- **WHEN** `watch` receives a second interrupt while the render started before the first one is still in progress
- **THEN** the command exits at once with code 130, possibly leaving an incomplete image and the previous `manifest.json`

#### Scenario: Recovering after a second interrupt
- **WHEN** `render` is run to the same output path after a second interrupt
- **THEN** the output is complete again and `manifest.json` matches it
