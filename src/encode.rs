// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 The pixel-perfect Authors

//! Deterministic PNG and APNG encoding, indexed or RGBA depending on the
//! color count.

use std::collections::{HashMap, HashSet};

use foldhash::fast::FixedState;

use crate::delta::{self, Rect};
use crate::model::Rgba;
use crate::raster::{Image, TRANSPARENT};

/// Hash maps keyed by color, looked up once or more per pixel. The default
/// SipHash is built to resist hash flooding, which does not apply to pixels,
/// and dominated encoding time; foldhash with a fixed seed is much faster and
/// keeps every run identical.
type ColorMap<V> = HashMap<Rgba, V, FixedState>;
type ColorSet = HashSet<Rgba, FixedState>;

/// Most colors an indexed PNG can hold.
pub const MAX_PALETTE: usize = 256;

/// How an image is stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Encoding {
    /// Color type 3. `palette` lists the non-opaque colors first, then the
    /// opaque ones, each group by first appearance in row-major order.
    Indexed {
        palette: Vec<Rgba>,
        /// Number of leading palette entries with alpha below 255
        /// (the length of `tRNS`).
        transparent: usize,
        bit_depth: u8,
    },
    /// Color type 6, 8 bits per channel.
    Rgba,
}

/// Chooses the encoding of `image` from its distinct colors.
pub fn choose(image: &Image) -> Encoding {
    choose_all(std::iter::once(image))
}

/// Chooses one encoding for several images (the frames of an animation, in
/// order) from their distinct colors together. Colors are taken by first
/// appearance scanning the images in order, each in row-major order.
pub fn choose_all<'a>(images: impl IntoIterator<Item = &'a Image>) -> Encoding {
    let mut seen = ColorSet::default();
    let (mut translucent, mut opaque) = (Vec::new(), Vec::new());
    for &p in images.into_iter().flat_map(|image| image.pixels()) {
        if !seen.insert(p) {
            continue;
        }
        if seen.len() > MAX_PALETTE {
            return Encoding::Rgba;
        }
        if p.0[3] < 255 {
            translucent.push(p);
        } else {
            opaque.push(p);
        }
    }
    let transparent = translucent.len();
    let mut palette = translucent;
    palette.extend(opaque);
    let bit_depth = match palette.len() {
        0..=2 => 1,
        3..=4 => 2,
        5..=16 => 4,
        _ => 8,
    };
    Encoding::Indexed {
        palette,
        transparent,
        bit_depth,
    }
}

/// Turns images into encoded pixel rows for one [`Encoding`].
struct PixelRows {
    /// Palette index of each color and the bit depth, when indexed.
    indexed: Option<(ColorMap<u8>, u8)>,
}

impl PixelRows {
    fn of(&self, image: &Image) -> Vec<u8> {
        match &self.indexed {
            None => image.pixels().iter().flat_map(|p| p.0).collect(),
            Some((index, bit_depth)) => pack_indices(image, index, *bit_depth),
        }
    }
}

/// Sets color type, bit depth, `PLTE` and `tRNS` on `encoder`.
fn configure<W: std::io::Write>(encoder: &mut png::Encoder<W>, encoding: Encoding) -> PixelRows {
    encoder.set_compression(png::Compression::Balanced);
    encoder.set_filter(png::Filter::Adaptive);
    let indexed = match encoding {
        Encoding::Rgba => {
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            None
        }
        Encoding::Indexed {
            palette,
            transparent,
            bit_depth,
        } => {
            encoder.set_color(png::ColorType::Indexed);
            encoder.set_depth(match bit_depth {
                1 => png::BitDepth::One,
                2 => png::BitDepth::Two,
                4 => png::BitDepth::Four,
                _ => png::BitDepth::Eight,
            });
            let plte: Vec<u8> = palette
                .iter()
                .flat_map(|p| [p.0[0], p.0[1], p.0[2]])
                .collect();
            encoder.set_palette(plte);
            if transparent > 0 {
                let trns: Vec<u8> = palette[..transparent].iter().map(|p| p.0[3]).collect();
                encoder.set_trns(trns);
            }
            let index: ColorMap<u8> = palette
                .iter()
                .enumerate()
                .map(|(i, &c)| (c, i as u8))
                .collect();
            Some((index, bit_depth))
        }
    };
    PixelRows { indexed }
}

/// Encodes `image` as PNG bytes. Same image, same bytes.
pub fn png(image: &Image) -> Vec<u8> {
    png_with(image, choose(image))
}

/// Encodes `image` as PNG bytes with a given `encoding`, e.g. one chosen with
/// [`choose_all`] for several images that must share a palette. An indexed
/// `encoding` must contain every color of `image`.
pub fn png_with(image: &Image, encoding: Encoding) -> Vec<u8> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, image.width(), image.height());
    let data = configure(&mut encoder, encoding);
    let mut writer = encoder
        .write_header()
        .expect("writing to a Vec cannot fail");
    writer
        .write_image_data(&data.of(image))
        .expect("image data matches the header");
    writer.finish().expect("writing to a Vec cannot fail");
    out
}

/// One APNG frame: the image and its delay `num / den` seconds.
#[derive(Debug, Clone, Copy)]
pub struct ApngFrame<'a> {
    pub image: &'a Image,
    pub delay: (u16, u16),
}

/// How one APNG frame after the first is written.
struct FramePart {
    rect: Rect,
    image: Image,
    blend: png::BlendOp,
}

