// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 The pixel-perfect Authors

//! Deterministic PNG and APNG encoding, indexed or RGBA depending on the
//! color count.

use std::collections::{HashMap, HashSet};

use foldhash::fast::FixedState;
use rayon::prelude::*;

use crate::delta::{self, Rect};
use crate::model::Rgba;
use crate::raster::{Image, TRANSPARENT};

/// Hash maps keyed by color, looked up once or more per pixel. The default
/// SipHash is built to resist hash flooding, which does not apply to pixels,
/// and dominated encoding time; foldhash with a fixed seed is much faster and
/// keeps every run identical.
type ColorMap<V> = HashMap<Rgba, V, FixedState>;
type ColorSet = HashSet<Rgba, FixedState>;

/// How many frames are prepared in parallel before being consumed in order:
/// one per worker thread. Working in batches keeps the extra memory to a few
/// frames, instead of every frame of an animation at once; larger batches
/// were measured to add memory without saving time.
pub(crate) fn parallel_batch() -> usize {
    rayon::current_num_threads().max(1)
}

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
    // Pixel art is made of runs of one color: a pixel equal to the previous
    // one is already in `seen`, so most pixels skip the hash lookup.
    let mut last = None;
    for &p in images.into_iter().flat_map(|image| image.pixels()) {
        if last == Some(p) {
            continue;
        }
        last = Some(p);
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

/// One APNG frame, ready to be written: where it goes, how it blends, and
/// its data, a complete zlib stream of its own.
struct EncodedPart {
    rect: Rect,
    blend: png::BlendOp,
    zlib: Vec<u8>,
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

/// The part of `next` to write after `prev` (the whole of `next` for the
/// first frame), already compressed: the changed area with blend SOURCE, or,
/// when every changed pixel is opaque and the encoding can store
/// `(0, 0, 0, 0)`, the masked area with blend OVER if it compresses smaller.
///
/// The data comes from a standalone PNG of the part with the same encoding:
/// the `png` crate compresses an APNG frame exactly as it compresses that
/// image alone (a fresh zlib stream over the filtered rows). When both ways
/// are tried, the smaller PNG is kept as is instead of being compressed again.
fn encoded_part(prev: Option<&Image>, next: &Image, encoding: &Encoding) -> EncodedPart {
    let source = |rect: Rect, png: &[u8]| EncodedPart {
        rect,
        blend: png::BlendOp::Source,
        zlib: idat_data(png),
    };
    let Some(prev) = prev else {
        return source(Rect::full(next), &png_with(next, encoding.clone()));
    };
    let Some(rect) = delta::changed_rect(prev, next).map(|r| ffmpeg_safe(r, encoding)) else {
        let unchanged = delta::crop(next, Rect::UNCHANGED);
        return source(Rect::UNCHANGED, &png_with(&unchanged, encoding.clone()));
    };
    let area = png_with(&delta::crop(next, rect), encoding.clone());
    let can_mask = match encoding {
        Encoding::Rgba => true,
        Encoding::Indexed { palette, .. } => palette.contains(&TRANSPARENT),
    };
    if can_mask && let Some(masked) = delta::masked(prev, next, rect) {
        let masked = png_with(&masked, encoding.clone());
        // Changed area on a tie.
        if masked.len() < area.len() {
            return EncodedPart {
                rect,
                blend: png::BlendOp::Over,
                zlib: idat_data(&masked),
            };
        }
    }
    source(rect, &area)
}

/// The `IDAT` payloads of a PNG, concatenated: its zlib stream.
fn idat_data(png: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut at = 8; // after the signature
    while at + 12 <= png.len() {
        let len = u32::from_be_bytes(png[at..at + 4].try_into().expect("4 bytes")) as usize;
        if &png[at + 4..at + 8] == b"IDAT" {
            out.extend_from_slice(&png[at + 8..at + 8 + len]);
        }
        at += 12 + len;
    }
    out
}

/// Appends a PNG chunk: length, type, data and the CRC-32 of type and data.
fn write_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    let len = u32::try_from(data.len()).expect("chunk data fits a PNG chunk");
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut crc = crc32fast::Hasher::new();
    crc.update(kind);
    crc.update(data);
    out.extend_from_slice(&crc.finalize().to_be_bytes());
}

/// Largest `IDAT` payload, as the `png` crate writes them: 2^31 - 1 bytes.
const MAX_IDAT_DATA: usize = (u32::MAX >> 1) as usize;
/// Largest `fdAT` payload after its 4-byte sequence number: 2^31 - 5 bytes.
const MAX_FDAT_DATA: usize = MAX_IDAT_DATA - 4;

/// `data` split into chunk payloads of at most `max` bytes, as the `png`
/// crate splits them. A zlib stream is never empty, so every frame gets at
/// least one data chunk.
fn split_chunks(data: &[u8], max: usize) -> Vec<&[u8]> {
    data.chunks(max).collect()
}

/// The `IEND` chunk: always these 12 bytes.
const IEND: [u8; 12] = [0, 0, 0, 0, b'I', b'E', b'N', b'D', 0xAE, 0x42, 0x60, 0x82];

/// The APNG header as the `png` crate writes it for `encoding` and
/// `frame_count` frames: signature, `IHDR`, `acTL`, then `PLTE` and `tRNS`
/// when indexed.
fn apng_header(width: u32, height: u32, encoding: &Encoding, frame_count: u32) -> Vec<u8> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, width, height);
    configure(&mut encoder, encoding.clone());
    encoder
        .set_animated(frame_count, 0)
        .expect("at least one frame");
    let writer = encoder
        .write_header()
        .expect("writing to a Vec cannot fail");
    // An unfinished writer appends IEND when dropped: remove it.
    drop(writer);
    assert!(
        out.ends_with(&IEND),
        "the png crate ends its output with IEND"
    );
    out.truncate(out.len() - IEND.len());
    out
}

