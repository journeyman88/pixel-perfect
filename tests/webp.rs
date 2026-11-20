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

// Blend verification (see openspec change `optimize-animation-rendering`,
// `blend-verification.md`).

/// The `VP8L` bitstream `image-webp` writes for RGBA pixels.
fn raw_vp8l(pixels: &[[u8; 4]], width: u32, height: u32) -> Vec<u8> {
    let data: Vec<u8> = pixels.iter().flatten().copied().collect();
    let mut file = Vec::new();
    image_webp::WebPEncoder::new(&mut file)
        .encode(&data, width, height, image_webp::ColorType::Rgba8)
        .unwrap();
    webp_chunks(&file).remove(0).data
}

fn raw_anmf(bitstream: &[u8], width: u32, height: u32, blend: bool) -> Vec<u8> {
    let u24 = |v: u32| v.to_le_bytes()[..3].to_vec();
    let mut payload = [u24(0), u24(0), u24(width - 1), u24(height - 1), u24(100)].concat();
    payload.push(if blend { 0 } else { 0b10 });
    payload.extend(raw_chunk(b"VP8L", bitstream));
    raw_chunk(b"ANMF", &payload)
}

/// Opaque source pixels over every channel value and several destination
/// alphas must replace the canvas; transparent ones must keep it. Fails with
/// `image-webp` 0.2.4, which blends opaque pixels too and turns a channel `c`
/// into `c - 1`: the reason masked frames are not used for WebP. Run with
/// `cargo test --test webp -- --ignored` to re-check after an update.
#[test]
#[ignore = "image-webp 0.2.4 does not composite opaque-over exactly"]
fn masked_blend_is_exact_in_image_webp() {
    let (w, h) = (256u32, 10u32);
    let alphas = [0u8, 1, 128, 254, 255];
    let dst = |x: u32, y: u32| -> [u8; 4] {
        let a = alphas[(y % 5) as usize];
        if a == 0 {
            [0; 4]
        } else {
            [255 - x as u8, x as u8, (x * 7 % 256) as u8, a]
        }
    };
    let src = |x: u32, y: u32| -> [u8; 4] {
        if y < 5 {
            [x as u8, 255 - x as u8, (x * 13 % 256) as u8, 255]
        } else {
            [0; 4]
        }
    };
    let grid = |f: &dyn Fn(u32, u32) -> [u8; 4]| -> Vec<[u8; 4]> {
        (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .map(|(x, y)| f(x, y))
            .collect()
    };
    let u24 = |v: u32| v.to_le_bytes()[..3].to_vec();
    let file = raw_file(&[
        raw_chunk(
            b"VP8X",
            &[[0b0001_0010, 0, 0, 0].to_vec(), u24(w - 1), u24(h - 1)].concat(),
        ),
        raw_chunk(b"ANIM", &[0; 6]),
        raw_anmf(&raw_vp8l(&grid(&dst), w, h), w, h, false),
        raw_anmf(&raw_vp8l(&grid(&src), w, h), w, h, true),
    ]);
    let frames = decode_webp_frames(&file);
    let expected: Vec<Rgba> = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .map(|(x, y)| Rgba(if y < 5 { src(x, y) } else { dst(x, y) }))
        .collect();
    let wrong: Vec<_> = frames[1]
        .0
        .iter()
        .zip(&expected)
        .enumerate()
        .filter(|(_, (g, e))| g != e)
        .map(|(i, (g, e))| (i as u32 % w, i as u32 / w, *g, *e))
        .collect();
    assert!(
        wrong.is_empty(),
        "{} wrong pixels, e.g. {:?}",
        wrong.len(),
        &wrong[..wrong.len().min(3)]
    );
}

// Changed-area frames

/// (x, y, width, height, flags) of every `ANMF`.
fn anmf_rects(file: &[u8]) -> Vec<(u32, u32, u32, u32, u8)> {
    webp_chunks(file)
        .iter()
        .filter(|c| c.fourcc == "ANMF")
        .map(|c| {
            let f = anmf(c);
            (f.x, f.y, f.width, f.height, f.flags)
        })
        .collect()
}

fn assert_webp_round_trip(images: &[&Image], file: &[u8]) {
    let decoded = decode_webp_frames(file);
    assert_eq!(decoded.len(), images.len());
    for (i, ((pixels, _), image)) in decoded.iter().zip(images).enumerate() {
        assert_eq!(pixels.as_slice(), image.pixels(), "frame {i}");
    }
}

#[test]
fn odd_offset_is_extended_to_even() {
    let a = Image::new(8, 6, vec![OPAQUE; 48]);
    let mut pixels = vec![OPAQUE; 48];
    pixels[3 * 8 + 5] = HALF;
    let b = Image::new(8, 6, pixels);
    let (file, _) = webp::animated(&[frame(&a, 100), frame(&b, 100)]).unwrap();
    let rects = anmf_rects(&file);
    assert_eq!(rects[0], (0, 0, 8, 6, 0b10), "first frame: full canvas");
    assert_eq!(
        rects[1],
        (4, 2, 2, 2, 0b10),
        "pixel (5, 3) in a 2x2 area at (4, 2)"
    );
    assert_webp_round_trip(&[&a, &b], &file);
}

#[test]
fn transparent_over_opaque_overwrites_the_area() {
    let a = Image::new(4, 1, vec![OPAQUE; 4]);
    let b = Image::new(4, 1, vec![OPAQUE, OPAQUE, CLEAR, OPAQUE]);
    let (file, _) = webp::animated(&[frame(&a, 100), frame(&b, 100)]).unwrap();
    assert_eq!(anmf_rects(&file)[1], (2, 0, 1, 1, 0b10));
    assert_eq!(decode_webp_frames(&file)[1].0[2], CLEAR);
}

#[test]
fn split_duration_repeats_as_1x1() {
    let a = img(&["ab", "cd"]);
    let b = img(&["ab", "ce"]);
    let (file, count) = webp::animated(&[frame(&a, 1000), frame(&b, 20_000_000)]).unwrap();
    assert_eq!(count, 3);
    let rects = anmf_rects(&file);
    assert_eq!(rects[1], (0, 0, 2, 2, 0b10), "(1, 1) extended to (0, 0)");
    assert_eq!(rects[2], (0, 0, 1, 1, 0b10), "nothing changes");
    assert_webp_round_trip(&[&a, &b, &b], &file);
}

#[test]
fn moving_square_round_trip() {
    let at = |dx: u32| {
        Image::from_fn(32, 16, |x, y| {
            if (3 + dx..11 + dx).contains(&x) && (5..13).contains(&y) {
                OPAQUE
            } else {
                HALF
            }
        })
    };
    let images: Vec<Image> = (0..6).map(at).collect();
    let frames: Vec<WebpFrame> = images.iter().map(|i| frame(i, 50)).collect();
    let (file, _) = webp::animated(&frames).unwrap();
    for r in &anmf_rects(&file)[1..] {
        // Rows 5..13, top edge extended to the even row 4.
        assert!(r.2 <= 10 && r.3 == 9, "small area, not 32x16: {r:?}");
        assert_eq!((r.0 % 2, r.1 % 2), (0, 0), "even offsets");
    }
    assert_webp_round_trip(&images.iter().collect::<Vec<_>>(), &file);
}
