# Examples

Worksheets to try the engine with. Render one with:

```sh
pixel-perfect render examples/sprites.yaml -o out --scale 8
```

| Worksheet | What it shows |
|---|---|
| [`sprites.yaml`](sprites.yaml) | Sprites only, one for each PNG encoding: indexed at 1, 2, 4 and 8 bits per pixel, and RGBA for an image with more than 256 colors. Also semi-transparent colors, palette inheritance and a derived sprite. |

The header comment of each worksheet lists what it produces. The test
`tests/examples.rs` renders every example, so they stay valid as the engine
evolves.
