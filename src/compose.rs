// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 The pixel-perfect Authors

//! Compositing: the source-over rule, drawing with clipping, and rendering of
//! static panels and scenes.

use crate::model::{ElementSource, Panel, Rgba, Scene};
use crate::raster::{self, Image, TRANSPARENT};
use crate::render;
use crate::semantic::ResolvedWorksheet;

/// `n / d` rounded half up: `floor((2n + d) / 2d)`.
fn round(n: u32, d: u32) -> u32 {
    (2 * n + d) / (2 * d)
}

/// Source-over of non-premultiplied 8-bit pixels, in integer arithmetic.
///
/// `A = sa·255 + da·(255 − sa)`; the alpha is `round(A, 255)` and each color
/// channel is `round(sc·sa·255 + dc·da·(255 − sa), A)`. `A = 0` gives
/// [`TRANSPARENT`]. Every intermediate value fits in `u32` (at most about
/// 3.4·10⁷).
pub fn source_over(src: Rgba, dst: Rgba) -> Rgba {
    let (sa, da) = (u32::from(src.0[3]), u32::from(dst.0[3]));
    let a = sa * 255 + da * (255 - sa);
    if a == 0 {
        return TRANSPARENT;
    }
    let channel = |i: usize| {
        let n = u32::from(src.0[i]) * sa * 255 + u32::from(dst.0[i]) * da * (255 - sa);
        round(n, a) as u8
    };
    Rgba([channel(0), channel(1), channel(2), round(a, 255) as u8])
}

/// [`source_over`] with shortcuts for the common cases. The shortcuts give
/// exactly the result of the full formula on canonical pixels.
pub fn blend(src: Rgba, dst: Rgba) -> Rgba {
    match (src.0[3], dst.0[3]) {
        (255, _) => src,
        (0, _) => dst,
        (_, 0) => src,
        _ => source_over(src, dst),
    }
}

/// Draws `image` on `canvas` with its top-left corner at `(x, y)`, blending
/// with [`blend`]. Pixels falling outside the canvas are discarded.
pub fn draw(canvas: &mut Image, image: &Image, x: i64, y: i64) {
    let (cw, ch) = (i64::from(canvas.width()), i64::from(canvas.height()));
    let x0 = x.clamp(0, cw);
    let y0 = y.clamp(0, ch);
    let x1 = x.saturating_add(i64::from(image.width())).clamp(0, cw);
    let y1 = y.saturating_add(i64::from(image.height())).clamp(0, ch);
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    let stride = cw as usize;
    let pixels = canvas.pixels_mut();
    for cy in y0..y1 {
        for cx in x0..x1 {
            let src = image.get((cx - x) as u32, (cy - y) as u32);
            let dst = &mut pixels[cy as usize * stride + cx as usize];
            *dst = blend(src, *dst);
        }
    }
}

/// Indices of `items` in drawing order: ascending `z`, ties in array order.
fn drawing_order<T>(items: &[T], z: impl Fn(&T) -> i64) -> Vec<usize> {
    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by_key(|&i| z(&items[i])); // stable: ties keep array order
    order
}

/// True if a visible element of the panel plays a flipbook.
pub fn is_animated_panel(panel: &Panel) -> bool {
    panel
        .elements
        .iter()
        .any(|e| e.visible && matches!(e.source(), ElementSource::Flipbook(_)))
}

/// True if a visible layer of the scene is a timeline or uses an animated
/// panel.
pub fn is_animated_scene(ws: &ResolvedWorksheet, scene: &Scene) -> bool {
    scene
        .layers
        .iter()
        .filter(|l| l.visible)
        .any(|layer| match &layer.panel {
            Some(id) => find_panel(ws, id).is_some_and(is_animated_panel),
            None => true, // `_timeline`
        })
}

pub(crate) fn find_panel<'a>(ws: &'a ResolvedWorksheet, id: &str) -> Option<&'a Panel> {
    ws.model.panels.iter().find(|p| p.id == id)
}

pub(crate) fn find_scene<'a>(ws: &'a ResolvedWorksheet, id: &str) -> Option<&'a Scene> {
    ws.model.scenes.iter().find(|s| s.id == id)
}

/// An image ready to be drawn: its top-left corner and its `zIndex`.
#[derive(Debug, Clone, Copy)]
pub struct Placed<'a> {
    pub image: &'a Image,
    pub coordinates: [i64; 2],
    pub z_index: i64,
}

/// Draws `items` on a transparent `width` x `height` canvas in ascending
/// `zIndex`, ties in slice order. Callers pass only visible items. Static and
/// animated panels and scenes all go through this function.
pub fn flatten(width: u32, height: u32, items: &[Placed]) -> Image {
    let mut canvas = Image::transparent(width, height);
    for i in drawing_order(items, |p| p.z_index) {
        let [x, y] = items[i].coordinates;
        draw(&mut canvas, items[i].image, x, y);
    }
    canvas
}

/// Renders the static panel `id`.
///
/// Panics if `id` is not a panel or the panel is animated; the export plan
/// only passes static panels.
pub fn panel(ws: &ResolvedWorksheet, id: &str) -> Image {
    let panel = find_panel(ws, id).unwrap_or_else(|| panic!("unknown panel {id:?}"));
    let visible: Vec<_> = panel.elements.iter().filter(|e| e.visible).collect();
    let images: Vec<Image> = visible
        .iter()
        .map(|element| {
            let ElementSource::Sprite(sprite) = element.source() else {
                unreachable!("panel {id:?} is animated");
            };
            raster::apply_all(&render::sprite(ws, sprite), &element.transforms)
        })
        .collect();
    let items: Vec<Placed> = visible
        .iter()
        .zip(&images)
        .map(|(e, image)| Placed {
            image,
            coordinates: e.coordinates,
            z_index: e.z_index,
        })
        .collect();
    flatten(panel.width, panel.height, &items)
}

/// Renders the static scene `id`: each visible panel layer is flattened on
/// its own canvas, then drawn on the scene. Hidden layers are skipped.
///
/// Panics if `id` is not a scene or the scene is animated.
pub fn scene(ws: &ResolvedWorksheet, id: &str) -> Image {
    let scene = find_scene(ws, id).unwrap_or_else(|| panic!("unknown scene {id:?}"));
    let layers: Vec<_> = scene.layers.iter().filter(|l| l.visible).collect();
    let images: Vec<Image> = layers
        .iter()
        .map(|layer| {
            let Some(panel_id) = &layer.panel else {
                unreachable!("scene {id:?} is animated");
            };
            panel(ws, panel_id)
        })
        .collect();
    let items: Vec<Placed> = layers
        .iter()
        .zip(&images)
        .map(|(l, image)| Placed {
            image,
            coordinates: l.coordinates,
            z_index: l.z_index,
        })
        .collect();
    flatten(scene.width, scene.height, &items)
}
