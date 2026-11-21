# Examples

Worksheets to try the engine with. Render one with:

```sh
pixel-perfect render examples/sprites.yaml -o out --scale 8
```

| Worksheet | What it shows |
|---|---|
| [`sprites.yaml`](sprites.yaml) | Sprites only, one for each PNG encoding: indexed at 1, 2, 4 and 8 bits per pixel, and RGBA for an image with more than 256 colors. Also semi-transparent colors, palette inheritance and a derived sprite. |
| [`composition.yaml`](composition.yaml) | A static scene of three panels: placement and clipping, `zIndex`, element transforms, a hidden element, and semi-transparent mist and shadows blended into colors that are in no palette. |
| [`masquerade.yaml`](masquerade.yaml) | A 256x256 masked ball built to use the whole format: derived palettes dress the same dancers in other colors, `overrideLines` change plumes and masks, every transform kind except `tile` is used (columns and chains stretched from single rows, mirrored curtains, a cropped half moon, rotated confetti), one wallpaper panel is stacked nine times as scene layers, and semi-transparent glow and moonlight blend over the hall. |
| [`animation.yaml`](animation.yaml) | Animations written as APNG: a walk-cycle flipbook used as an element, a timeline that walks there and back in a seamless loop (`moveTo`, `appendTransforms` to face left, `seekFlipbook` on the turn, `setVisible` for a speech bubble) over a cobblestone path made with one `tile` transform, a flickering torch at another fps, and a scene that combines both clocks. |
| [`campfire.yaml`](campfire.yaml) | A 256x256 animated campfire at night that uses the whole format: ten scene layers from the sky to a vignette, every transform kind (sky, stars, clouds, forest rows and grass are each one `tile`), derived palettes for aerial perspective, a 6-frame flames flipbook (one frame derived with `overrideLines` and `=`, one flipped), and a fire timeline that uses every transition kind, next to a twinkling-stars timeline at another fps. |

[`slime.png`](slime.png) is the `slime` sprite of `sprites.yaml` rendered at
`--scale 4`. Import it back to see that the round trip is exact:

```sh
# Prints sprites.yaml with a new sprite "slime-copy" whose lines equal those
# of "slime"; --dry-run leaves the file untouched.
pixel-perfect import examples/sprites.yaml examples/slime.png \
  --id slime-copy --palette slime --scale 4 --dry-run
```

Without `--palette slime`, the importer also creates a palette `slime-copy`,
with characters `a`, `b`, `c`... in the order the colors first appear.

The header comment of each worksheet lists what it produces. The test
`tests/examples.rs` renders every example, so they stay valid as the engine
evolves.
