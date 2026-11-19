# Examples

Worksheets to try the engine with. Render one with:

```sh
pixel-perfect render examples/sprites.yaml -o out --scale 8
```

| Worksheet | What it shows |
|---|---|
| [`sprites.yaml`](sprites.yaml) | Sprites only, one for each PNG encoding: indexed at 1, 2, 4 and 8 bits per pixel, and RGBA for an image with more than 256 colors. Also semi-transparent colors, palette inheritance and a derived sprite. |
| [`composition.yaml`](composition.yaml) | A static scene of three panels: placement and clipping, `zIndex`, element transforms, a hidden element, and semi-transparent mist and shadows blended into colors that are in no palette. |
| [`masquerade.yaml`](masquerade.yaml) | A 256x256 masked ball built to use the whole format: derived palettes dress the same dancers in other colors, `overrideLines` change plumes and masks, every transform kind is used (columns and chains stretched from single rows, mirrored curtains, a cropped half moon, rotated confetti), one wallpaper panel is stacked nine times as scene layers, and semi-transparent glow and moonlight blend over the hall. |

The header comment of each worksheet lists what it produces. The test
`tests/examples.rs` renders every example, so they stay valid as the engine
evolves.
