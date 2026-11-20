// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 The pixel-perfect Authors

//! WebP encoding: container layout, round trips and durations.

#[path = "common/images.rs"]
mod images;

use images::{anmf, decode_webp, decode_webp_frames, img, webp_chunks};
use pixel_perfect::model::Rgba;
use pixel_perfect::raster::Image;
use pixel_perfect::webp::{self, WebpFrame};

/// A RIFF chunk with its padding byte.
fn raw_chunk(fourcc: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut out = fourcc.to_vec();
    out.extend((payload.len() as u32).to_le_bytes());
    out.extend(payload);
    if payload.len() % 2 == 1 {
        out.push(0);
    }
    out
}

fn raw_file(chunks: &[Vec<u8>]) -> Vec<u8> {
    let body: Vec<u8> = chunks.concat();
    let mut out = b"RIFF".to_vec();
    out.extend(((body.len() + 4) as u32).to_le_bytes());
    out.extend(b"WEBP");
    out.extend(body);
    out
}

#[test]
fn chunk_lister_reads_a_hand_built_file() {
    let mut frame = vec![
        1, 0, 0, // x / 2
        2, 0, 0, // y / 2
        7, 0, 0, // width - 1
        3, 0, 0, // height - 1
        0xe8, 0x03, 0,    // 1000 ms
        0b10, // do not blend, no dispose
    ];
    frame.extend(raw_chunk(b"VP8L", &[0xaa, 0xbb, 0xcc]));
    let file = raw_file(&[
        raw_chunk(b"VP8X", &[0; 10]),
        raw_chunk(b"ANIM", &[0; 6]),
        raw_chunk(b"ANMF", &frame),
    ]);

    let chunks = webp_chunks(&file);
    let names: Vec<&str> = chunks.iter().map(|c| c.fourcc.as_str()).collect();
    assert_eq!(names, ["VP8X", "ANIM", "ANMF"]);
    let f = anmf(&chunks[2]);
    assert_eq!((f.x, f.y, f.width, f.height), (2, 4, 8, 4));
    assert_eq!((f.duration_ms, f.flags), (1000, 0b10));
    assert_eq!(f.chunks.len(), 1);
    assert_eq!(f.chunks[0].fourcc, "VP8L");
    assert_eq!(
        f.chunks[0].data,
        [0xaa, 0xbb, 0xcc],
        "odd payload, padding stripped"
    );
}

fn names(file: &[u8]) -> Vec<String> {
    webp_chunks(file).into_iter().map(|c| c.fourcc).collect()
}

const CLEAR: Rgba = Rgba([0, 0, 0, 0]);
const HALF: Rgba = Rgba([10, 200, 30, 128]);
const OPAQUE: Rgba = Rgba([250, 20, 40, 255]);

// Still images

#[test]
fn still_round_trip_with_transparency() {
    let image = Image::new(3, 2, vec![CLEAR, HALF, OPAQUE, OPAQUE, CLEAR, HALF]);
    let file = webp::still(&image);
    assert_eq!(names(&file), ["VP8L"]);
    let (w, h, pixels) = decode_webp(&file);
    assert_eq!((w, h), (3, 2));
    assert_eq!(pixels, image.pixels());
}

#[test]
fn still_round_trip_opaque() {
    let image = img(&["abc", "cba", "aaa"]);
    let (_, _, pixels) = decode_webp(&webp::still(&image));
    assert_eq!(pixels, image.pixels());
}

// Animations

fn frame(image: &Image, duration_ms: u128) -> WebpFrame<'_> {
    WebpFrame { image, duration_ms }
}

#[test]
fn animated_chunk_layout() {
    let images = [img(&["ab"]), img(&["bc"]), img(&["cd"]), img(&["de"])];
    let frames: Vec<WebpFrame> = images.iter().map(|i| frame(i, 100)).collect();
    let (file, count) = webp::animated(&frames).unwrap();
    assert_eq!(count, 4);
    let chunks = webp_chunks(&file);
    assert_eq!(
        names(&file),
        ["VP8X", "ANIM", "ANMF", "ANMF", "ANMF", "ANMF"]
    );

    let vp8x = &chunks[0].data;
    assert_eq!(vp8x.len(), 10);
    assert_eq!(
        vp8x[0], 0b0000_0010,
        "animation flag only: every pixel is opaque"
    );
    assert_eq!(images::u24(&vp8x[4..7]) + 1, 2, "canvas width");
    assert_eq!(images::u24(&vp8x[7..10]) + 1, 1, "canvas height");
    assert_eq!(
        chunks[1].data,
        [0, 0, 0, 0, 0, 0],
        "transparent background, loop forever"
    );

    for c in &chunks[2..] {
        let f = anmf(c);
        assert_eq!((f.x, f.y, f.width, f.height), (0, 0, 2, 1));
        assert_eq!(f.duration_ms, 100);
        assert_eq!(f.flags, 0b10, "do not blend, do not dispose");
        assert_eq!(f.chunks.len(), 1);
        assert_eq!(f.chunks[0].fourcc, "VP8L");
    }
}

