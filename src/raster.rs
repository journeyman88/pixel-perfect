//! RGBA images and the transform operations.

use crate::model::{Axis, Rgba, Rotation, Transform};

/// The fully transparent pixel. Every pixel with alpha 0 is stored as this.
pub const TRANSPARENT: Rgba = Rgba([0, 0, 0, 0]);

/// Returns `color`, or [`TRANSPARENT`] if its alpha is 0.
pub fn canonical(color: Rgba) -> Rgba {
    if color.0[3] == 0 { TRANSPARENT } else { color }
}

/// An RGBA8 image in row-major order. Fully transparent pixels are always
/// stored as [`TRANSPARENT`] (canonical transparency).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    width: u32,
    height: u32,
    pixels: Vec<Rgba>,
}

impl Image {
    /// Builds an image from row-major pixels, canonicalizing transparency.
    ///
    /// Panics if `pixels.len() != width * height`.
    pub fn new(width: u32, height: u32, mut pixels: Vec<Rgba>) -> Image {
        assert_eq!(
            pixels.len() as u64,
            u64::from(width) * u64::from(height),
            "pixel count does not match {width}x{height}"
        );
        for p in &mut pixels {
            *p = canonical(*p);
        }
        Image {
            width,
            height,
            pixels,
        }
    }

    /// Builds an image by calling `f(x, y)` for every pixel.
    pub fn from_fn(width: u32, height: u32, mut f: impl FnMut(u32, u32) -> Rgba) -> Image {
        let mut pixels = Vec::with_capacity(width as usize * height as usize);
        for y in 0..height {
            for x in 0..width {
                pixels.push(f(x, y));
            }
        }
        Image::new(width, height, pixels)
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// Pixels in row-major order.
    pub fn pixels(&self) -> &[Rgba] {
        &self.pixels
    }

    /// The pixel at `(x, y)`. Panics if out of bounds.
    pub fn get(&self, x: u32, y: u32) -> Rgba {
        assert!(
            x < self.width && y < self.height,
            "({x}, {y}) is outside the image"
        );
        self.pixels[y as usize * self.width as usize + x as usize]
    }

    /// Enlarges the image by `factor` in both directions (each pixel becomes a
    /// `factor` x `factor` block).
    pub fn upscale(&self, factor: u32) -> Image {
        if factor == 1 {
            return self.clone();
        }
        Image::from_fn(self.width * factor, self.height * factor, |x, y| {
            self.get(x / factor, y / factor)
        })
    }
}

/// Applies one transform. Bounds of `crop` are validated when the worksheet is
/// loaded; an out-of-bounds crop here is a bug and panics.
pub fn apply(image: &Image, transform: &Transform) -> Image {
    let (w, h) = (image.width, image.height);
    match *transform {
        Transform::Flip { axis: Axis::X } => Image::from_fn(w, h, |x, y| image.get(w - 1 - x, y)),
        Transform::Flip { axis: Axis::Y } => Image::from_fn(w, h, |x, y| image.get(x, h - 1 - y)),
        Transform::Rotate {
            degrees: Rotation::Deg90,
        } => Image::from_fn(h, w, |x, y| image.get(y, h - 1 - x)),
        Transform::Rotate {
            degrees: Rotation::Deg180,
        } => Image::from_fn(w, h, |x, y| image.get(w - 1 - x, h - 1 - y)),
        Transform::Rotate {
            degrees: Rotation::Deg270,
        } => Image::from_fn(h, w, |x, y| image.get(w - 1 - y, x)),
        Transform::Scale { width, height } => Image::from_fn(width, height, |x, y| {
            let sx = u64::from(x) * u64::from(w) / u64::from(width);
            let sy = u64::from(y) * u64::from(h) / u64::from(height);
            image.get(sx as u32, sy as u32)
        }),
        Transform::Crop {
            area: [cx, cy, cw, ch],
        } => {
            assert!(
                u64::from(cx) + u64::from(cw) <= u64::from(w)
                    && u64::from(cy) + u64::from(ch) <= u64::from(h),
                "crop area [{cx}, {cy}, {cw}, {ch}] outside {w}x{h} image"
            );
            Image::from_fn(cw, ch, |x, y| image.get(cx + x, cy + y))
        }
    }
}

/// Applies `transforms` in array order.
pub fn apply_all(image: &Image, transforms: &[Transform]) -> Image {
    transforms
        .iter()
        .fold(image.clone(), |img, t| apply(&img, t))
}
