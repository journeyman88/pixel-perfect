// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 The pixel-perfect Authors

//! PNG decoding into canonical 8-bit RGBA, and `--scale` downscaling.

use std::io::Cursor;

use png::{BitDepth, ColorType, Transformations};

use crate::model::Rgba;
use crate::raster::{Image, canonical};

const SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

/// Decodes a PNG into 8-bit RGBA without loss.
///
/// Palette, low bit depth and `tRNS` are expanded. 16-bit images are accepted
/// only when every sample is `v * 257`. Color management chunks are ignored.
/// Every pixel with alpha 0 becomes transparent. `max_bytes` bounds what the
/// decoder may allocate.
pub fn decode(bytes: &[u8], max_bytes: usize) -> Result<Image, String> {
    if !bytes.starts_with(SIGNATURE) {
        return Err("not a PNG file".into());
    }
    let mut decoder =
        png::Decoder::new_with_limits(Cursor::new(bytes), png::Limits { bytes: max_bytes });
    decoder.set_transformations(Transformations::EXPAND);
    let mut reader = decoder
        .read_info()
        .map_err(|e| format!("cannot decode PNG: {e}"))?;
    if reader.info().animation_control.is_some() {
        return Err(
            "animated PNG (acTL chunk) is not supported: import its frames as separate PNGs".into(),
        );
    }
    let size = reader
        .output_buffer_size()
        .ok_or("image is too large to decode")?;
    let mut buf = vec![0; size];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| format!("cannot decode PNG: {e}"))?;
    let channels = match info.color_type {
        ColorType::Grayscale => 1,
        ColorType::GrayscaleAlpha => 2,
        ColorType::Rgb => 3,
        ColorType::Rgba => 4,
        ColorType::Indexed => unreachable!("EXPAND turns indexed images into RGB(A)"),
    };
    let wide = info.bit_depth == BitDepth::Sixteen;
    let sample_size = if wide { 2 } else { 1 };
    let (width, height) = (info.width, info.height);
    let mut pixels = Vec::with_capacity(width as usize * height as usize);
    for y in 0..height {
        let row = &buf[y as usize * info.line_size..][..info.line_size];
        for x in 0..width {
            let mut s = [0u8; 4];
            for (c, sample) in s.iter_mut().take(channels).enumerate() {
                let at = (x as usize * channels + c) * sample_size;
                *sample = if wide {
                    let v = u16::from_be_bytes([row[at], row[at + 1]]);
                    if v % 257 != 0 {
                        return Err(format!(
                            "pixel ({x}, {y}) has the 16-bit sample {v}, which has no exact \
                             8-bit equivalent"
                        ));
                    }
                    (v / 257) as u8
                } else {
                    row[at]
                };
            }
            pixels.push(canonical(Rgba(match channels {
                1 => [s[0], s[0], s[0], 255],
                2 => [s[0], s[0], s[0], s[1]],
                3 => [s[0], s[1], s[2], 255],
                _ => s,
            })));
        }
    }
    Ok(Image::new(width, height, pixels))
}

/// Treats `image` as an `n`x enlargement and returns the original: every
/// aligned `n`x`n` block must be uniform and becomes one pixel.
pub fn downscale(image: &Image, n: u32) -> Result<Image, String> {
    if n == 1 {
        return Ok(image.clone());
    }
    for (name, size) in [("width", image.width()), ("height", image.height())] {
        if size % n != 0 {
            return Err(format!(
                "{name} {size} is not a multiple of --scale {n} (image is {}x{})",
                image.width(),
                image.height()
            ));
        }
    }
    let at = |x: u32, y: u32| image.pixels()[y as usize * image.width() as usize + x as usize];
    let (w, h) = (image.width() / n, image.height() / n);
    for by in 0..h {
        for bx in 0..w {
            let (x0, y0) = (bx * n, by * n);
            let first = at(x0, y0);
            for y in y0..y0 + n {
                for x in x0..x0 + n {
                    if at(x, y) != first {
                        return Err(format!(
                            "the {n}x{n} block at ({x0}, {y0}) is not a single color \
                             (pixel ({x}, {y}) differs)"
                        ));
                    }
                }
            }
        }
    }
    Ok(Image::from_fn(w, h, |x, y| at(x * n, y * n)))
}
