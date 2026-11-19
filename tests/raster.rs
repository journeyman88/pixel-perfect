// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 The pixel-perfect Authors

//! Image buffer and transforms: scenarios of the sprite-rendering spec.

#[path = "common/images.rs"]
mod images;

use images::{img, rows};
use pixel_perfect::model::{Axis, Rgba, Rotation, Transform};
use pixel_perfect::raster::{Image, TRANSPARENT, apply, apply_all};

fn flip(axis: Axis) -> Transform {
    Transform::Flip { axis }
}

fn rotate(degrees: Rotation) -> Transform {
    Transform::Rotate { degrees }
}

#[test]
fn transparent_pixels_are_canonical() {
    let image = Image::new(2, 1, vec![Rgba([200, 10, 10, 0]), Rgba([1, 2, 3, 4])]);
    assert_eq!(image.pixels(), [TRANSPARENT, Rgba([1, 2, 3, 4])]);
}

#[test]
fn horizontal_flip() {
    assert_eq!(rows(&apply(&img(&["ABC"]), &flip(Axis::X))), ["CBA"]);
}

#[test]
fn vertical_flip() {
    assert_eq!(
        rows(&apply(&img(&["A", "B", "C"]), &flip(Axis::Y))),
        ["C", "B", "A"]
    );
}

#[test]
fn rotate_90() {
    let r = apply(&img(&["AB", "CD", "EF"]), &rotate(Rotation::Deg90));
    assert_eq!((r.width(), r.height()), (3, 2));
    assert_eq!(rows(&r), ["ECA", "FDB"]);
}

#[test]
fn rotate_180() {
    assert_eq!(
        rows(&apply(&img(&["AB", "CD"]), &rotate(Rotation::Deg180))),
        ["DC", "BA"]
    );
}

#[test]
fn rotate_270() {
    let r = apply(&img(&["AB", "CD", "EF"]), &rotate(Rotation::Deg270));
    assert_eq!(rows(&r), ["BDF", "ACE"]);
}

#[test]
fn four_quarter_turns_are_identity() {
    let original = img(&["ABC", "DEF"]);
    let turned = apply_all(&original, &[rotate(Rotation::Deg90); 4]);
    assert_eq!(turned, original);
}

#[test]
fn scale_integer_upscale() {
    let s = apply(
        &img(&["AB"]),
        &Transform::Scale {
            width: 4,
            height: 2,
        },
    );
    assert_eq!(rows(&s), ["AABB", "AABB"]);
}

#[test]
fn scale_downscale() {
    let s = apply(
        &img(&["ABCD"]),
        &Transform::Scale {
            width: 2,
            height: 1,
        },
    );
    assert_eq!(rows(&s), ["AC"]);
}

#[test]
fn scale_non_integer_ratio() {
    let s = apply(
        &img(&["AB"]),
        &Transform::Scale {
            width: 3,
            height: 1,
        },
    );
    assert_eq!(rows(&s), ["AAB"]);
}

#[test]
fn crop_inner_area() {
    let c = apply(
        &img(&["ABC", "DEF", "GHI"]),
        &Transform::Crop { area: [1, 1, 2, 2] },
    );
    assert_eq!(rows(&c), ["EF", "HI"]);
}

fn tile(width: u32, height: u32) -> Transform {
    Transform::Tile { width, height }
}

#[test]
fn tile_repeats_horizontally() {
    assert_eq!(rows(&apply(&img(&["AB"]), &tile(5, 1))), ["ABABA"]);
}

#[test]
fn tile_repeats_in_both_directions() {
    assert_eq!(
        rows(&apply(&img(&["AB", "CD"]), &tile(3, 3))),
        ["ABA", "CDC", "ABA"]
    );
}

#[test]
fn tile_smaller_than_the_image() {
    assert_eq!(rows(&apply(&img(&["ABC", "DEF"]), &tile(2, 1))), ["AB"]);
}

#[test]
fn tile_to_the_same_size_is_identity() {
    let original = img(&["AB.", "CDE"]);
    assert_eq!(apply(&original, &tile(3, 2)), original);
}

#[test]
fn tile_repeats_transparency() {
    assert_eq!(rows(&apply(&img(&["A."]), &tile(4, 1))), ["A.A."]);
}

#[test]
fn tile_in_a_chain() {
    let r = apply_all(&img(&["AB"]), &[rotate(Rotation::Deg90), tile(3, 4)]);
    assert_eq!(rows(&r), ["AAA", "BBB", "AAA", "BBB"]);
}

#[test]
fn chain_order_matters() {
    let r = apply_all(&img(&["AB"]), &[rotate(Rotation::Deg90), flip(Axis::Y)]);
    assert_eq!(rows(&r), ["B", "A"]);
    let other = apply_all(&img(&["AB"]), &[flip(Axis::Y), rotate(Rotation::Deg90)]);
    assert_eq!(rows(&other), ["A", "B"]);
}

#[test]
fn upscale_turns_pixels_into_blocks() {
    let u = img(&["AB"]).upscale(3);
    assert_eq!(rows(&u), ["AAABBB", "AAABBB", "AAABBB"]);
}
