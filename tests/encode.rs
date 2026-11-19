//! PNG encoding: color count, palette order, chunks, round trip, determinism.

#[path = "common/images.rs"]
mod images;

use images::{chunk_data, chunks, decode, header};
use pixel_perfect::encode::{self, Encoding};
use pixel_perfect::model::Rgba;
use pixel_perfect::raster::{Image, TRANSPARENT};

/// A 1-row image with `n` distinct opaque colors.
fn with_colors(n: u32) -> Image {
    Image::from_fn(n, 1, |x, _| {
        Rgba([(x % 256) as u8, (x / 256) as u8, 7, 255])
    })
}

#[test]
fn bit_depth_follows_color_count() {
    let cases = [
        (1, 1),
        (2, 1),
        (3, 2),
        (4, 2),
        (5, 4),
        (16, 4),
        (17, 8),
        (256, 8),
    ];
    for (colors, depth) in cases {
        let png = encode::png(&with_colors(colors));
        assert_eq!(header(&png).2, depth, "{colors} colors");
        assert_eq!(header(&png).3, 3, "{colors} colors: indexed");
    }
}

#[test]
fn more_than_256_colors_is_rgba() {
    for colors in [257, 300] {
        let png = encode::png(&with_colors(colors));
        let (_, _, depth, color_type) = header(&png);
        assert_eq!((depth, color_type), (8, 6), "{colors} colors: RGBA8");
        assert!(chunk_data(&png, "PLTE").is_none());
    }
}

#[test]
fn three_colors_plus_transparency_is_two_bits() {
    let red = Rgba([255, 0, 0, 255]);
    let green = Rgba([0, 255, 0, 255]);
    let blue = Rgba([0, 0, 255, 255]);
    let image = Image::new(4, 1, vec![red, TRANSPARENT, green, blue]);
    assert_eq!(header(&encode::png(&image)).2, 2);
}

#[test]
fn palette_puts_non_opaque_first_by_first_appearance() {
    let red = Rgba([255, 0, 0, 255]);
    let green = Rgba([0, 255, 0, 255]);
    let half_blue = Rgba([0, 0, 255, 128]);
    let image = Image::new(5, 1, vec![red, TRANSPARENT, green, half_blue, red]);
    assert_eq!(
        encode::choose(&image),
        Encoding::Indexed {
            palette: vec![TRANSPARENT, half_blue, red, green],
            transparent: 2,
            bit_depth: 2,
        }
    );
    let png = encode::png(&image);
    assert_eq!(
        chunk_data(&png, "PLTE").unwrap(),
        [0, 0, 0, 0, 0, 255, 255, 0, 0, 0, 255, 0]
    );
    assert_eq!(chunk_data(&png, "tRNS").unwrap(), [0, 128]);
}

#[test]
fn opaque_image_has_no_trns() {
    let png = encode::png(&with_colors(3));
    assert_eq!(chunks(&png), ["IHDR", "PLTE", "IDAT", "IEND"]);
}

#[test]
fn only_minimal_chunks_are_written() {
    let translucent = Image::new(2, 1, vec![TRANSPARENT, Rgba([1, 2, 3, 255])]);
    assert_eq!(
        chunks(&encode::png(&translucent)),
        ["IHDR", "PLTE", "tRNS", "IDAT", "IEND"]
    );
    assert_eq!(
        chunks(&encode::png(&with_colors(300))),
        ["IHDR", "IDAT", "IEND"]
    );
}

#[test]
fn round_trip_is_lossless() {
    let odd_width = Image::from_fn(13, 5, |x, y| match (x + y) % 3 {
        0 => TRANSPARENT,
        1 => Rgba([10, 20, 30, 77]),
        _ => Rgba([x as u8, y as u8, 200, 255]),
    });
    let many = Image::from_fn(40, 10, |x, y| {
        Rgba([x as u8 * 6, y as u8 * 20, (x * y) as u8, (x + 1) as u8])
    });
    for image in [
        odd_width,
        many,
        with_colors(1),
        with_colors(5),
        with_colors(17),
    ] {
        let (w, h, pixels) = decode(&encode::png(&image));
        assert_eq!((w, h), (image.width(), image.height()));
        assert_eq!(pixels, image.pixels());
    }
}

#[test]
fn same_image_same_bytes() {
    let image = Image::from_fn(9, 9, |x, y| Rgba([(x * 20) as u8, (y * 20) as u8, 0, 255]));
    assert_eq!(encode::png(&image), encode::png(&image.clone()));
}
