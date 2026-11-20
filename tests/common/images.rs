// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 The pixel-perfect Authors

//! Letter-coded images and PNG inspection helpers.
#![allow(dead_code)]

use std::io::Cursor;

use pixel_perfect::model::Rgba;
use pixel_perfect::raster::Image;

/// A distinct opaque color per letter.
pub fn letter(c: char) -> Rgba {
    Rgba([c as u8, 0, 0, 255])
}

/// Builds an image from rows of letters.
pub fn img(rows: &[&str]) -> Image {
    let width = rows[0].len() as u32;
    let pixels = rows.iter().flat_map(|r| r.chars().map(letter)).collect();
    Image::new(width, rows.len() as u32, pixels)
}

/// The rows of a letter-coded image.
pub fn rows(image: &Image) -> Vec<String> {
    image
        .pixels()
        .chunks(image.width() as usize)
        .map(|row| row.iter().map(|p| p.0[0] as char).collect())
        .collect()
}

/// Chunk types of a PNG file, in order.
pub fn chunks(png: &[u8]) -> Vec<String> {
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n", "PNG signature");
    let mut out = Vec::new();
    let mut i = 8;
    while i < png.len() {
        let len = u32::from_be_bytes(png[i..i + 4].try_into().unwrap()) as usize;
        out.push(String::from_utf8(png[i + 4..i + 8].to_vec()).unwrap());
        i += 12 + len;
    }
    out
}

/// The data of the first chunk of type `name`.
pub fn chunk_data<'a>(png: &'a [u8], name: &str) -> Option<&'a [u8]> {
    let mut i = 8;
    while i < png.len() {
        let len = u32::from_be_bytes(png[i..i + 4].try_into().unwrap()) as usize;
        if &png[i + 4..i + 8] == name.as_bytes() {
            return Some(&png[i + 8..i + 8 + len]);
        }
        i += 12 + len;
    }
    None
}

/// Header facts: (width, height, bit depth, color type).
pub fn header(png: &[u8]) -> (u32, u32, u8, u8) {
    let ihdr = chunk_data(png, "IHDR").unwrap();
    (
        u32::from_be_bytes(ihdr[0..4].try_into().unwrap()),
        u32::from_be_bytes(ihdr[4..8].try_into().unwrap()),
        ihdr[8],
        ihdr[9],
    )
}

/// One decoded APNG frame.
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedFrame {
    pub pixels: Vec<Rgba>,
    pub delay: (u16, u16),
    /// (x offset, y offset, width, height, dispose op, blend op)
    pub control: (u32, u32, u32, u32, u8, u8),
}

/// Decodes every frame of an APNG to RGBA8, with `num_plays`.
pub fn decode_apng(data: &[u8]) -> (Vec<DecodedFrame>, u32) {
    let mut decoder = png::Decoder::new(Cursor::new(data));
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().unwrap();
    let actl = reader.info().animation_control.expect("acTL");
    let mut frames = Vec::new();
    for _ in 0..actl.num_frames {
        let mut buf = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut buf).unwrap();
        let fc = *reader.info().frame_control().expect("fcTL");
        let pixels = match info.color_type {
            png::ColorType::Rgba => buf[..info.buffer_size()]
                .chunks(4)
                .map(|c| Rgba([c[0], c[1], c[2], c[3]]))
                .collect(),
            png::ColorType::Rgb => buf[..info.buffer_size()]
                .chunks(3)
                .map(|c| Rgba([c[0], c[1], c[2], 255]))
                .collect(),
            other => panic!("unexpected decoded color type {other:?}"),
        };
        frames.push(DecodedFrame {
            pixels,
            delay: (fc.delay_num, fc.delay_den),
            control: (
                fc.x_offset,
                fc.y_offset,
                fc.width,
                fc.height,
                fc.dispose_op as u8,
                fc.blend_op as u8,
            ),
        });
    }
    (frames, actl.num_plays)
}

/// Decodes a PNG to RGBA8 pixels with an independent decoder path.
pub fn decode(png: &[u8]) -> (u32, u32, Vec<Rgba>) {
    let mut decoder = png::Decoder::new(Cursor::new(png));
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).unwrap();
    let pixels = match info.color_type {
        png::ColorType::Rgba => buf[..info.buffer_size()]
            .chunks(4)
            .map(|c| Rgba([c[0], c[1], c[2], c[3]]))
            .collect(),
        png::ColorType::Rgb => buf[..info.buffer_size()]
            .chunks(3)
            .map(|c| Rgba([c[0], c[1], c[2], 255]))
            .collect(),
        other => panic!("unexpected decoded color type {other:?}"),
    };
    (info.width, info.height, pixels)
}