/// Size of `image` encoded alone as a PNG with `encoding`: a measure of its
/// compressed data, comparable between images of the same size.
fn encoded_len(image: &Image, encoding: &Encoding) -> usize {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, image.width(), image.height());
    let data = configure(&mut encoder, encoding.clone());
    let mut writer = encoder
        .write_header()
        .expect("writing to a Vec cannot fail");
    writer
        .write_image_data(&data.of(image))
        .expect("image data matches the header");
    writer.finish().expect("writing to a Vec cannot fail");
    out.len()
}

/// `rect` extended to the left edge for indexed APNGs at 2 or 4 bits per
/// pixel: FFmpeg (8.1) decodes such sub-frames wrongly when their x offset is
/// not 0, although the files are valid. Other encodings are left as they are.
fn ffmpeg_safe(rect: Rect, encoding: &Encoding) -> Rect {
    match encoding {
        Encoding::Indexed {
            bit_depth: 2 | 4, ..
        } => Rect {
            x: 0,
            width: rect.x + rect.width,
            ..rect
        },
        _ => rect,
    }
}

/// The part of `next` to write after `prev`: the changed area with blend
/// SOURCE, or, when every changed pixel is opaque and the encoding can store
/// `(0, 0, 0, 0)`, the masked area with blend OVER if it compresses smaller.
fn frame_part(prev: &Image, next: &Image, encoding: &Encoding) -> FramePart {
    let Some(rect) = delta::changed_rect(prev, next).map(|r| ffmpeg_safe(r, encoding)) else {
        return FramePart {
            rect: Rect::UNCHANGED,
            image: delta::crop(next, Rect::UNCHANGED),
            blend: png::BlendOp::Source,
        };
    };
    let area = FramePart {
        rect,
        image: delta::crop(next, rect),
        blend: png::BlendOp::Source,
    };
    let can_mask = match encoding {
        Encoding::Rgba => true,
        Encoding::Indexed { palette, .. } => palette.contains(&TRANSPARENT),
    };
    if !can_mask {
        return area;
    }
    match delta::masked(prev, next, rect) {
        Some(masked) if encoded_len(&masked, encoding) < encoded_len(&area.image, encoding) => {
            FramePart {
                rect,
                image: masked,
                blend: png::BlendOp::Over,
            }
        }
        _ => area,
    }
}

/// Encodes an animation as APNG bytes: infinite loop, frame 0 as the default
/// image on the full canvas, one palette for all frames, `dispose_op` NONE.
/// Every later frame covers only the area that changed since the previous
/// one (1x1 at (0, 0) when nothing changed), either overwriting it (`blend_op`
/// SOURCE) or, when smaller and exact, blending a masked area OVER it (see
/// [`delta::masked`]). Same frames, same bytes.
///
/// Panics if `frames` is empty or the images differ in size.
pub fn apng(frames: &[ApngFrame]) -> Vec<u8> {
    let first = frames
        .first()
        .expect("an animation has at least one frame")
        .image;
    let (width, height) = (first.width(), first.height());
    let encoding = choose_all(frames.iter().map(|f| f.image));
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, width, height);
    let data = configure(&mut encoder, encoding.clone());
    encoder
        .set_animated(frames.len() as u32, 0)
        .expect("at least one frame");
    let mut writer = encoder
        .write_header()
        .expect("writing to a Vec cannot fail");
    for (i, frame) in frames.iter().enumerate() {
        assert_eq!(
            (frame.image.width(), frame.image.height()),
            (width, height),
            "every frame has the canvas size"
        );
        let part = match i {
            0 => FramePart {
                rect: Rect::full(frame.image),
                image: frame.image.clone(),
                blend: png::BlendOp::Source,
            },
            _ => frame_part(frames[i - 1].image, frame.image, &encoding),
        };
        // Position (0, 0) first, so that every size fits, then the size, then
        // the real position.
        writer.reset_frame_position().expect("animated writer");
        writer
            .set_frame_dimension(part.rect.width, part.rect.height)
            .expect("the rectangle is inside the canvas");
        writer
            .set_frame_position(part.rect.x, part.rect.y)
            .expect("the rectangle is inside the canvas");
        writer
            .set_frame_delay(frame.delay.0, frame.delay.1)
            .expect("animated writer");
        writer
            .set_dispose_op(png::DisposeOp::None)
            .expect("animated writer");
        writer.set_blend_op(part.blend).expect("animated writer");
        writer
            .write_image_data(&data.of(&part.image))
            .expect("image data matches the frame size");
    }
    writer.finish().expect("writing to a Vec cannot fail");
    out
}

/// Packs palette indices into rows of `bit_depth` bits per pixel, most
/// significant bits first, each row padded to a whole byte.
fn pack_indices(image: &Image, index: &ColorMap<u8>, bit_depth: u8) -> Vec<u8> {
    let per_byte = 8 / bit_depth as usize;
    let row_bytes = (image.width() as usize).div_ceil(per_byte);
    let mut data = vec![0u8; row_bytes * image.height() as usize];
    for (row, pixels) in image.pixels().chunks(image.width() as usize).enumerate() {
        let out = &mut data[row * row_bytes..(row + 1) * row_bytes];
        for (x, p) in pixels.iter().enumerate() {
            let shift = 8 - bit_depth as usize * (x % per_byte + 1);
            out[x / per_byte] |= index[p] << shift;
        }
    }
    data
}
