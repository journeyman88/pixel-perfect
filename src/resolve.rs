// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 The pixel-perfect Authors

//! Resolution of palette and sprite inheritance.
//!
//! Resolution reports cycles and the per-resource rules that only make sense
//! on resolved data (row widths, overrides, palette characters). Missing
//! references are reported by `semantic`; here they just make the dependent
//! resource unresolved, so that one mistake does not cascade into many errors.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::error::Diagnostic;
use crate::model::{Rgba, Worksheet};

/// The transparent pixel character.
pub const TRANSPARENT: char = '.';
/// In `overrideLines`, copies the base sprite's character in the same column.
pub const PASS_THROUGH: char = '=';

/// A palette with its inheritance applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPalette {
    pub colors: BTreeMap<char, Rgba>,
}

/// A sprite with its inheritance applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSprite {
    /// Id of the palette resolving the characters.
    pub palette: String,
    /// Rows top to bottom, one `char` per pixel.
    pub rows: Vec<Vec<char>>,
}

impl ResolvedSprite {
    pub fn width(&self) -> u32 {
        self.rows.first().map_or(0, |r| r.len() as u32)
    }

    pub fn height(&self) -> u32 {
        self.rows.len() as u32
    }
}

#[derive(Clone)]
enum State<T> {
    Unvisited,
    InProgress,
    Done(Option<T>),
}

/// Formats an inheritance cycle as `a -> b -> a`.
fn cycle_message(kind: &str, ids: &[&str]) -> String {
    format!("{kind} inheritance cycle: {}", ids.join(" -> "))
}

/// Resolves every palette. Unresolvable palettes (cycles, missing bases) are
/// left out of the result.
pub fn resolve_palettes(
    ws: &Worksheet,
    index: &HashMap<&str, usize>,
    diagnostics: &mut Vec<Diagnostic>,
) -> BTreeMap<String, ResolvedPalette> {
    struct Ctx<'a> {
        ws: &'a Worksheet,
        index: &'a HashMap<&'a str, usize>,
        states: Vec<State<ResolvedPalette>>,
        stack: Vec<usize>,
    }

    fn visit(
        ctx: &mut Ctx,
        i: usize,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<ResolvedPalette> {
        match &ctx.states[i] {
            State::Done(result) => return result.clone(),
            State::InProgress => {
                let start = ctx.stack.iter().position(|&s| s == i).unwrap_or(0);
                let mut ids: Vec<&str> = ctx.stack[start..]
                    .iter()
                    .map(|&s| ctx.ws.palettes[s].id.as_str())
                    .collect();
                ids.push(&ctx.ws.palettes[i].id);
                let last = *ctx.stack.last().expect("in-progress node is on the stack");
                diagnostics.push(Diagnostic::at(
                    format!("/palettes/{last}/_palette"),
                    cycle_message("palette", &ids),
                ));
                return None;
            }
            State::Unvisited => {}
        }
        ctx.states[i] = State::InProgress;
        ctx.stack.push(i);
        let palette = &ctx.ws.palettes[i];
        let mut colors = match &palette.palette {
            None => Some(BTreeMap::new()),
            Some(base) => match ctx.index.get(base.as_str()) {
                Some(&b) => visit(ctx, b, diagnostics).map(|p| p.colors),
                None => None,
            },
        };
        if let Some(colors) = &mut colors {
            for color in &palette.colors {
                colors.insert(color.char, color.rgba);
            }
        }
        ctx.stack.pop();
        let result = colors.map(|colors| ResolvedPalette { colors });
        ctx.states[i] = State::Done(result.clone());
        result
    }

    let mut ctx = Ctx {
        ws,
        index,
        states: vec![State::Unvisited; ws.palettes.len()],
        stack: Vec::new(),
    };
    let mut resolved = BTreeMap::new();
    for i in 0..ws.palettes.len() {
        // Only the first palette with a given id is reachable by reference.
        if index.get(ws.palettes[i].id.as_str()) != Some(&i) {
            continue;
        }
        if let Some(p) = visit(&mut ctx, i, diagnostics) {
            resolved.insert(ws.palettes[i].id.clone(), p);
        }
    }
    resolved
}