/// One chunk of a WebP RIFF file: its FourCC and payload (without padding).
#[derive(Debug, Clone, PartialEq)]
pub struct WebpChunk {
    pub fourcc: String,
    pub data: Vec<u8>,
}

/// Lists the chunks of a WebP file in order, checking the RIFF header, the
/// RIFF size and the even padding of every chunk.
pub fn webp_chunks(data: &[u8]) -> Vec<WebpChunk> {
    assert_eq!(&data[0..4], b"RIFF", "RIFF signature");
    let riff_size = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
    assert_eq!(riff_size, data.len() - 8, "RIFF size");
    assert_eq!(&data[8..12], b"WEBP", "WEBP form type");
    riff_chunks(&data[12..])
}

/// Parses a sequence of RIFF chunks that fills `data` exactly.
fn riff_chunks(data: &[u8]) -> Vec<WebpChunk> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < data.len() {
        let fourcc = String::from_utf8(data[i..i + 4].to_vec()).unwrap();
        let len = u32::from_le_bytes(data[i + 4..i + 8].try_into().unwrap()) as usize;
        out.push(WebpChunk {
            fourcc,
            data: data[i + 8..i + 8 + len].to_vec(),
        });
        if len % 2 == 1 {
            assert_eq!(data[i + 8 + len], 0, "padding byte is zero");
        }
        i += 8 + len + len % 2;
    }
    assert_eq!(i, data.len(), "chunks fill the file exactly");
    out
}

/// Reads a little-endian 24-bit field.
pub fn u24(bytes: &[u8]) -> u32 {
    u32::from(bytes[0]) | u32::from(bytes[1]) << 8 | u32::from(bytes[2]) << 16
}

/// The fields of an `ANMF` chunk.
#[derive(Debug, Clone, PartialEq)]
pub struct Anmf {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub duration_ms: u32,
    /// Bit 1: do not blend; bit 0: dispose to background.
    pub flags: u8,
    /// The chunks nested in the frame.
    pub chunks: Vec<WebpChunk>,
}

pub fn anmf(chunk: &WebpChunk) -> Anmf {
    assert_eq!(chunk.fourcc, "ANMF");
    let d = &chunk.data;
    Anmf {
        x: u24(&d[0..3]) * 2,
        y: u24(&d[3..6]) * 2,
        width: u24(&d[6..9]) + 1,
        height: u24(&d[9..12]) + 1,
        duration_ms: u24(&d[12..15]),
        flags: d[15],
        chunks: riff_chunks(&d[16..]),
    }
}

/// Decodes a still WebP to RGBA8 pixels.
pub fn decode_webp(data: &[u8]) -> (u32, u32, Vec<Rgba>) {
    let mut decoder = image_webp::WebPDecoder::new(Cursor::new(data)).unwrap();
    assert!(!decoder.is_animated(), "still image");
    let (w, h) = decoder.dimensions();
    let mut buf = vec![0; decoder.output_buffer_size().unwrap()];
    decoder.read_image(&mut buf).unwrap();
    (w, h, rgba_pixels(&buf, decoder.has_alpha()))
}

/// Decodes every frame of an animated WebP to RGBA8, with its duration in ms.
pub fn decode_webp_frames(data: &[u8]) -> Vec<(Vec<Rgba>, u32)> {
    let mut decoder = image_webp::WebPDecoder::new(Cursor::new(data)).unwrap();
    assert!(decoder.is_animated(), "animated image");
    let alpha = decoder.has_alpha();
    let mut out = Vec::new();
    for _ in 0..decoder.num_frames() {
        let mut buf = vec![0; decoder.output_buffer_size().unwrap()];
        let duration = decoder.read_frame(&mut buf).unwrap();
        out.push((rgba_pixels(&buf, alpha), duration));
    }
    out
}

fn rgba_pixels(buf: &[u8], alpha: bool) -> Vec<Rgba> {
    if alpha {
        buf.chunks(4)
            .map(|c| Rgba([c[0], c[1], c[2], c[3]]))
            .collect()
    } else {
        buf.chunks(3)
            .map(|c| Rgba([c[0], c[1], c[2], 255]))
            .collect()
    }
}
