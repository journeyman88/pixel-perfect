// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 The pixel-perfect Authors

//! Deterministic lossless WebP encoding, still and animated, in pure Rust.
//!
//! `image-webp` encodes each image to a `VP8L` bitstream; the RIFF container
//! around it (simple for stills, `VP8X` + `ANIM` + `ANMF` for animations) is
//! written here, so the file layout does not depend on the crate.

use image_webp::{ColorType, EncoderParams, WebPEncoder};
use rayon::prelude::*;

use crate::delta::{self, Rect};
use crate::raster::Image;

/// Largest width or height a `VP8L` bitstream can store (14 bits).
pub const MAX_SIDE: u32 = 16384;

/// Longest duration of one `ANMF` frame, in milliseconds (24 bits).
pub const MAX_DURATION_MS: u128 = 0xff_ffff;

/// `VP8X` flag: the file contains an animation.
const ANIMATION_FLAG: u8 = 1 << 1;
/// `VP8X` flag: some pixel has alpha below 255.
const ALPHA_FLAG: u8 = 1 << 4;
/// `ANMF` flag: do not blend with the previous canvas (disposal bit 0 = none).
const NO_BLEND_FLAG: u8 = 1 << 1;

/// One animation frame: the image and how long it is shown, in milliseconds.
/// The duration may exceed [`MAX_DURATION_MS`]; [`animated`] splits it.
#[derive(Debug, Clone, Copy)]
pub struct WebpFrame<'a> {
    pub image: &'a Image,
    pub duration_ms: u128,
}

/// Why an animation could not be encoded.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum WebpError {
    #[error("the WebP file would exceed 4 GiB, the largest size its RIFF container can hold")]
    TooLarge,
}

/// True if some pixel of `image` has alpha below 255.
fn has_alpha(image: &Image) -> bool {
    image.pixels().iter().any(|p| p.0[3] < 255)
}

/// The `VP8L` bitstream of `image`, with fixed parameters. With `alpha` it is
/// encoded as RGBA8 and its header says alpha is used; without, as RGB8 (every
/// pixel must be opaque). Decoders such as libwebp reject a `VP8L` frame that
/// uses alpha when the `VP8X` alpha flag is off, so the two must agree.
///
/// Panics if a side is 0 or over [`MAX_SIDE`]; the export plan rejects those.
fn vp8l(image: &Image, alpha: bool) -> Vec<u8> {
    let (data, color): (Vec<u8>, ColorType) = if alpha {
        let data = image.pixels().iter().flat_map(|p| p.0).collect();
        (data, ColorType::Rgba8)
    } else {
        debug_assert!(!has_alpha(image));
        let data = image
            .pixels()
            .iter()
            .flat_map(|p| [p.0[0], p.0[1], p.0[2]])
            .collect();
        (data, ColorType::Rgb8)
    };
    // Set every parameter explicitly, so a new default in `image-webp` shows up
    // as a code change, not as a silent change of the output.
    let mut params = EncoderParams::default();
    params.use_predictor_transform = true;
    let mut file = Vec::new();
    let mut encoder = WebPEncoder::new(&mut file);
    encoder.set_params(params);
    encoder
        .encode(&data, image.width(), image.height(), color)
        .expect("the export plan checked the WebP size limit");
    // Without metadata, `image-webp` writes the simple format:
    // RIFF <size> WEBP VP8L <size> <bitstream> [padding].
    assert_eq!(&file[0..4], b"RIFF");
    assert_eq!(&file[8..16], b"WEBPVP8L");
    let len = u32::from_le_bytes(file[16..20].try_into().unwrap()) as usize;
    assert_eq!(20 + len + len % 2, file.len(), "a single VP8L chunk");
    file[20..20 + len].to_vec()
}

/// Appends a RIFF chunk: FourCC, little-endian size, payload, even padding.
fn chunk(out: &mut Vec<u8>, fourcc: &[u8; 4], payload: &[u8]) {
    out.extend_from_slice(fourcc);
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(payload);
    if payload.len() % 2 == 1 {
        out.push(0);
    }
}

/// Size of a chunk with this payload length, header and padding included.
fn chunk_len(payload: usize) -> u64 {
    8 + payload as u64 + payload as u64 % 2
}

/// A little-endian 24-bit field. Panics if `value` does not fit.
fn u24(value: u32) -> [u8; 3] {
    assert!(value <= 0xff_ffff, "{value} does not fit 24 bits");
    let [a, b, c, _] = value.to_le_bytes();
    [a, b, c]
}

/// Wraps chunks into a RIFF WEBP file.
fn riff(body: Vec<u8>) -> Result<Vec<u8>, WebpError> {
    let size = u32::try_from(body.len() + 4).map_err(|_| WebpError::TooLarge)?;
    let mut out = Vec::with_capacity(body.len() + 12);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&size.to_le_bytes());
    out.extend_from_slice(b"WEBP");
    out.extend(body);
    Ok(out)
}

/// Encodes a still image as a simple lossless WebP: `RIFF`, `WEBP`, one `VP8L`
/// chunk, using alpha only if some pixel has alpha below 255. Same image, same
/// bytes.
pub fn still(image: &Image) -> Vec<u8> {
    let mut body = Vec::new();
    chunk(&mut body, b"VP8L", &vp8l(image, has_alpha(image)));
    riff(body).expect("one image of at most 16384x16384 fits the RIFF size")
}

