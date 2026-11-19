//! Semantic validation: the rules the schema marks as `$comment: "Engine: ..."`.

use std::collections::{BTreeMap, HashMap};

use crate::error::Diagnostic;
use crate::model::{ElementSource, Rotation, Transform, Transition, Worksheet};
use crate::resolve::{self, ResolvedPalette, ResolvedSprite};

/// A worksheet that passed every check, with inheritance resolved.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedWorksheet {
    pub model: Worksheet,
    pub palettes: BTreeMap<String, ResolvedPalette>,
    pub sprites: BTreeMap<String, ResolvedSprite>,
}

/// Maps each id to the index of its first occurrence.
fn index_by_id<'a, T>(items: &'a [T], id: impl Fn(&'a T) -> &'a str) -> HashMap<&'a str, usize> {
    let mut index = HashMap::new();
    for (i, item) in items.iter().enumerate() {
        index.entry(id(item)).or_insert(i);
    }
    index
}

/// Reports every occurrence of an id after the first.
fn check_unique<'a>(
    ids: impl Iterator<Item = &'a str>,
    kind: &str,
    pointer: impl Fn(usize) -> String,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut seen: HashMap<&str, usize> = HashMap::new();
    for (i, id) in ids.enumerate() {
        if let Some(first) = seen.get(id) {
            diagnostics.push(Diagnostic::at(
                pointer(i),
                format!("duplicate {kind} id {id:?} (first defined at index {first})"),
            ));
        } else {
            seen.insert(id, i);
        }
    }
}

