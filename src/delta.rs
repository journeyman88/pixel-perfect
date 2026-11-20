// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 The pixel-perfect Authors

//! Differences between consecutive animation frames, so that encoders can
//! write only the area that changes.

use crate::model::Rgba;
use crate::raster::{Image, TRANSPARENT};

/// A rectangle of pixels: top-left corner and size, never empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    /// The 1x1 rectangle at (0, 0): the frame written when nothing changed.
    pub const UNCHANGED: Rect = Rect {
        x: 0,
        y: 0,
        width: 1,
        height: 1,
    };

    /// The whole canvas of `image`.
    pub fn full(image: &Image) -> Rect {
        Rect {
            x: 0,
            y: 0,
            width: image.width(),
            height: image.height(),
        }
    }
}

/// The smallest rectangle containing every pixel that differs between `prev`
/// and `next`, or `None` if they are identical.
///
/// Panics if the images differ in size.
pub fn changed_rect(prev: &Image, next: &Image) -> Option<Rect> {
    assert_eq!(
        (prev.width(), prev.height()),
        (next.width(), next.height()),
        "frames have the canvas size"
    );
    let width = next.width() as usize;
    let mut bounds: Option<(usize, usize, usize, usize)> = None;
    let rows = prev.pixels().chunks(width).zip(next.pixels().chunks(width));
    for (y, (a, b)) in rows.enumerate() {
        let Some(left) = a.iter().zip(b).position(|(p, q)| p != q) else {
            continue;
        };
        let right = a.iter().zip(b).rposition(|(p, q)| p != q).unwrap();
        bounds = Some(match bounds {
            None => (left, y, right, y),
            Some((x0, y0, x1, _)) => (x0.min(left), y0, x1.max(right), y),
        });
    }
    bounds.map(|(x0, y0, x1, y1)| Rect {
        x: x0 as u32,
        y: y0 as u32,
        width: (x1 - x0 + 1) as u32,
        height: (y1 - y0 + 1) as u32,
    })
}

/// `rect` extended by one pixel to the left and/or top when its left or top
/// edge is odd, for formats that store offsets halved (WebP). The right and
/// bottom edges do not move, so the result stays inside the canvas.
pub fn align_even(rect: Rect) -> Rect {
    let (dx, dy) = (rect.x % 2, rect.y % 2);
    Rect {
        x: rect.x - dx,
        y: rect.y - dy,
        width: rect.width + dx,
        height: rect.height + dy,
    }
}

/// The pixels of `image` inside `rect`.
///
/// Panics if `rect` is not inside the image.
pub fn crop(image: &Image, rect: Rect) -> Image {
    assert!(
        rect.x + rect.width <= image.width() && rect.y + rect.height <= image.height(),
        "{rect:?} inside {}x{}",
        image.width(),
        image.height()
    );
    Image::from_fn(rect.width, rect.height, |x, y| {
        image.get(rect.x + x, rect.y + y)
    })
}

/// The masked version of `next` inside `rect`: pixels that differ from `prev`
/// keep their value in `next`, the others are `(0, 0, 0, 0)`. Blended OVER the
/// previous canvas it gives `next` exactly, but only if every changed pixel is
/// fully opaque; otherwise returns `None`.
pub fn masked(prev: &Image, next: &Image, rect: Rect) -> Option<Image> {
    let mut opaque = true;
    let image = Image::from_fn(rect.width, rect.height, |x, y| {
        let (px, py) = (rect.x + x, rect.y + y);
        let (old, new): (Rgba, Rgba) = (prev.get(px, py), next.get(px, py));
        if old == new {
            TRANSPARENT
        } else {
            opaque &= new.0[3] == 255;
            new
        }
    });
    opaque.then_some(image)
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: Rgba = Rgba([1, 2, 3, 255]);
    const B: Rgba = Rgba([9, 8, 7, 255]);
    const HALF: Rgba = Rgba([9, 8, 7, 128]);

    fn canvas(w: u32, h: u32, changes: &[(u32, u32, Rgba)]) -> Image {
        Image::from_fn(w, h, |x, y| {
            changes
                .iter()
                .find(|c| (c.0, c.1) == (x, y))
                .map_or(A, |c| c.2)
        })
    }

    fn rect(x: u32, y: u32, width: u32, height: u32) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn identical_frames_have_no_rect() {
        let a = canvas(4, 3, &[]);
        assert_eq!(changed_rect(&a, &a.clone()), None);
    }

    #[test]
    fn one_pixel() {
        let a = canvas(8, 6, &[]);
        let b = canvas(8, 6, &[(5, 3, B)]);
        assert_eq!(changed_rect(&a, &b), Some(rect(5, 3, 1, 1)));
    }

    #[test]
    fn opposite_corners() {
        let a = canvas(8, 6, &[]);
        let b = canvas(8, 6, &[(0, 5, B), (7, 0, B)]);
        assert_eq!(changed_rect(&a, &b), Some(rect(0, 0, 8, 6)));
        let b = canvas(8, 6, &[(6, 1, B), (2, 4, B), (3, 2, B)]);
        assert_eq!(changed_rect(&a, &b), Some(rect(2, 1, 5, 4)));
    }

    #[test]
    fn even_alignment() {
        assert_eq!(align_even(rect(5, 3, 1, 1)), rect(4, 2, 2, 2));
        assert_eq!(align_even(rect(4, 3, 2, 1)), rect(4, 2, 2, 2));
        assert_eq!(align_even(rect(0, 0, 3, 3)), rect(0, 0, 3, 3));
        // A changed pixel on the last column of an odd-width canvas: the right
        // edge does not move, so the rectangle stays inside.
        let r = align_even(rect(7, 0, 1, 1));
        assert_eq!(r, rect(6, 0, 2, 1));
        assert!(r.x + r.width <= 8);
    }

    #[test]
    fn crop_takes_the_rect() {
        let b = canvas(8, 6, &[(5, 3, B)]);
        let c = crop(&b, rect(4, 2, 2, 2));
        assert_eq!(c.pixels(), [A, A, A, B]);
    }

    #[test]
    fn masked_frames() {
        let a = canvas(4, 1, &[]);
        let b = canvas(4, 1, &[(1, 0, B), (3, 0, B)]);
        let m = masked(&a, &b, rect(1, 0, 3, 1)).unwrap();
        assert_eq!(m.pixels(), [B, TRANSPARENT, B]);
        // A changed pixel that is not opaque cannot be blended OVER.
        let c = canvas(4, 1, &[(1, 0, B), (3, 0, HALF)]);
        assert_eq!(masked(&a, &c, rect(1, 0, 3, 1)), None);
        let d = canvas(4, 1, &[(2, 0, TRANSPARENT)]);
        assert_eq!(masked(&a, &d, rect(2, 0, 1, 1)), None);
    }
}
