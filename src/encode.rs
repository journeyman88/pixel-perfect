//! Deterministic PNG and APNG encoding, indexed or RGBA depending on the
//! color count.

use std::collections::HashMap;

use crate::model::Rgba;
use crate::raster::Image;

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
    let mut seen: HashMap<Rgba, ()> = HashMap::new();
    let (mut translucent, mut opaque) = (Vec::new(), Vec::new());
    for &p in images.into_iter().flat_map(|image| image.pixels()) {
        if seen.insert(p, ()).is_some() {
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
    indexed: Option<(HashMap<Rgba, u8>, u8)>,
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
            let index: HashMap<Rgba, u8> = palette
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
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, image.width(), image.height());
    let data = configure(&mut encoder, choose(image));
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

/// Encodes an animation as APNG bytes: infinite loop, frame 0 as the default
/// image, every frame on the full canvas with `dispose_op` NONE and `blend_op`
/// SOURCE, one palette for all frames. Same frames, same bytes.
///
/// Panics if `frames` is empty or the images differ in size.
pub fn apng(frames: &[ApngFrame]) -> Vec<u8> {
    let first = frames
        .first()
        .expect("an animation has at least one frame")
        .image;
    let (width, height) = (first.width(), first.height());
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, width, height);
    let data = configure(&mut encoder, choose_all(frames.iter().map(|f| f.image)));
    encoder
        .set_animated(frames.len() as u32, 0)
        .expect("at least one frame");
    let mut writer = encoder
        .write_header()
        .expect("writing to a Vec cannot fail");
    for frame in frames {
        assert_eq!(
            (frame.image.width(), frame.image.height()),
            (width, height),
            "every frame has the canvas size"
        );
        writer
            .set_frame_delay(frame.delay.0, frame.delay.1)
            .expect("animated writer");
        writer
            .set_dispose_op(png::DisposeOp::None)
            .expect("animated writer");
        writer
            .set_blend_op(png::BlendOp::Source)
            .expect("animated writer");
        writer
            .write_image_data(&data.of(frame.image))
            .expect("image data matches the header");
    }
    writer.finish().expect("writing to a Vec cannot fail");
    out
}

/// Packs palette indices into rows of `bit_depth` bits per pixel, most
/// significant bits first, each row padded to a whole byte.
fn pack_indices(image: &Image, index: &HashMap<Rgba, u8>, bit_depth: u8) -> Vec<u8> {
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