/// Resolves every sprite against the resolved palettes. Sprites with errors
/// are left out of the result, so sprites derived from them are skipped too.
pub fn resolve_sprites(
    ws: &Worksheet,
    index: &HashMap<&str, usize>,
    palettes: &BTreeMap<String, ResolvedPalette>,
    diagnostics: &mut Vec<Diagnostic>,
) -> BTreeMap<String, ResolvedSprite> {
    struct Ctx<'a> {
        ws: &'a Worksheet,
        index: &'a HashMap<&'a str, usize>,
        palettes: &'a BTreeMap<String, ResolvedPalette>,
        states: Vec<State<ResolvedSprite>>,
        stack: Vec<usize>,
    }

    fn visit(ctx: &mut Ctx, i: usize, diagnostics: &mut Vec<Diagnostic>) -> Option<ResolvedSprite> {
        match &ctx.states[i] {
            State::Done(result) => return result.clone(),
            State::InProgress => {
                let start = ctx.stack.iter().position(|&s| s == i).unwrap_or(0);
                let mut ids: Vec<&str> = ctx.stack[start..]
                    .iter()
                    .map(|&s| ctx.ws.sprites[s].id.as_str())
                    .collect();
                ids.push(&ctx.ws.sprites[i].id);
                let last = *ctx.stack.last().expect("in-progress node is on the stack");
                diagnostics.push(Diagnostic::at(
                    format!("/sprites/{last}/_sprite"),
                    cycle_message("sprite", &ids),
                ));
                return None;
            }
            State::Unvisited => {}
        }
        ctx.states[i] = State::InProgress;
        ctx.stack.push(i);
        let result = resolve_one(ctx, i, diagnostics);
        ctx.stack.pop();
        ctx.states[i] = State::Done(result.clone());
        result
    }

    /// Where a resolved row comes from, for error locations.
    #[derive(Clone)]
    enum Origin {
        Line(usize),
        Override(String),
        Inherited,
    }

    fn resolve_one(
        ctx: &mut Ctx,
        i: usize,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<ResolvedSprite> {
        let sprite = &ctx.ws.sprites[i];
        let at = |suffix: &str| format!("/sprites/{i}{suffix}");
        let mut ok = true;

        let (palette, rows, origins) = match &sprite.sprite {
            None => {
                let lines = sprite.lines.as_deref().unwrap_or_default();
                let rows: Vec<Vec<char>> = lines.iter().map(|l| l.chars().collect()).collect();
                let width = rows.first().map_or(0, Vec::len);
                if let Some(r) = rows.iter().position(|row| row.len() != width) {
                    diagnostics.push(Diagnostic::at(
                        at(&format!("/lines/{r}")),
                        format!(
                            "row {r} is {} pixels wide, but row 0 is {width}",
                            rows[r].len()
                        ),
                    ));
                    return None;
                }
                let origins = (0..rows.len()).map(Origin::Line).collect();
                (sprite.palette.clone()?, rows, origins)
            }
            Some(base_id) => {
                let &b = ctx.index.get(base_id.as_str())?;
                let base = visit(ctx, b, diagnostics)?;
                let sprite = &ctx.ws.sprites[i];
                let palette = sprite.palette.clone().unwrap_or(base.palette.clone());
                let mut origins = vec![Origin::Inherited; base.rows.len()];
                let mut rows = base.rows.clone();
                let width = base.width() as usize;
                let mut overrides: Vec<(&String, &String)> =
                    sprite.override_lines.iter().flatten().collect();
                overrides.sort_by_key(|(k, _)| (k.len(), k.as_str()));
                for (key, value) in overrides {
                    let pointer = at(&format!("/overrideLines/{key}"));
                    let row = match key.parse::<usize>() {
                        Ok(r) if r < rows.len() => r,
                        _ => {
                            diagnostics.push(Diagnostic::at(
                                pointer,
                                format!(
                                    "row index {key} is out of range: base sprite {base_id:?} has {} rows",
                                    rows.len()
                                ),
                            ));
                            ok = false;
                            continue;
                        }
                    };
                    let chars: Vec<char> = value.chars().collect();
                    if chars.len() != width {
                        diagnostics.push(Diagnostic::at(
                            pointer,
                            format!(
                                "override is {} pixels wide, but base sprite {base_id:?} is {width}",
                                chars.len()
                            ),
                        ));
                        ok = false;
                        continue;
                    }
                    rows[row] = chars
                        .iter()
                        .zip(&base.rows[row])
                        .map(|(&c, &base_c)| if c == PASS_THROUGH { base_c } else { c })
                        .collect();
                    origins[row] = Origin::Override(key.clone());
                }
                (palette, rows, origins)
            }
        };
        let sprite = &ctx.ws.sprites[i];

        let resolved = ResolvedSprite { palette, rows };
        if let Some(w) = sprite.width {
            if w != resolved.width() {
                diagnostics.push(Diagnostic::at(
                    at("/width"),
                    format!(
                        "declared width {w} does not match the resolved width {}",
                        resolved.width()
                    ),
                ));
                ok = false;
            }
        }
        if let Some(h) = sprite.height {
            if h != resolved.height() {
                diagnostics.push(Diagnostic::at(
                    at("/height"),
                    format!(
                        "declared height {h} does not match the resolved height {}",
                        resolved.height()
                    ),
                ));
                ok = false;
            }
        }

        // Missing palettes are reported as broken references by `semantic`.
        let palette = ctx.palettes.get(&resolved.palette)?;
        let mut reported = BTreeSet::new();
        for (r, row) in resolved.rows.iter().enumerate() {
            for (c, &ch) in row.iter().enumerate() {
                if ch == TRANSPARENT || palette.colors.contains_key(&ch) || !reported.insert(ch) {
                    continue;
                }
                let pointer = match &origins[r] {
                    Origin::Line(l) => at(&format!("/lines/{l}")),
                    Origin::Override(k) => at(&format!("/overrideLines/{k}")),
                    Origin::Inherited => at("/_palette"),
                };
                diagnostics.push(Diagnostic::at(
                    pointer,
                    format!(
                        "character {ch:?} at row {r}, column {c} is not in palette {:?}",
                        resolved.palette
                    ),
                ));
                ok = false;
            }
        }

        ok.then_some(resolved)
    }

    let mut ctx = Ctx {
        ws,
        index,
        palettes,
        states: vec![State::Unvisited; ws.sprites.len()],
        stack: Vec::new(),
    };
    let mut resolved = BTreeMap::new();
    for i in 0..ws.sprites.len() {
        if index.get(ws.sprites[i].id.as_str()) != Some(&i) {
            continue;
        }
        if let Some(s) = visit(&mut ctx, i, diagnostics) {
            resolved.insert(ws.sprites[i].id.clone(), s);
        }
    }
    resolved
}