/// The durations of the `ANMF` frames written for one frame of `duration_ms`:
/// parts of [`MAX_DURATION_MS`], then the remainder if any.
pub fn split_duration(duration_ms: u128) -> impl Iterator<Item = u32> {
    let full = duration_ms / MAX_DURATION_MS;
    let rest = duration_ms % MAX_DURATION_MS;
    let full = std::iter::repeat_n(MAX_DURATION_MS as u32, full as usize);
    full.chain((rest != 0).then_some(rest as u32))
}

/// Encodes an animation as an animated lossless WebP: `VP8X` (animation flag,
/// alpha flag if some pixel of some frame has alpha below 255; every `VP8L`
/// then uses alpha, otherwise none does), `ANIM` (transparent
/// background, infinite loop), then one `ANMF` per frame that does not blend
/// and does not dispose: the first covers the canvas, each later one only the
/// area that changed (extended to even offsets; 1x1 at (0, 0) when nothing
/// changed), so the canvas always equals the current frame.
/// Frames longer than [`MAX_DURATION_MS`] are written as several identical
/// frames. Returns the bytes and the number of `ANMF` chunks.
///
/// Panics if `frames` is empty, the images differ in size or a duration is 0.
pub fn animated(frames: &[WebpFrame]) -> Result<(Vec<u8>, usize), WebpError> {
    let first = frames
        .first()
        .expect("an animation has at least one frame")
        .image;
    let (width, height) = (first.width(), first.height());
    let alpha = frames.par_iter().any(|f| has_alpha(f.image));

    let mut body = Vec::new();
    let mut vp8x = vec![ANIMATION_FLAG | if alpha { ALPHA_FLAG } else { 0 }, 0, 0, 0];
    vp8x.extend(u24(width - 1));
    vp8x.extend(u24(height - 1));
    chunk(&mut body, b"VP8X", &vp8x);
    // Background color (B, G, R, A) and loop count 0 (forever).
    chunk(&mut body, b"ANIM", &[0, 0, 0, 0, 0, 0]);

    for frame in frames {
        assert_eq!(
            (frame.image.width(), frame.image.height()),
            (width, height),
            "every frame has the canvas size"
        );
        assert!(
            frame.duration_ms > 0,
            "zero-length frames are dropped earlier"
        );
    }
    // The first part of each frame shows the new frame: its changed area,
    // encoded as VP8L. It depends only on the frame and the previous one, so
    // every frame is encoded in parallel; the container is then assembled in
    // order, so the file is the same as when encoding one frame at a time.
    // The bitstreams are compressed, so keeping them all costs little memory.
    let firsts: Vec<(Rect, Vec<u8>)> = (0..frames.len())
        .into_par_iter()
        .map(|i| {
            let image = frames[i].image;
            let rect = match i {
                0 => Rect::full(image),
                _ => delta::align_even(
                    delta::changed_rect(frames[i - 1].image, image).unwrap_or(Rect::UNCHANGED),
                ),
            };
            (rect, vp8l(&delta::crop(image, rect), alpha))
        })
        .collect();

    let mut count = 0;
    for (frame, (first_rect, first)) in frames.iter().zip(firsts) {
        // The parts of a split duration after the first change nothing.
        let mut repeat: Option<Vec<u8>> = None;
        for (part, duration) in split_duration(frame.duration_ms).enumerate() {
            let (rect, bitstream) = if part == 0 {
                (first_rect, &first)
            } else {
                let unchanged = repeat
                    .get_or_insert_with(|| vp8l(&delta::crop(frame.image, Rect::UNCHANGED), alpha));
                (Rect::UNCHANGED, &*unchanged)
            };
            let frame_len = chunk_len(16 + chunk_len(bitstream.len()) as usize);
            if body.len() as u64 + frame_len + 4 > u64::from(u32::MAX) {
                return Err(WebpError::TooLarge);
            }
            let mut payload = Vec::with_capacity(16 + bitstream.len() + 9);
            payload.extend(u24(rect.x / 2));
            payload.extend(u24(rect.y / 2));
            payload.extend(u24(rect.width - 1));
            payload.extend(u24(rect.height - 1));
            payload.extend(u24(duration));
            payload.push(NO_BLEND_FLAG);
            chunk(&mut payload, b"VP8L", bitstream);
            chunk(&mut body, b"ANMF", &payload);
            count += 1;
        }
    }
    Ok((riff(body)?, count))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_durations_split_into_full_parts_and_a_rest() {
        // 20000 s.
        let parts: Vec<u32> = split_duration(20_000_000).collect();
        assert_eq!(parts, [16_777_215, 3_222_785]);
        let parts: Vec<u32> = split_duration(2 * MAX_DURATION_MS).collect();
        assert_eq!(parts, [16_777_215, 16_777_215]);
        let parts: Vec<u32> = split_duration(40).collect();
        assert_eq!(parts, [40]);
    }

    #[test]
    fn odd_chunks_are_padded() {
        let mut out = Vec::new();
        chunk(&mut out, b"TEST", &[1, 2, 3]);
        assert_eq!(out, b"TEST\x03\x00\x00\x00\x01\x02\x03\x00");
        assert_eq!(chunk_len(3), 12);
    }
}
