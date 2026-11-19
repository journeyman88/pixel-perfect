// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 The pixel-perfect Authors

//! Rasterization of resolved resources into images.

use crate::raster::{Image, TRANSPARENT};
use crate::semantic::ResolvedWorksheet;
use crate::{compose, resolve};

/// Rasterizes the sprite `id`: each character becomes its palette color and
/// `.` becomes transparent.
///
/// Panics if `id` is not a resolved sprite; callers pass ids from a validated
/// worksheet.
pub fn sprite(ws: &ResolvedWorksheet, id: &str) -> Image {
    let sprite = &ws.sprites[id];
    let palette = &ws.palettes[&sprite.palette];
    Image::from_fn(sprite.width(), sprite.height(), |x, y| {
        match sprite.rows[y as usize][x as usize] {
            resolve::TRANSPARENT => TRANSPARENT,
            c => palette.colors[&c],
        }
    })
}

/// Renders the static resource `id` of `collection` (`sprites`, `panels` or
/// `scenes`), before `--scale`.
///
/// Panics for other collections and for animated panels and scenes; the export
/// plan only passes supported resources.
pub fn resource(ws: &ResolvedWorksheet, collection: &str, id: &str) -> Image {
    match collection {
        "sprites" => sprite(ws, id),
        "panels" => compose::panel(ws, id),
        "scenes" => compose::scene(ws, id),
        other => unreachable!("{other} cannot be rendered as a static image"),
    }
}