#[test]
fn alpha_flag_follows_the_pixels() {
    let opaque = Image::new(1, 1, vec![OPAQUE]);
    let half = Image::new(1, 1, vec![HALF]);
    let (file, _) = webp::animated(&[frame(&opaque, 10), frame(&half, 10)]).unwrap();
    assert_eq!(
        webp_chunks(&file)[0].data[0],
        0b0001_0010,
        "animation and alpha"
    );
    let (file, _) = webp::animated(&[frame(&opaque, 10)]).unwrap();
    assert_eq!(webp_chunks(&file)[0].data[0], 0b0000_0010);
}

#[test]
fn transparent_pixel_replaces_an_opaque_one() {
    let a = Image::new(2, 1, vec![OPAQUE, OPAQUE]);
    let b = Image::new(2, 1, vec![CLEAR, OPAQUE]);
    let (file, _) = webp::animated(&[frame(&a, 50), frame(&b, 50)]).unwrap();
    let decoded = decode_webp_frames(&file);
    assert_eq!(decoded[1].0[0], CLEAR);
}

#[test]
fn animated_round_trip_with_durations() {
    let a = Image::new(2, 2, vec![CLEAR, HALF, OPAQUE, CLEAR]);
    let b = Image::new(2, 2, vec![OPAQUE, CLEAR, HALF, HALF]);
    let c = img(&["xy", "yx"]);
    let (file, _) = webp::animated(&[frame(&a, 333), frame(&b, 334), frame(&c, 333)]).unwrap();
    let decoded = decode_webp_frames(&file);
    let expected = [(&a, 333), (&b, 334), (&c, 333)];
    assert_eq!(decoded.len(), 3);
    for ((pixels, duration), (image, ms)) in decoded.iter().zip(expected) {
        assert_eq!(pixels.as_slice(), image.pixels());
        assert_eq!(*duration, ms);
    }
}

#[test]
fn single_frame_is_still_animated() {
    let a = img(&["a"]);
    let (file, count) = webp::animated(&[frame(&a, 1000)]).unwrap();
    assert_eq!(count, 1);
    assert_eq!(names(&file), ["VP8X", "ANIM", "ANMF"]);
}

#[test]
fn long_frame_is_split() {
    let a = img(&["a"]);
    let b = img(&["b"]);
    // 20000 s, then 1 s.
    let (file, count) = webp::animated(&[frame(&a, 20_000_000), frame(&b, 1000)]).unwrap();
    assert_eq!(count, 3);
    let chunks = webp_chunks(&file);
    let durations: Vec<u32> = chunks[2..].iter().map(|c| anmf(c).duration_ms).collect();
    assert_eq!(durations, [16_777_215, 3_222_785, 1000]);
    assert_eq!(
        anmf(&chunks[2]).chunks,
        anmf(&chunks[3]).chunks,
        "identical frames"
    );
}

/// The `alpha_is_used` bit of a `VP8L` bitstream (after the signature and the
/// two 14-bit sizes).
fn vp8l_alpha(bitstream: &[u8]) -> bool {
    assert_eq!(bitstream[0], 0x2f, "VP8L signature");
    let bits = u32::from_le_bytes(bitstream[1..5].try_into().unwrap());
    bits >> 28 & 1 == 1
}

#[test]
fn vp8l_alpha_agrees_with_the_vp8x_flag() {
    let opaque = Image::new(1, 1, vec![OPAQUE]);
    let half = Image::new(1, 1, vec![HALF]);
    for (frames, alpha) in [
        (vec![frame(&opaque, 10), frame(&opaque, 10)], false),
        (vec![frame(&opaque, 10), frame(&half, 10)], true),
    ] {
        let (file, _) = webp::animated(&frames).unwrap();
        let chunks = webp_chunks(&file);
        assert_eq!(chunks[0].data[0] & 0b1_0000 != 0, alpha);
        for c in &chunks[2..] {
            assert_eq!(vp8l_alpha(&anmf(c).chunks[0].data), alpha, "every frame");
        }
    }
    let still = |image: &Image| vp8l_alpha(&webp_chunks(&webp::still(image))[0].data);
    assert!(!still(&opaque));
    assert!(still(&half));
}
