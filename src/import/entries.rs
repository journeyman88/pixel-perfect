// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 The pixel-perfect Authors

//! Palette entries and sprite lines for an imported image.

use std::collections::{HashMap, HashSet};

use crate::edit::Node;
use crate::import::alphabet;
use crate::model::{Rgba, Worksheet};
use crate::raster::{Image, TRANSPARENT};
use crate::resolve;

/// Writes an 8-bit alpha as the decimal the engine converts back to it:
/// `None` for 255 (omitted), otherwise the shortest, then smallest, decimal
/// `a` with `Rgba::alpha_to_u8(a) == alpha`.
pub fn format_alpha(alpha: u8) -> Option<String> {
    if alpha == 255 {
        return None;
    }
    if alpha == 0 {
        return Some("0".into());
    }
    for digits in 1..=3u32 {
        let scale = 10u32.pow(digits);
        for k in 1..scale {
            let a = f64::from(k) / f64::from(scale);
            if Rgba::alpha_to_u8(a) == alpha {
                let text = format!("{a:.prec$}", prec = digits as usize);
                return Some(text);
            }
        }
    }
    unreachable!("three digits always suffice for alpha {alpha}")
}

/// The `rgba` node for a color, in the shortest form.
pub fn rgba_node(color: Rgba) -> Node {
    let [r, g, b, a] = color.0;
    let mut items = vec![
        Node::Num(r.to_string()),
        Node::Num(g.to_string()),
        Node::Num(b.to_string()),
    ];
    if let Some(alpha) = format_alpha(a) {
        items.push(Node::Num(alpha));
    }
    Node::flow_seq(items)
}

/// The sprite lines for `image`, mapping colors through `chars`.
fn lines(image: &Image, chars: &HashMap<Rgba, char>) -> Vec<String> {
    image
        .pixels()
        .chunks(image.width() as usize)
        .map(|row| {
            row.iter()
                .map(|p| match *p {
                    TRANSPARENT => resolve::TRANSPARENT,
                    color => chars[&color],
                })
                .collect()
        })
        .collect()
}

/// Palette entries, and sprite lines using them.
pub type Generated = (Vec<(char, Rgba)>, Vec<String>);

/// A new palette for `image`: one entry per distinct visible color, in order
/// of first appearance, with characters taken from the import alphabet.
/// Returns the entries and the sprite lines.
pub fn new_palette(image: &Image, max_colors: usize) -> Result<Generated, String> {
    let mut order: Vec<Rgba> = Vec::new();
    let mut seen: HashSet<Rgba> = HashSet::new();
    for &p in image.pixels() {
        if p != TRANSPARENT && seen.insert(p) {
            order.push(p);
        }
    }
    if order.is_empty() {
        return Err("the image has no visible pixel: every pixel is fully transparent".into());
    }
    if order.len() > max_colors {
        return Err(format!(
            "the image has {} colors, more than --max-colors {max_colors}; the importer does not \
             quantize: reduce the colors in an image editor, or raise --max-colors (up to {})",
            order.len(),
            alphabet::len()
        ));
    }
    let entries: Vec<(char, Rgba)> = order
        .into_iter()
        .enumerate()
        .map(|(i, color)| (alphabet::nth(i).expect("limit checked"), color))
        .collect();
    let chars = entries.iter().map(|&(c, color)| (color, c)).collect();
    Ok((entries, lines(image, &chars)))
}

/// The entries of palette `id` after inheritance, in resolution order: the
/// base palette's entries first (an override keeps its base position), then
/// the new characters in the order they are declared. `None` if there is no
/// such palette. The worksheet must be valid (no cycles).
pub fn resolved_entries(ws: &Worksheet, id: &str) -> Option<Vec<(char, Rgba)>> {
    let palette = ws.palettes.iter().find(|p| p.id == id)?;
    let mut entries = match &palette.palette {
        Some(base) => resolved_entries(ws, base)?,
        None => Vec::new(),
    };
    for color in &palette.colors {
        match entries.iter_mut().find(|(c, _)| *c == color.char) {
            Some(entry) => entry.1 = color.rgba,
            None => entries.push((color.char, color.rgba)),
        }
    }
    Some(entries)
}

/// Sprite lines for `image` using the existing palette `entries`: each color
/// takes the first character with the same 8-bit color.
pub fn reuse_palette(
    image: &Image,
    entries: &[(char, Rgba)],
    palette_id: &str,
) -> Result<Vec<String>, String> {
    let mut chars: HashMap<Rgba, char> = HashMap::new();
    for &(c, color) in entries.iter().rev() {
        chars.insert(color, c);
    }
    let mut missing: Vec<(Rgba, u32, u32)> = Vec::new();
    let mut reported: HashSet<Rgba> = HashSet::new();
    for (i, &p) in image.pixels().iter().enumerate() {
        if p != TRANSPARENT && !chars.contains_key(&p) && reported.insert(p) {
            let w = image.width() as usize;
            missing.push((p, (i % w) as u32, (i / w) as u32));
        }
    }
    if !missing.is_empty() {
        let list: Vec<String> = missing
            .iter()
            .take(10)
            .map(|(color, x, y)| {
                let [r, g, b, a] = color.0;
                let rgba = if a == 255 {
                    format!("({r}, {g}, {b})")
                } else {
                    format!("({r}, {g}, {b}, {})", format_alpha(a).unwrap_or_default())
                };
                format!("{rgba} at ({x}, {y})")
            })
            .collect();
        return Err(format!(
            "palette {palette_id:?} lacks {} color(s) of the image: {}{}",
            missing.len(),
            list.join(", "),
            if missing.len() > 10 { ", ..." } else { "" }
        ));
    }
    Ok(lines(image, &chars))
}