/// Encodes an animation as APNG bytes: infinite loop, frame 0 as the default
/// image on the full canvas, one palette for all frames, `dispose_op` NONE.
/// Every later frame covers only the area that changed since the previous
/// one (1x1 at (0, 0) when nothing changed), either overwriting it (`blend_op`
/// SOURCE) or, when smaller and exact, blending a masked area OVER it (see
/// [`delta::masked`]). Same frames, same bytes.
///
/// The header comes from the `png` crate; the frame chunks are written here.
/// Each frame is compressed once, all frames in parallel (see
/// [`encoded_part`]), which the crate's writer, compressing frame by frame on
/// one thread, could not do. The bytes are the same as the writer's.
///
/// Panics if `frames` is empty or the images differ in size.
pub fn apng(frames: &[ApngFrame]) -> Vec<u8> {
    let first = frames
        .first()
        .expect("an animation has at least one frame")
        .image;
    let (width, height) = (first.width(), first.height());
    for frame in frames {
        assert_eq!(
            (frame.image.width(), frame.image.height()),
            (width, height),
            "every frame has the canvas size"
        );
    }
    let encoding = choose_all(frames.iter().map(|f| f.image));
    let frame_count = u32::try_from(frames.len()).expect("at most 2^31 - 1 frames");
    let mut out = apng_header(width, height, &encoding, frame_count);
    // Only the compressed data is kept, so memory grows by the size of the
    // file, not by raw frames.
    let parts: Vec<EncodedPart> = (0..frames.len())
        .into_par_iter()
        .map(|i| {
            let prev = i.checked_sub(1).map(|p| frames[p].image);
            encoded_part(prev, frames[i].image, &encoding)
        })
        .collect();
    let mut sequence: u32 = 0;
    for (i, (frame, part)) in frames.iter().zip(&parts).enumerate() {
        let mut fctl = Vec::with_capacity(26);
        fctl.extend_from_slice(&sequence.to_be_bytes());
        fctl.extend_from_slice(&part.rect.width.to_be_bytes());
        fctl.extend_from_slice(&part.rect.height.to_be_bytes());
        fctl.extend_from_slice(&part.rect.x.to_be_bytes());
        fctl.extend_from_slice(&part.rect.y.to_be_bytes());
        fctl.extend_from_slice(&frame.delay.0.to_be_bytes());
        fctl.extend_from_slice(&frame.delay.1.to_be_bytes());
        fctl.push(png::DisposeOp::None as u8);
        fctl.push(part.blend as u8);
        write_chunk(&mut out, b"fcTL", &fctl);
        sequence += 1;
        if i == 0 {
            for data in split_chunks(&part.zlib, MAX_IDAT_DATA) {
                write_chunk(&mut out, b"IDAT", data);
            }
        } else {
            for data in split_chunks(&part.zlib, MAX_FDAT_DATA) {
                let mut fdat = Vec::with_capacity(4 + data.len());
                fdat.extend_from_slice(&sequence.to_be_bytes());
                fdat.extend_from_slice(data);
                write_chunk(&mut out, b"fdAT", &fdat);
                sequence += 1;
            }
        }
    }
    out.extend_from_slice(&IEND);
    out
}

