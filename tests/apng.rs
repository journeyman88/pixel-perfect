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

// Changed-area and masked frames

/// The `fcTL` fields of every decoded frame.
fn controls(data: &[u8]) -> Vec<(u32, u32, u32, u32, u8, u8)> {
    decode_apng(data).0.iter().map(|f| f.control).collect()
}

/// Decoded canvases must equal the encoded frames.
fn assert_round_trip(images: &[Image], data: &[u8]) {
    let (decoded, _) = decode_apng(data);
    assert_eq!(decoded.len(), images.len());
    for (i, (frame, image)) in decoded.iter().zip(images).enumerate() {
        assert_eq!(frame.pixels, image.pixels(), "frame {i}");
    }
}

/// A `w`x`h` canvas of `background` with an 8x8 square of `square` at `at`.
fn square(w: u32, h: u32, background: Rgba, square: Rgba, at: (u32, u32)) -> Image {
    Image::from_fn(w, h, |x, y| {
        let inside = (at.0..at.0 + 8).contains(&x) && (at.1..at.1 + 8).contains(&y);
        if inside { square } else { background }
    })
}

/// A canvas with more than 256 colors, so it is encoded as RGBA.
fn gradient(w: u32, h: u32) -> Image {
    Image::from_fn(w, h, |x, y| {
        Rgba([x as u8 * 4, y as u8 * 4, (x + y) as u8, 255])
    })
}

const SOURCE: u8 = 0;
const OVER: u8 = 1;

#[test]
fn small_change_writes_only_the_changed_area() {
    let frames = [square(64, 32, G, R, (10, 4)), square(64, 32, G, R, (11, 4))];
    let data = apng(&frames, &[(1, 10), (1, 10)]);
    let c = controls(&data);
    assert_eq!(c[0], (0, 0, 64, 32, 0, SOURCE), "first frame: full canvas");
    assert_eq!(
        c[1],
        (10, 4, 9, 8, 0, SOURCE),
        "the sprite's old and new columns"
    );
    assert_round_trip(&frames, &data);
}

#[test]
fn transparent_over_opaque_is_a_changed_area_frame() {
    let frames = [image(&[R, R, R]), image(&[R, TRANSPARENT, R])];
    let data = apng(&frames, &[(1, 10), (1, 10)]);
    assert_eq!(controls(&data)[1], (1, 0, 1, 1, 0, SOURCE));
    assert_eq!(decode_apng(&data).0[1].pixels[1], TRANSPARENT);
}

#[test]
fn identical_frame_is_written_as_1x1() {
    let frames = [image(&[R, G]), image(&[R, G])];
    let data = apng(&frames, &[(65535, 1), (4465, 1)]);
    assert_eq!(controls(&data)[1], (0, 0, 1, 1, 0, SOURCE));
    assert_round_trip(&frames, &data);
}

#[test]
fn scattered_opaque_changes_are_masked_in_rgba() {
    let a = gradient(64, 64);
    let mut pixels = a.pixels().to_vec();
    pixels[0] = Rgba([1, 2, 3, 255]);
    pixels[64 * 64 - 1] = Rgba([3, 2, 1, 255]);
    let b = Image::new(64, 64, pixels);
    let frames = [a, b];
    let data = apng(&frames, &[(1, 10), (1, 10)]);
    assert_eq!(controls(&data)[1], (0, 0, 64, 64, 0, OVER));
    assert_round_trip(&frames, &data);
}

#[test]
fn masked_needs_transparent_in_an_indexed_palette() {
    // Only red and green: the masked frame would need (0, 0, 0, 0).
    let corners = |c: Rgba| {
        Image::from_fn(64, 64, |x, y| {
            if (x, y) == (0, 0) || (x, y) == (63, 63) {
                c
            } else {
                R
            }
        })
    };
    let frames = [corners(R), corners(G)];
    let data = apng(&frames, &[(1, 10), (1, 10)]);
    assert_eq!(controls(&data)[1], (0, 0, 64, 64, 0, SOURCE));
    assert_round_trip(&frames, &data);

    // With a transparent pixel somewhere, the palette has it and masking is
    // allowed.
    let with_hole = |c: Rgba| {
        Image::from_fn(64, 64, |x, y| match (x, y) {
            (0, 0) | (63, 63) => c,
            (30, 30) => TRANSPARENT,
            _ => R,
        })
    };
    let frames = [with_hole(R), with_hole(G)];
    let data = apng(&frames, &[(1, 10), (1, 10)]);
    assert_eq!(controls(&data)[1], (0, 0, 64, 64, 0, OVER));
    assert_round_trip(&frames, &data);
}

#[test]
fn translucent_change_is_never_masked() {
    let a = gradient(64, 64);
    let mut pixels = a.pixels().to_vec();
    pixels[0] = Rgba([1, 2, 3, 128]);
    pixels[64 * 64 - 1] = Rgba([3, 2, 1, 255]);
    let frames = [a, Image::new(64, 64, pixels)];
    let data = apng(&frames, &[(1, 10), (1, 10)]);
    assert_eq!(controls(&data)[1].5, SOURCE);
    assert_round_trip(&frames, &data);
}

#[test]
fn fixture_animations_round_trip() {
    use pixel_perfect::animate::{self, Animation};
    for fixture in ["valid/animation.json", "valid/full.json"] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(fixture);
        let ws = pixel_perfect::load_file(&path).unwrap();
        for (collection, ids) in [
            (
                "flipbooks",
                ws.model
                    .flipbooks
                    .iter()
                    .map(|f| f.id.clone())
                    .collect::<Vec<_>>(),
            ),
            (
                "panels",
                ws.model.panels.iter().map(|p| p.id.clone()).collect(),
            ),
            (
                "timelines",
                ws.model.timelines.iter().map(|t| t.id.clone()).collect(),
            ),
            (
                "scenes",
                ws.model.scenes.iter().map(|s| s.id.clone()).collect(),
            ),
        ] {
            for id in ids {
                if !animate::is_animated(&ws, collection, &id) {
                    continue;
                }
                let animation = Animation::new(&ws, collection, &id).unwrap();
                let frames = animation.frames();
                let images: Vec<Image> = frames.iter().map(|(i, _)| i.clone()).collect();
                let delays: Vec<(u16, u16)> = frames
                    .iter()
                    .map(|(_, t)| {
                        let (n, d) = pixel_perfect::time::reduced(*t, animation.clock);
                        (n as u16, d as u16)
                    })
                    .collect();
                assert_round_trip(&images, &apng(&images, &delays));
            }
        }
    }
}

#[test]
fn indexed_2_and_4_bit_frames_start_at_the_left_edge() {
    // 3 colors: indexed, 2 bits per pixel. FFmpeg 8.1 misdecodes sub-frames
    // of 2- and 4-bit images whose x offset is not 0.
    let frames = [square(64, 32, G, R, (10, 4)), square(64, 32, G, B, (10, 4))];
    let data = apng(&frames, &[(1, 10), (1, 10)]);
    assert_eq!(header(&data).2, 2, "bit depth");
    assert_eq!(controls(&data)[1], (0, 4, 18, 8, 0, SOURCE));
    assert_round_trip(&frames, &data);
    // 2 colors: 1 bit per pixel, which FFmpeg decodes correctly at any offset.
    let frames = [square(64, 32, G, G, (10, 4)), square(64, 32, G, R, (10, 4))];
    let data = apng(&frames, &[(1, 10), (1, 10)]);
    assert_eq!(header(&data).2, 1, "bit depth");
    assert_eq!(controls(&data)[1], (10, 4, 8, 8, 0, SOURCE));
}