/// The palette node `{ id, colors }`.
pub fn palette_node(id: &str, entries: &[(char, Rgba)]) -> Node {
    let colors = entries
        .iter()
        .map(|&(c, color)| {
            Node::flow_map(vec![
                ("char".into(), Node::Str(c.to_string())),
                ("rgba".into(), rgba_node(color)),
            ])
        })
        .collect();
    Node::map(vec![
        ("id".into(), Node::Str(id.into())),
        ("colors".into(), Node::seq(colors)),
    ])
}

/// The sprite node `{ id, description?, _palette, lines }`.
pub fn sprite_node(id: &str, description: Option<&str>, palette: &str, lines: &[String]) -> Node {
    let mut entries = vec![("id".into(), Node::Str(id.into()))];
    if let Some(d) = description {
        entries.push(("description".into(), Node::Str(d.into())));
    }
    entries.push(("_palette".into(), Node::Str(palette.into())));
    entries.push((
        "lines".into(),
        Node::seq(lines.iter().map(|l| Node::Quoted(l.clone())).collect()),
    ));
    Node::map(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgba(r: u8, g: u8, b: u8, a: u8) -> Rgba {
        Rgba([r, g, b, a])
    }

    #[test]
    fn alpha_round_trips_for_every_value() {
        for a in 0..=255u8 {
            match format_alpha(a) {
                None => assert_eq!(a, 255),
                Some(text) => {
                    let parsed: f64 = text.parse().unwrap();
                    assert_eq!(Rgba::alpha_to_u8(parsed), a, "{text}");
                    let digits = text.split('.').nth(1).map_or(0, str::len);
                    assert!(digits <= 3, "{text}");
                }
            }
        }
    }

    #[test]
    fn alpha_uses_the_shortest_then_smallest_decimal() {
        assert_eq!(format_alpha(128).as_deref(), Some("0.5"));
        assert_eq!(format_alpha(153).as_deref(), Some("0.6"));
        assert_eq!(format_alpha(255), None);
        // 0.35 * 255 = 89.25 -> 89; no one-digit decimal gives 89.
        assert_eq!(format_alpha(89).as_deref(), Some("0.35"));
    }

    #[test]
    fn characters_follow_first_appearance() {
        let (red, blue, green) = (
            rgba(255, 0, 0, 255),
            rgba(0, 0, 255, 255),
            rgba(0, 255, 0, 255),
        );
        let image = Image::new(3, 2, vec![red, red, blue, green, blue, TRANSPARENT]);
        let (entries, lines) = new_palette(&image, 256).unwrap();
        assert_eq!(entries, vec![('a', red), ('b', blue), ('c', green)]);
        assert_eq!(lines, vec!["aab", "cb."]);
    }

    #[test]
    fn many_colors_continue_into_tier_one() {
        let pixels: Vec<Rgba> = (0..288u32)
            .map(|i| rgba((i % 256) as u8, (i / 256) as u8, 7, 255))
            .collect();
        let image = Image::new(16, 18, pixels);
        assert!(new_palette(&image, 256).unwrap_err().contains("288 colors"));
        let (entries, _) = new_palette(&image, 300).unwrap();
        assert_eq!(entries[0].0, 'a');
        assert_eq!(entries[91].0, '`');
        assert_eq!(entries[92].0, '¡');
        let tier1: HashSet<char> = alphabet::chars().take(alphabet::tier1_len()).collect();
        assert!(entries.iter().all(|(c, _)| tier1.contains(c)));
    }

    #[test]
    fn too_many_colors_is_not_quantized() {
        let pixels: Vec<Rgba> = (0..300u32)
            .map(|i| rgba(i as u8, (i >> 8) as u8, 0, 255))
            .collect();
        let err = new_palette(&Image::new(300, 1, pixels), 256).unwrap_err();
        assert!(
            err.contains("300 colors, more than --max-colors 256"),
            "{err}"
        );
        assert!(err.contains("does not quantize"), "{err}");
    }

    #[test]
    fn beyond_tier_one() {
        let n = alphabet::tier1_len() + 10;
        let pixels: Vec<Rgba> = (0..n as u32)
            .map(|i| rgba(i as u8, (i >> 8) as u8, (i >> 16) as u8, 255))
            .collect();
        let (entries, lines) = new_palette(&Image::new(n as u32, 1, pixels), n).unwrap();
        assert_eq!(entries.len(), n);
        assert_eq!(entries[n - 1].0, alphabet::nth(n - 1).unwrap());
        assert_eq!(lines[0].chars().count(), n);
    }

    #[test]
    fn fully_transparent_image_is_rejected() {
        let err = new_palette(&Image::transparent(2, 2), 256).unwrap_err();
        assert!(err.contains("no visible pixel"), "{err}");
    }

    #[test]
    fn reuse_picks_the_first_matching_character() {
        let red = rgba(255, 0, 0, 255);
        let entries = [('x', red), ('y', red)];
        let image = Image::new(2, 1, vec![red, TRANSPARENT]);
        assert_eq!(reuse_palette(&image, &entries, "p").unwrap(), vec!["x."]);
    }

    #[test]
    fn reuse_reports_missing_colors() {
        let image = Image::from_fn(3, 8, |x, y| {
            if (x, y) == (2, 7) {
                rgba(255, 0, 255, 255)
            } else {
                rgba(1, 1, 1, 255)
            }
        });
        let err = reuse_palette(&image, &[('a', rgba(1, 1, 1, 255))], "slime").unwrap_err();
        assert!(err.contains("lacks 1 color(s)"), "{err}");
        assert!(err.contains("(255, 0, 255) at (2, 7)"), "{err}");
    }
}