/// Packs palette indices into rows of `bit_depth` bits per pixel, most
/// significant bits first, each row padded to a whole byte.
fn pack_indices(image: &Image, index: &ColorMap<u8>, bit_depth: u8) -> Vec<u8> {
    let per_byte = 8 / bit_depth as usize;
    let row_bytes = (image.width() as usize).div_ceil(per_byte);
    let mut data = vec![0u8; row_bytes * image.height() as usize];
    // The index of the previous pixel's color: runs of one color, the common
    // case in pixel art, need a single lookup.
    let mut last: Option<(Rgba, u8)> = None;
    for (row, pixels) in image.pixels().chunks(image.width() as usize).enumerate() {
        let out = &mut data[row * row_bytes..(row + 1) * row_bytes];
        for (x, p) in pixels.iter().enumerate() {
            let i = match last {
                Some((color, i)) if color == *p => i,
                _ => {
                    let i = index[p];
                    last = Some((*p, i));
                    i
                }
            };
            let shift = 8 - bit_depth as usize * (x % per_byte + 1);
            out[x / per_byte] |= i << shift;
        }
    }
    data
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    const RED: Rgba = Rgba([255, 0, 0, 255]);
    const GREEN: Rgba = Rgba([0, 255, 0, 255]);
    const BLUE: Rgba = Rgba([0, 0, 255, 255]);

    /// An 8x8 red image with `changes` pixels set to the given colors.
    fn frame(changes: &[(u32, u32, Rgba)]) -> Image {
        Image::from_fn(8, 8, |x, y| {
            changes
                .iter()
                .find(|&&(cx, cy, _)| (cx, cy) == (x, y))
                .map_or(RED, |&(_, _, c)| c)
        })
    }

    /// The chunks of a PNG after its signature, as (type, data), checking
    /// every CRC on the way.
    fn chunks(png: &[u8]) -> Vec<(String, Vec<u8>)> {
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        let mut out = Vec::new();
        let mut at = 8;
        while at < png.len() {
            let len = u32::from_be_bytes(png[at..at + 4].try_into().unwrap()) as usize;
            let kind = &png[at + 4..at + 8];
            let data = &png[at + 8..at + 8 + len];
            let crc = u32::from_be_bytes(png[at + 8 + len..at + 12 + len].try_into().unwrap());
            let mut expected = crc32fast::Hasher::new();
            expected.update(kind);
            expected.update(data);
            assert_eq!(
                crc,
                expected.finalize(),
                "CRC of {}",
                String::from_utf8_lossy(kind)
            );
            out.push((String::from_utf8(kind.to_vec()).unwrap(), data.to_vec()));
            at += 12 + len;
        }
        out
    }

    fn be32(data: &[u8]) -> u32 {
        u32::from_be_bytes(data[..4].try_into().unwrap())
    }

    fn three_frames() -> Vec<u8> {
        let images = [
            frame(&[]),
            frame(&[(1, 1, GREEN)]),
            frame(&[(1, 1, GREEN), (6, 5, BLUE)]),
        ];
        let frames: Vec<ApngFrame> = images
            .iter()
            .map(|image| ApngFrame {
                image,
                delay: (1, 10),
            })
            .collect();
        apng(&frames)
    }

    #[test]
    fn chunk_order_and_sequence_numbers() {
        let chunks = chunks(&three_frames());
        let header: Vec<&str> = chunks.iter().take(3).map(|(k, _)| k.as_str()).collect();
        assert_eq!(header, ["IHDR", "acTL", "PLTE"]);
        assert_eq!(be32(&chunks[1].1), 3, "acTL frame count");
        let rest: Vec<String> = chunks[3..]
            .iter()
            .map(|(kind, data)| match kind.as_str() {
                "fcTL" | "fdAT" => format!("{kind} {}", be32(data)),
                _ => kind.clone(),
            })
            .collect();
        assert_eq!(
            rest,
            [
                "fcTL 0", "IDAT", "fcTL 1", "fdAT 2", "fcTL 3", "fdAT 4", "IEND"
            ]
        );
    }

    #[test]
    fn every_frame_is_an_independent_zlib_stream() {
        let chunks = chunks(&three_frames());
        let ihdr = &chunks[0].1;
        let (bit_depth, color_type) = (ihdr[8], ihdr[9]);
        let bits_per_pixel = match color_type {
            3 => usize::from(bit_depth),
            6 => 32,
            other => panic!("unexpected color type {other}"),
        };
        // Each fcTL is followed by its frame's data chunks.
        let mut frames: Vec<((u32, u32), Vec<u8>)> = Vec::new();
        for (kind, data) in &chunks {
            match kind.as_str() {
                "fcTL" => frames.push(((be32(&data[4..]), be32(&data[8..])), Vec::new())),
                "IDAT" => frames.last_mut().unwrap().1.extend_from_slice(data),
                "fdAT" => frames.last_mut().unwrap().1.extend_from_slice(&data[4..]),
                _ => {}
            }
        }
        assert_eq!(frames.len(), 3);
        for (i, ((width, height), zlib)) in frames.iter().enumerate() {
            let mut rows = Vec::new();
            flate2::read::ZlibDecoder::new(zlib.as_slice())
                .read_to_end(&mut rows)
                .unwrap_or_else(|e| panic!("frame {i} does not inflate on its own: {e}"));
            let row_bytes = (*width as usize * bits_per_pixel).div_ceil(8);
            // One filter-type byte per row, then the row.
            assert_eq!(rows.len(), *height as usize * (1 + row_bytes), "frame {i}");
        }
    }

    #[test]
    fn data_is_split_at_the_largest_payload() {
        assert_eq!(MAX_IDAT_DATA, (1 << 31) - 1);
        assert_eq!(MAX_FDAT_DATA, (1 << 31) - 5);
        let data: Vec<u8> = (0..9).collect();
        let lens = |n: usize| -> Vec<usize> {
            split_chunks(&data[..n], 4)
                .iter()
                .map(|c| c.len())
                .collect()
        };
        assert_eq!(lens(3), [3], "below the limit");
        assert_eq!(lens(4), [4], "at the limit");
        assert_eq!(lens(5), [4, 1], "just above the limit");
        assert_eq!(lens(9), [4, 4, 1]);
        let joined: Vec<u8> = split_chunks(&data, 4).concat();
        assert_eq!(joined, data, "nothing lost or reordered");
    }
}
