## Why

`watch` handles two interrupts differently. The first lets the render in
progress finish and exits with code 0. A second interrupt, sent while that
render is still running, exits at once with code 130. The README describes
this, but the spec only covers the first interrupt. Its statement that an
interrupted render never leaves an inconsistent manifest does not hold for
the second one.

## What Changes

- *Ending watch* states what each interrupt does:
  - the first finishes the render in progress and exits with code 0;
  - a second, during that render, exits immediately with code 130.
- It also states what a second interrupt can leave on disk: a partly written
  image, a mix of old and new images, and the previous render's
  `manifest.json` (or none). It says how to get a consistent output again.
- The code does not change: it already behaves this way.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `watch-command`: the *Ending watch* requirement covers the second
  interrupt, with new scenarios.

## Impact

Only the spec changes. Code, tests and the README are left as they are; the
README already says "A second Ctrl+C stops at once, without waiting for the
render in progress".
