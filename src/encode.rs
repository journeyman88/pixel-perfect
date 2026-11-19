//! Deterministic PNG encoding, indexed or RGBA depending on the color count.

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
    let mut seen: HashMap<Rgba, ()> = HashMap::new();
    let (mut translucent, mut opaque) = (Vec::new(), Vec::new());
    for &p in image.pixels() {
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

/// Encodes `image` as PNG bytes. Same image, same bytes.
pub fn png(image: &Image) -> Vec<u8> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, image.width(), image.height());
    encoder.set_compression(png::Compression::Balanced);
    encoder.set_filter(png::Filter::Adaptive);

    let data = match choose(image) {
        Encoding::Rgba => {
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            image.pixels().iter().flat_map(|p| p.0).collect::<Vec<u8>>()
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
            pack_indices(image, &index, bit_depth)
        }
    };

    let mut writer = encoder
        .write_header()
        .expect("writing to a Vec cannot fail");
    writer
        .write_image_data(&data)
        .expect("image data matches the header");
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
