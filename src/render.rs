//! Rasterization of resolved resources into images.

use crate::raster::{Image, TRANSPARENT};
use crate::resolve;
use crate::semantic::ResolvedWorksheet;

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
