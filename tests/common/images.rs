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
