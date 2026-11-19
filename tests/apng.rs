// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 The pixel-perfect Authors

//! APNG encoding: scenarios of the apng-export spec that concern the encoder.

#[path = "common/images.rs"]
mod images;

use images::{chunk_data, chunks, decode, decode_apng, header};
use pixel_perfect::encode::{self, ApngFrame, Encoding};
use pixel_perfect::model::Rgba;
use pixel_perfect::raster::{Image, TRANSPARENT};

const R: Rgba = Rgba([255, 0, 0, 255]);
const G: Rgba = Rgba([0, 255, 0, 255]);
const B: Rgba = Rgba([0, 0, 255, 255]);

fn image(pixels: &[Rgba]) -> Image {
    Image::new(pixels.len() as u32, 1, pixels.to_vec())
}

fn apng(images: &[Image], delays: &[(u16, u16)]) -> Vec<u8> {
    let frames: Vec<ApngFrame> = images
        .iter()
        .zip(delays)
        .map(|(image, &delay)| ApngFrame { image, delay })
        .collect();
    encode::apng(&frames)
}

// APNG structure

#[test]
fn chunk_layout_and_loop() {
    let frames = [image(&[R, G]), image(&[G, B]), image(&[B, R])];
    let data = apng(&frames, &[(1, 10), (1, 10), (1, 5)]);
    assert_eq!(
        chunks(&data),
        [
            "IHDR", "acTL", "PLTE", "fcTL", "IDAT", "fcTL", "fdAT", "fcTL", "fdAT", "IEND"
        ]
    );
    let actl = chunk_data(&data, "acTL").unwrap();
    assert_eq!(
        u32::from_be_bytes(actl[0..4].try_into().unwrap()),
        3,
        "num_frames"
    );
    assert_eq!(
        u32::from_be_bytes(actl[4..8].try_into().unwrap()),
        0,
        "num_plays"
    );
    let (decoded, plays) = decode_apng(&data);
    assert_eq!(plays, 0);
    for frame in &decoded {
        assert_eq!(
            frame.control,
            (0, 0, 2, 1, 0, 0),
            "full canvas, NONE, SOURCE"
        );
    }
}

#[test]
fn viewer_without_apng_support() {
    let frames = [image(&[R, G]), image(&[G, B])];
    let data = apng(&frames, &[(1, 2), (1, 2)]);
    // The plain PNG decoding path reads only the default image.
    assert_eq!(decode(&data).2, frames[0].pixels());
}

#[test]
fn transparent_pixel_over_an_opaque_one() {
    let frames = [image(&[R, G]), image(&[TRANSPARENT, G])];
    let (decoded, _) = decode_apng(&apng(&frames, &[(1, 2), (1, 2)]));
    assert_eq!(decoded[1].pixels[0], TRANSPARENT);
}

// APNG encoding chosen by color count

#[test]
fn shared_palette() {
    let frames = [image(&[R, G]), image(&[G, B])];
    assert_eq!(
        encode::choose_all(&frames),
        Encoding::Indexed {
            palette: vec![R, G, B],
            transparent: 0,
            bit_depth: 2
        }
    );
    let data = apng(&frames, &[(1, 2), (1, 2)]);
    assert_eq!(header(&data), (2, 1, 2, 3));
    assert_eq!(
        chunk_data(&data, "PLTE").unwrap(),
        [255, 0, 0, 0, 255, 0, 0, 0, 255]
    );
    assert!(chunk_data(&data, "tRNS").is_none());
}

#[test]
fn translucent_colors_first_across_frames() {
    let half = Rgba([9, 9, 9, 128]);
    let frames = [image(&[R, G]), image(&[half, TRANSPARENT])];
    let Encoding::Indexed {
        palette,
        transparent,
        ..
    } = encode::choose_all(&frames)
    else {
        panic!("indexed")
    };
    assert_eq!(palette, vec![half, TRANSPARENT, R, G]);
    assert_eq!(transparent, 2);
}

#[test]
fn colors_spread_over_frames() {
    let frames: Vec<Image> = (0..3u8)
        .map(|f| {
            let pixels: Vec<Rgba> = (0..100u8).map(|i| Rgba([f, i, 7, 255])).collect();
            image(&pixels)
        })
        .collect();
    assert_eq!(encode::choose_all(&frames), Encoding::Rgba);
    let data = apng(&frames, &[(1, 3); 3]);
    let (_, _, depth, color_type) = header(&data);
    assert_eq!((depth, color_type), (8, 6));
}

// Lossless and deterministic APNG

#[test]
fn round_trip() {
    let half = Rgba([30, 60, 90, 77]);
    let frames = [
        image(&[R, half, TRANSPARENT]),
        image(&[B, B, G]),
        image(&[half, R, R]),
    ];
    let delays = [(1, 4), (65535, 3), (7, 1000)];
    let (decoded, _) = decode_apng(&apng(&frames, &delays));
    assert_eq!(decoded.len(), 3);
    for ((frame, image), delay) in decoded.iter().zip(&frames).zip(delays) {
        assert_eq!(frame.pixels, image.pixels());
        assert_eq!(frame.delay, delay);
    }
}

#[test]
fn round_trip_rgba() {
    let frames: Vec<Image> = (0..2u8)
        .map(|f| image(&(0..200u8).map(|i| Rgba([f, i, 1, 255])).collect::<Vec<_>>()))
        .collect();
    let (decoded, _) = decode_apng(&apng(&frames, &[(1, 2), (1, 2)]));
    assert_eq!(decoded[0].pixels, frames[0].pixels());
    assert_eq!(decoded[1].pixels, frames[1].pixels());
}

#[test]
fn repeated_encoding_is_byte_identical() {
    let frames = [image(&[R, G]), image(&[G, B])];
    assert_eq!(
        apng(&frames, &[(1, 2), (1, 2)]),
        apng(&frames, &[(1, 2), (1, 2)])
    );
}

#[test]
fn single_frame() {
    let data = apng(&[image(&[R])], &[(3, 1)]);
    assert_eq!(
        chunks(&data),
        ["IHDR", "acTL", "PLTE", "fcTL", "IDAT", "IEND"]
    );
    let (decoded, _) = decode_apng(&data);
    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded[0].delay, (3, 1));
}