/// Runs every semantic check on a schema-valid worksheet.
pub fn check(ws: Worksheet) -> Result<ResolvedWorksheet, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

    // Unique ids.
    check_unique(
        ws.palettes.iter().map(|p| p.id.as_str()),
        "palette",
        |i| format!("/palettes/{i}/id"),
        &mut diagnostics,
    );
    check_unique(
        ws.sprites.iter().map(|s| s.id.as_str()),
        "sprite",
        |i| format!("/sprites/{i}/id"),
        &mut diagnostics,
    );
    check_unique(
        ws.flipbooks.iter().map(|f| f.id.as_str()),
        "flipbook",
        |i| format!("/flipbooks/{i}/id"),
        &mut diagnostics,
    );
    check_unique(
        ws.panels.iter().map(|p| p.id.as_str()),
        "panel",
        |i| format!("/panels/{i}/id"),
        &mut diagnostics,
    );
    check_unique(
        ws.timelines.iter().map(|t| t.id.as_str()),
        "timeline",
        |i| format!("/timelines/{i}/id"),
        &mut diagnostics,
    );
    check_unique(
        ws.scenes.iter().map(|s| s.id.as_str()),
        "scene",
        |i| format!("/scenes/{i}/id"),
        &mut diagnostics,
    );
    for (p, panel) in ws.panels.iter().enumerate() {
        let element_ids: Vec<(usize, &str)> = panel
            .elements
            .iter()
            .enumerate()
            .filter_map(|(e, el)| el.id.as_deref().map(|id| (e, id)))
            .collect();
        let mut seen: HashMap<&str, usize> = HashMap::new();
        for (e, id) in element_ids {
            if let Some(first) = seen.get(id) {
                diagnostics.push(Diagnostic::at(
                    format!("/panels/{p}/elements/{e}/id"),
                    format!("duplicate element id {id:?} in panel {:?} (first defined at index {first})", panel.id),
                ));
            } else {
                seen.insert(id, e);
            }
        }
    }

    let palettes = index_by_id(&ws.palettes, |p| p.id.as_str());
    let sprites = index_by_id(&ws.sprites, |s| s.id.as_str());
    let flipbooks = index_by_id(&ws.flipbooks, |f| f.id.as_str());
    let panels = index_by_id(&ws.panels, |p| p.id.as_str());
    let timelines = index_by_id(&ws.timelines, |t| t.id.as_str());
    let scenes = index_by_id(&ws.scenes, |s| s.id.as_str());

    // References.
    let mut reference = |index: &HashMap<&str, usize>, kind: &str, id: &str, pointer: String| {
        if !index.contains_key(id) {
            diagnostics.push(Diagnostic::at(pointer, format!("unknown {kind} {id:?}")));
        }
    };
    for (i, p) in ws.palettes.iter().enumerate() {
        if let Some(base) = &p.palette {
            reference(
                &palettes,
                "palette",
                base,
                format!("/palettes/{i}/_palette"),
            );
        }
    }
    for (i, s) in ws.sprites.iter().enumerate() {
        if let Some(base) = &s.sprite {
            reference(&sprites, "sprite", base, format!("/sprites/{i}/_sprite"));
        }
        if let Some(palette) = &s.palette {
            reference(
                &palettes,
                "palette",
                palette,
                format!("/sprites/{i}/_palette"),
            );
        }
    }
    for (i, f) in ws.flipbooks.iter().enumerate() {
        for (j, frame) in f.frames.iter().enumerate() {
            reference(
                &sprites,
                "sprite",
                &frame.sprite,
                format!("/flipbooks/{i}/frames/{j}/_sprite"),
            );
        }
    }
    for (i, p) in ws.panels.iter().enumerate() {
        for (j, el) in p.elements.iter().enumerate() {
            match el.source() {
                ElementSource::Sprite(s) => reference(
                    &sprites,
                    "sprite",
                    s,
                    format!("/panels/{i}/elements/{j}/_sprite"),
                ),
                ElementSource::Flipbook(f) => reference(
                    &flipbooks,
                    "flipbook",
                    f,
                    format!("/panels/{i}/elements/{j}/_flipbook"),
                ),
            }
        }
    }
    for (i, t) in ws.timelines.iter().enumerate() {
        reference(&panels, "panel", &t.panel, format!("/timelines/{i}/_panel"));
    }
    for (i, s) in ws.scenes.iter().enumerate() {
        for (j, layer) in s.layers.iter().enumerate() {
            if let Some(p) = &layer.panel {
                reference(
                    &panels,
                    "panel",
                    p,
                    format!("/scenes/{i}/layers/{j}/_panel"),
                );
            }
            if let Some(t) = &layer.timeline {
                reference(
                    &timelines,
                    "timeline",
                    t,
                    format!("/scenes/{i}/layers/{j}/_timeline"),
                );
            }
        }
    }
    for (i, t) in ws.timelines.iter().enumerate() {
        let Some(&p) = panels.get(t.panel.as_str()) else {
            continue;
        };
        let panel = &ws.panels[p];
        for (j, transition) in t.transitions.iter().enumerate() {
            let element = transition.element();
            let Some(target) = panel
                .elements
                .iter()
                .find(|e| e.id.as_deref() == Some(element))
            else {
                diagnostics.push(Diagnostic::at(
                    format!("/timelines/{i}/transitions/{j}/_element"),
                    format!("panel {:?} has no element with id {element:?}", panel.id),
                ));
                continue;
            };
            if let Transition::SeekFlipbook { frame_index, .. } = transition {
                match target.source() {
                    ElementSource::Sprite(_) => diagnostics.push(Diagnostic::at(
                        format!("/timelines/{i}/transitions/{j}/_element"),
                        format!(
                            "seekFlipbook targets element {element:?}, which is not a flipbook"
                        ),
                    )),
                    ElementSource::Flipbook(f) => {
                        if let Some(&fb) = flipbooks.get(f) {
                            let count = ws.flipbooks[fb].frames.len();
                            if *frame_index >= count {
                                diagnostics.push(Diagnostic::at(
                                    format!("/timelines/{i}/transitions/{j}/frameIndex"),
                                    format!(
                                        "frameIndex {frame_index} is out of range: flipbook {f:?} has {count} frames"
                                    ),
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    // Palettes: unique characters within one `colors` array, then inheritance.
    for (i, p) in ws.palettes.iter().enumerate() {
        let mut seen: HashMap<char, usize> = HashMap::new();
        for (j, color) in p.colors.iter().enumerate() {
            if let Some(first) = seen.insert(color.char, j) {
                seen.insert(color.char, first);
                diagnostics.push(Diagnostic::at(
                    format!("/palettes/{i}/colors/{j}/char"),
                    format!(
                        "character {:?} is already defined at index {first}",
                        color.char
                    ),
                ));
            }
        }
    }
    let resolved_palettes = resolve::resolve_palettes(&ws, &palettes, &mut diagnostics);
    let resolved_sprites =
        resolve::resolve_sprites(&ws, &sprites, &resolved_palettes, &mut diagnostics);

    // Crop bounds along declared transform chains.
    for (i, f) in ws.flipbooks.iter().enumerate() {
        for (j, frame) in f.frames.iter().enumerate() {
            if let Some(s) = resolved_sprites.get(&frame.sprite) {
                check_crops(
                    (s.width(), s.height()),
                    &frame.transforms,
                    &format!("/flipbooks/{i}/frames/{j}/transforms"),
                    &mut diagnostics,
                );
            }
        }
    }
    for (i, p) in ws.panels.iter().enumerate() {
        for (j, el) in p.elements.iter().enumerate() {
            let size = match el.source() {
                ElementSource::Sprite(s) => {
                    resolved_sprites.get(s).map(|s| (s.width(), s.height()))
                }
                ElementSource::Flipbook(f) => flipbooks
                    .get(f)
                    .map(|&f| (ws.flipbooks[f].width, ws.flipbooks[f].height)),
            };
            if let Some(size) = size {
                check_crops(
                    size,
                    &el.transforms,
                    &format!("/panels/{i}/elements/{j}/transforms"),
                    &mut diagnostics,
                );
            }
        }
    }

    // Export pointers.
    for (i, pointer) in ws.exports.iter().flatten().enumerate() {
        let mut parts = pointer.splitn(3, '/').skip(1);
        let (collection, id) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
        let found = match collection {
            "sprites" => sprites.contains_key(id),
            "flipbooks" => flipbooks.contains_key(id),
            "panels" => panels.contains_key(id),
            "timelines" => timelines.contains_key(id),
            "scenes" => scenes.contains_key(id),
            _ => false,
        };
        if !found {
            diagnostics.push(Diagnostic::at(
                format!("/exports/{i}"),
                format!("{pointer:?} does not resolve to an existing resource"),
            ));
        }
    }

    if diagnostics.is_empty() {
        Ok(ResolvedWorksheet {
            model: ws,
            palettes: resolved_palettes,
            sprites: resolved_sprites,
        })
    } else {
        Err(diagnostics)
    }
}

/// Follows the image size along a transform chain and checks every `crop`.
fn check_crops(
    size: (u32, u32),
    transforms: &[Transform],
    pointer: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let (mut w, mut h) = (u64::from(size.0), u64::from(size.1));
    for (t, transform) in transforms.iter().enumerate() {
        match *transform {
            Transform::Flip { .. }
            | Transform::Rotate {
                degrees: Rotation::Deg180,
            } => {}
            Transform::Rotate { .. } => (w, h) = (h, w),
            Transform::Scale { width, height } => (w, h) = (width.into(), height.into()),
            Transform::Crop {
                area: [x, y, cw, ch],
            } => {
                let (x, y, cw, ch) = (u64::from(x), u64::from(y), u64::from(cw), u64::from(ch));
                if x + cw > w || y + ch > h {
                    diagnostics.push(Diagnostic::at(
                        format!("{pointer}/{t}/area"),
                        format!(
                            "crop area [{x}, {y}, {cw}, {ch}] does not fit in the {w}x{h} image"
                        ),
                    ));
                    return;
                }
                (w, h) = (cw, ch);
            }
        }
    }
}
