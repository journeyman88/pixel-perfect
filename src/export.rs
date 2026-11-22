// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 The pixel-perfect Authors

//! Export planning, file naming, writing and the manifest.
//!
//! Rendering runs in two phases: [`plan`] decides every output and collects
//! all errors without touching the disk; [`write`] renders and writes only a
//! clean plan, so an invalid request never leaves a half-written output.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use blake2::{Blake2b, Digest, digest::consts::U8};
use serde::Serialize;

use crate::animate::{self, Animation, MAX_FRAMES};
use crate::error::Diagnostic;
use crate::model::{ElementSource, Panel, Transform, Transition};
use crate::raster::{self, Image};
use crate::semantic::ResolvedWorksheet;
use crate::webp::{self, WebpError, WebpFrame};
use crate::{encode, render, time};

/// Collections that can be exported, in worksheet order.
const COLLECTIONS: [&str; 6] = [
    "sprites",
    "flipbooks",
    "panels",
    "timelines",
    "scenes",
    "sequences",
];

/// Largest image side a PNG can store.
const MAX_PNG_SIDE: u64 = i32::MAX as u64;

/// Default of `--max-image-size`: 1 GiB of RGBA8 pixels.
pub const DEFAULT_MAX_IMAGE_SIZE: u64 = 1 << 30;

const UNITS: [(&str, u64); 4] = [
    ("TiB", 1 << 40),
    ("GiB", 1 << 30),
    ("MiB", 1 << 20),
    ("KiB", 1 << 10),
];

/// Parses a `--max-image-size` value: a positive number of bytes, optionally
/// followed by `KiB`, `MiB`, `GiB` or `TiB` (any case), or `none` (no limit).
pub fn parse_size(text: &str) -> Result<Option<u64>, String> {
    if text.eq_ignore_ascii_case("none") {
        return Ok(None);
    }
    let lower = text.to_ascii_lowercase();
    let (digits, factor) = UNITS
        .iter()
        .find_map(|(unit, factor)| {
            lower
                .strip_suffix(&unit.to_ascii_lowercase())
                .map(|d| (d, *factor))
        })
        .unwrap_or((&lower, 1));
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        let decimal = ["kb", "mb", "gb", "tb"].iter().any(|u| {
            lower
                .strip_suffix(u)
                .is_some_and(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()))
        });
        return Err(if decimal {
            format!("{text:?}: use binary units (KiB, MiB, GiB, TiB)")
        } else {
            format!(
                "{text:?}: expected a number of bytes, optionally with KiB, MiB, GiB or TiB, or \"none\""
            )
        });
    }
    let n: u64 = digits
        .parse()
        .map_err(|_| format!("{text:?} is too large"))?;
    if n == 0 {
        return Err("the size must be greater than 0".to_owned());
    }
    n.checked_mul(factor)
        .map(Some)
        .ok_or_else(|| format!("{text:?} is too large"))
}

/// A byte count in the largest binary unit that keeps it at least 1, e.g.
/// `1 GiB` or `1.49 GiB`.
pub fn human_size(bytes: u128) -> String {
    for (unit, factor) in UNITS {
        let factor = u128::from(factor);
        if bytes >= factor {
            return if bytes % factor == 0 {
                format!("{} {unit}", bytes / factor)
            } else {
                format!("{:.2} {unit}", bytes as f64 / factor as f64)
            };
        }
    }
    format!("{bytes} B")
}

/// Output file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Png,
    Webp,
}

impl OutputFormat {
    pub fn extension(self) -> &'static str {
        match self {
            OutputFormat::Png => "png",
            OutputFormat::Webp => "webp",
        }
    }

    /// Name used in messages.
    fn label(self) -> &'static str {
        match self {
            OutputFormat::Png => "PNG",
            OutputFormat::Webp => "WebP",
        }
    }

    /// Largest image side the format can store.
    fn max_side(self) -> u64 {
        match self {
            OutputFormat::Png => MAX_PNG_SIDE,
            OutputFormat::Webp => u64::from(webp::MAX_SIDE),
        }
    }
}

/// Options of a render.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderOptions {
    pub output_path: PathBuf,
    pub format: OutputFormat,
    /// Integer upscale factor, 1 to 64.
    pub scale: u32,
    /// Name files by the hash of their id instead of the id.
    pub safe_build: bool,
    /// Write animated resources as one still image per frame.
    pub single_frames: bool,
    /// Largest image, in bytes of RGBA8, the engine may build; `None` for no
    /// limit. See [`DEFAULT_MAX_IMAGE_SIZE`].
    pub max_image_size: Option<u64>,
}

/// One resource to write: a file, or a directory of frames.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedOutput {
    /// Export pointer, e.g. `/sprites/hero`.
    pub pointer: String,
    pub collection: &'static str,
    pub id: String,
    pub kind: EntryKind,
    /// Path relative to the output path, with `/` separators: the file, or
    /// for a frame collection the directory of its frames (no extension).
    pub path: String,
    /// Size after scaling.
    pub width: u32,
    pub height: u32,
    /// Loop length of an animated resource; `None` for a static image.
    pub loop_seconds: Option<Fraction>,
}

/// An exact number of seconds, reduced to lowest terms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Fraction {
    pub num: u64,
    pub den: u64,
}

/// Everything a render will do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub outputs: Vec<PlannedOutput>,
    /// Informational messages printed on standard error.
    pub notices: Vec<String>,
}

/// The `manifest.json` written next to the outputs, as defined by
/// `schema/manifest.schema.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Manifest {
    pub options: ManifestOptions,
    /// One entry per exported resource, sorted by `id`.
    pub files: Vec<ManifestEntry>,
}

impl Manifest {
    /// The entry of the resource with export pointer `id`.
    pub fn entry(&self, id: &str) -> Option<&ManifestEntry> {
        let i = self
            .files
            .binary_search_by(|e| e.id.as_str().cmp(id))
            .ok()?;
        Some(&self.files[i])
    }
}

/// The command-line options a render was made with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestOptions {
    pub format: &'static str,
    pub scale: u32,
    pub safe_build: bool,
    pub single_frames: bool,
}

/// How a resource is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EntryKind {
    /// A single still image.
    Static,
    /// A single animated image.
    Animation,
    /// One still image per frame (`--single-frames`).
    FrameCollection,
}

/// Fields are serialized in declaration order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestEntry {
    /// Export pointer, e.g. `/sprites/hero`.
    pub id: String,
    pub kind: EntryKind,
    /// The file; `None` for a frame collection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub width: u32,
    pub height: u32,
    /// Frames written, for an animation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame_count: Option<usize>,
    /// The frame files in playback order, for a frame collection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frames: Option<Vec<ManifestFrame>>,
    /// Loop length in seconds, for animated resources.
    #[serde(rename = "loop", skip_serializing_if = "Option::is_none")]
    pub loop_seconds: Option<Fraction>,
}

/// One frame of a frame collection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ManifestFrame {
    pub index: usize,
    pub path: String,
    /// How long the frame is shown, in seconds.
    pub duration: Fraction,
}

impl ManifestEntry {
    /// The entry of `output`. `frame_count` and `frames` are filled in when
    /// the animation is encoded.
    fn new(output: &PlannedOutput) -> Self {
        Self {
            id: output.pointer.clone(),
            kind: output.kind,
            path: (output.kind != EntryKind::FrameCollection).then(|| output.path.clone()),
            width: output.width,
            height: output.height,
            frame_count: None,
            frames: None,
            loop_seconds: output.loop_seconds,
        }
    }
}

/// File stem of frame `index` in a collection of `count` frames: zero-padded
/// to 4 digits, or to the width of the last index when that is more, so names
/// sort in playback order.
pub fn frame_name(index: usize, count: usize) -> String {
    let width = count.saturating_sub(1).to_string().len().max(4);
    format!("{index:0width$}")
}

/// Why a render failed.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    /// The export request is invalid; nothing was written.
    #[error("{} error(s) in export plan", .0.len())]
    Plan(Vec<Diagnostic>),
    /// Encoding an output failed after the plan accepted it.
    #[error("{pointer}: {source}")]
    Encode {
        pointer: String,
        #[source]
        source: WebpError,
    },
    /// Writing failed.
    #[error("{}: cannot write: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// BLAKE2b with an 8-byte digest (`b2sum -l 64`) of the id, as 16 hex digits.
pub fn id_hash(id: &str) -> String {
    Blake2b::<U8>::digest(id.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Why `id` cannot be used as a portable file name, if it cannot.
pub fn file_name_problem(id: &str) -> Option<&'static str> {
    if id == "." || id == ".." {
        return Some("is a relative directory name");
    }
    if let Some(c) = id.chars().find(|c| {
        matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control()
    }) {
        return Some(if c.is_control() {
            "contains a control character"
        } else {
            "contains a character not allowed in file names"
        });
    }
    if id.ends_with('.') || id.ends_with(' ') {
        return Some("ends with a dot or a space");
    }
    None
}

/// Index of the resource `id` in `collection`, and its size before scaling.
fn locate(ws: &ResolvedWorksheet, collection: &str, id: &str) -> Option<(usize, (u32, u32))> {
    let m = &ws.model;
    match collection {
        "sprites" => {
            let i = m.sprites.iter().position(|s| s.id == id)?;
            let s = ws.sprites.get(id)?;
            Some((i, (s.width(), s.height())))
        }
        "flipbooks" => {
            let i = m.flipbooks.iter().position(|f| f.id == id)?;
            Some((i, (m.flipbooks[i].width, m.flipbooks[i].height)))
        }
        "panels" => {
            let i = m.panels.iter().position(|p| p.id == id)?;
            Some((i, (m.panels[i].width, m.panels[i].height)))
        }
        "timelines" => {
            let i = m.timelines.iter().position(|t| t.id == id)?;
            let panel = m.panels.iter().find(|p| p.id == m.timelines[i].panel)?;
            Some((i, (panel.width, panel.height)))
        }
        "scenes" => {
            let i = m.scenes.iter().position(|s| s.id == id)?;
            Some((i, (m.scenes[i].width, m.scenes[i].height)))
        }
        "sequences" => {
            let i = m.sequences.iter().position(|q| q.id == id)?;
            let scene = m.scenes.iter().find(|s| s.id == m.sequences[i].scene)?;
            Some((i, (scene.width, scene.height)))
        }
        _ => None,
    }
}

fn collection_ids<'a>(ws: &'a ResolvedWorksheet, collection: &str) -> Vec<&'a str> {
    let m = &ws.model;
    match collection {
        "sprites" => m.sprites.iter().map(|s| s.id.as_str()).collect(),
        "flipbooks" => m.flipbooks.iter().map(|s| s.id.as_str()).collect(),
        "panels" => m.panels.iter().map(|s| s.id.as_str()).collect(),
        "timelines" => m.timelines.iter().map(|s| s.id.as_str()).collect(),
        "scenes" => m.scenes.iter().map(|s| s.id.as_str()).collect(),
        "sequences" => m.sequences.iter().map(|s| s.id.as_str()).collect(),
        _ => Vec::new(),
    }
}

/// Checks the size of every image built to render a resource against the
/// `--max-image-size` limit, reporting each offending pointer once.
struct SizeCheck {
    limit: u64,
    diagnostics: Vec<Diagnostic>,
    reported: HashSet<String>,
}

impl SizeCheck {
    fn check(&mut self, pointer: String, what: &str, (w, h): (u64, u64)) {
        let bytes = u128::from(w) * u128::from(h) * 4;
        if bytes <= u128::from(self.limit) || !self.reported.insert(pointer.clone()) {
            return;
        }
        self.diagnostics.push(Diagnostic::at(
            pointer,
            format!(
                "{what} is {w}x{h} ({}), over the {} image size limit; raise it with --max-image-size",
                human_size(bytes),
                human_size(u128::from(self.limit)),
            ),
        ));
    }

    fn sprite(&mut self, ws: &ResolvedWorksheet, id: &str, pointer: String) -> (u32, u32) {
        let s = &ws.sprites[id];
        let size = (s.width(), s.height());
        self.check(pointer, &format!("sprite {id:?}"), wide(size));
        size
    }

    /// Follows a transform chain from `size`, checking each step.
    fn chain(&mut self, mut size: (u32, u32), transforms: &[Transform], base: &str) -> (u32, u32) {
        for (t, transform) in transforms.iter().enumerate() {
            size = raster::output_size(size, transform);
            self.check(
                format!("{base}/{t}"),
                "the result of this transform",
                wide(size),
            );
        }
        size
    }

    /// Every frame of a flipbook (sprite and transforms) and its canvas.
    fn flipbook(&mut self, ws: &ResolvedWorksheet, id: &str) -> (u32, u32) {
        let Some(index) = ws.model.flipbooks.iter().position(|f| f.id == id) else {
            return (0, 0);
        };
        let fb = &ws.model.flipbooks[index];
        for (k, frame) in fb.frames.iter().enumerate() {
            let base = format!("/flipbooks/{index}/frames/{k}");
            let size = self.sprite(ws, &frame.sprite, base.clone());
            self.chain(size, &frame.transforms, &format!("{base}/transforms"));
        }
        let size = (fb.width, fb.height);
        self.check(
            format!("/flipbooks/{index}"),
            &format!("flipbook {id:?}"),
            wide(size),
        );
        size
    }

    /// The source of a panel element and its own transforms; returns the size
    /// at the end of the chain.
    fn element(&mut self, ws: &ResolvedWorksheet, index: usize, e: usize) -> (u32, u32) {
        let element = &ws.model.panels[index].elements[e];
        let base = format!("/panels/{index}/elements/{e}");
        let size = match element.source() {
            ElementSource::Sprite(sprite) => self.sprite(ws, sprite, base.clone()),
            ElementSource::Flipbook(f) => self.flipbook(ws, f),
        };
        self.chain(size, &element.transforms, &format!("{base}/transforms"))
    }

    /// A panel as drawn on its own: visible elements only.
    fn panel(&mut self, ws: &ResolvedWorksheet, index: usize) {
        let panel = &ws.model.panels[index];
        for (e, element) in panel.elements.iter().enumerate() {
            if element.visible {
                self.element(ws, index, e);
            }
        }
        self.panel_canvas(index, panel);
    }

    fn panel_canvas(&mut self, index: usize, panel: &Panel) {
        self.check(
            format!("/panels/{index}"),
            &format!("panel {:?}", panel.id),
            wide((panel.width, panel.height)),
        );
    }

    /// A timeline: every element of its panel (any may become visible), and
    /// every chain extended by `appendTransforms`.
    fn timeline(&mut self, ws: &ResolvedWorksheet, index: usize) {
        let timeline = &ws.model.timelines[index];
        let Some(p) = ws.model.panels.iter().position(|p| p.id == timeline.panel) else {
            return;
        };
        let panel = &ws.model.panels[p];
        for (e, element) in panel.elements.iter().enumerate() {
            let own = self.element(ws, p, e);
            let Some(id) = &element.id else { continue };
            let mut size = own;
            for j in timeline.instant_order() {
                match &timeline.transitions[j] {
                    Transition::AppendTransforms {
                        element,
                        transforms,
                        ..
                    } if element == id => {
                        size = self.chain(
                            size,
                            transforms,
                            &format!("/timelines/{index}/transitions/{j}/transforms"),
                        );
                    }
                    Transition::ResetTransforms { element, .. } if element == id => size = own,
                    _ => {}
                }
            }
        }
        self.panel_canvas(p, panel);
    }

    fn scene(&mut self, ws: &ResolvedWorksheet, index: usize) {
        let scene = &ws.model.scenes[index];
        for layer in &scene.layers {
            if let Some(id) = &layer.panel
                && let Some(p) = ws.model.panels.iter().position(|p| &p.id == id)
            {
                self.panel(ws, p);
            }
            if let Some(id) = &layer.timeline
                && let Some(t) = ws.model.timelines.iter().position(|t| &t.id == id)
            {
                self.timeline(ws, t);
            }
        }
        self.check(
            format!("/scenes/{index}"),
            &format!("scene {:?}", scene.id),
            wide((scene.width, scene.height)),
        );
    }
}

fn wide((w, h): (u32, u32)) -> (u64, u64) {
    (w.into(), h.into())
}

/// Checks the time model of an animated export without rendering it: the loop
/// fits, the frames fit the size limit and the format's frame count, and, for
/// APNG, every segment can be written as an exact delay. Returns the loop
/// length.
fn plan_animation(
    ws: &ResolvedWorksheet,
    collection: &str,
    id: &str,
    (w, h): (u64, u64),
    options: &RenderOptions,
) -> Result<Fraction, String> {
    let animation = Animation::new(ws, collection, id)
        .map_err(|_| "the clock or the loop of this animation is too long to compute".to_owned())?;
    let frame_bytes = (w * h * 4).max(1);
    let by_size = options.max_image_size.map_or(usize::MAX, |max| {
        usize::try_from(max / frame_bytes).unwrap_or(usize::MAX)
    });
    // A single frame over the limit is reported by the size walk.
    let limit = by_size.clamp(1, MAX_FRAMES);
    let ticks = animation.change_ticks(limit).map_err(|_| {
        if by_size < MAX_FRAMES {
            format!(
                "the animation needs more than {limit} frames of {w}x{h} (over {}), more than the {} image size limit; raise it with --max-image-size",
                human_size(u128::from(frame_bytes) * (limit as u128 + 1)),
                human_size(u128::from(options.max_image_size.unwrap_or(0))),
            )
        } else {
            format!(
                "the animation needs more than {MAX_FRAMES} frames, the {} maximum",
                options.format.label()
            )
        }
    })?;
    // With --single-frames no animated container is written, so its timing
    // limits do not apply.
    if options.single_frames {
    } else if options.format == OutputFormat::Webp {
        let bound = animation.ms_frame_bound(&ticks, webp::MAX_DURATION_MS);
        if bound > MAX_FRAMES as u128 {
            return Err(format!(
                "the animation may need up to {bound} WebP frames, more than the {MAX_FRAMES} maximum (frames longer than {} ms are split)",
                webp::MAX_DURATION_MS
            ));
        }
    } else if let Some((tick, len)) = animation.delay_overflow(&ticks) {
        let (num, den) = time::reduced(len, animation.clock);
        let rates: Vec<String> = animation.rates.iter().map(u32::to_string).collect();
        return Err(format!(
            "a frame at {}/{} s lasts {num}/{den} s, which APNG cannot store exactly (denominator over 65535); it comes from combining the rates {} fps",
            time::reduced(tick, animation.clock).0,
            time::reduced(tick, animation.clock).1,
            rates.join(", ")
        ));
    }
    let (num, den) = time::reduced(animation.loop_len, animation.clock);
    Ok(Fraction { num, den })
}

/// A length in seconds as `n` or `n/d`.
fn seconds_text(f: &Fraction) -> String {
    if f.den == 1 {
        f.num.to_string()
    } else {
        format!("{}/{}", f.num, f.den)
    }
}

/// Decides every output of a render, or returns every problem found.
pub fn plan(ws: &ResolvedWorksheet, options: &RenderOptions) -> Result<Plan, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut notices = Vec::new();

    // Selection: (collection, id). Every collection is exportable.
    let mut selected: Vec<(&'static str, String)> = Vec::new();
    match &ws.model.exports {
        Some(exports) => {
            for pointer in exports {
                let (collection, id) = pointer[1..].split_once('/').unwrap_or_default();
                let Some(&collection) = COLLECTIONS.iter().find(|&&c| c == collection) else {
                    continue; // unreachable after validation
                };
                selected.push((collection, id.to_owned()));
            }
        }
        None => {
            for collection in COLLECTIONS {
                for id in collection_ids(ws, collection) {
                    selected.push((collection, id.to_owned()));
                }
            }
        }
    }

    // Names.
    let ext = options.format.extension();
    let mut taken: HashMap<(&str, String), String> = HashMap::new();
    let mut outputs = Vec::new();
    for (collection, id) in selected {
        let Some((index, (w, h))) = locate(ws, collection, &id) else {
            continue; // unreachable after validation
        };
        let id_pointer = format!("/{collection}/{index}/id");
        let (name, key) = if options.safe_build {
            let hash = id_hash(&id);
            (hash.clone(), hash)
        } else {
            if let Some(problem) = file_name_problem(&id) {
                diagnostics.push(Diagnostic::at(
                    id_pointer,
                    format!("id {id:?} {problem}; use --safe-build to name files by hash"),
                ));
                continue;
            }
            (id.clone(), id.to_lowercase())
        };
        if let Some(other) = taken.get(&(collection, key.clone())) {
            let reason = if options.safe_build {
                "has the same hash as"
            } else {
                "differs only by letter case from"
            };
            diagnostics.push(Diagnostic::at(
                id_pointer,
                format!(
                    "id {id:?} {reason} {other:?}; their files would collide{}",
                    if options.safe_build {
                        ""
                    } else {
                        " (use --safe-build)"
                    }
                ),
            ));
            continue;
        }
        taken.insert((collection, key), id.clone());

        let (sw, sh) = (
            u64::from(w) * u64::from(options.scale),
            u64::from(h) * u64::from(options.scale),
        );
        let max_side = options.format.max_side();
        if sw > max_side || sh > max_side {
            let hint = match options.format {
                OutputFormat::Png => String::new(),
                OutputFormat::Webp => {
                    format!(" (at most {max_side} pixels per side); PNG has no such limit")
                }
            };
            diagnostics.push(Diagnostic::at(
                id_pointer,
                format!(
                    "scaled image {sw}x{sh} is too large for {}{hint}",
                    options.format.label()
                ),
            ));
            continue;
        }
        let loop_seconds = if animate::is_animated(ws, collection, &id) {
            let resource = format!("/{collection}/{index}");
            match plan_animation(ws, collection, &id, (sw, sh), options) {
                Ok(seconds) => {
                    if collection == "sequences" {
                        let q = &ws.model.sequences[index];
                        let (num, den) = time::reduced(q.duration, u64::from(q.fps));
                        let own = Fraction { num, den };
                        if own != seconds {
                            notices.push(format!(
                                "/sequences/{id} loops every {} s (its own duration is {} s) because of its animated layers",
                                seconds_text(&seconds),
                                seconds_text(&own)
                            ));
                        }
                    }
                    Some(seconds)
                }
                Err(message) => {
                    diagnostics.push(Diagnostic::at(resource, message));
                    continue;
                }
            }
        } else {
            None
        };
        let kind = match (loop_seconds.is_some(), options.single_frames) {
            (false, _) => EntryKind::Static,
            (true, false) => EntryKind::Animation,
            (true, true) => EntryKind::FrameCollection,
        };
        outputs.push(PlannedOutput {
            pointer: format!("/{collection}/{id}"),
            collection,
            kind,
            path: match kind {
                EntryKind::FrameCollection => format!("{collection}/{name}"),
                _ => format!("{collection}/{name}.{ext}"),
            },
            id,
            width: sw as u32,
            height: sh as u32,
            loop_seconds,
        });
    }

    if let Some(limit) = options.max_image_size {
        let mut sizes = SizeCheck {
            limit,
            diagnostics: Vec::new(),
            reported: HashSet::new(),
        };
        for output in &outputs {
            let Some((index, (w, h))) = locate(ws, output.collection, &output.id) else {
                continue;
            };
            match output.collection {
                "sprites" => {
                    sizes.sprite(ws, &output.id, format!("/sprites/{index}"));
                }
                "flipbooks" => {
                    sizes.flipbook(ws, &output.id);
                }
                "panels" => sizes.panel(ws, index),
                "timelines" => sizes.timeline(ws, index),
                "scenes" => sizes.scene(ws, index),
                "sequences" => {
                    let scene = &ws.model.sequences[index].scene;
                    if let Some(k) = ws.model.scenes.iter().position(|s| &s.id == scene) {
                        sizes.scene(ws, k);
                    }
                }
                _ => {}
            }
            if options.scale > 1 {
                let scale = u64::from(options.scale);
                sizes.check(
                    format!("/{}/{index}", output.collection),
                    &format!("the image after --scale {}", options.scale),
                    (u64::from(w) * scale, u64::from(h) * scale),
                );
            }
        }
        diagnostics.extend(sizes.diagnostics);
    }

    if diagnostics.is_empty() {
        Ok(Plan { outputs, notices })
    } else {
        Err(diagnostics)
    }
}

/// Renders one planned output to its final image (after scaling).
pub fn image(ws: &ResolvedWorksheet, output: &PlannedOutput, scale: u32) -> Image {
    render::resource(ws, output.collection, &output.id).upscale(scale)
}

/// Renders an animated output and encodes it in `format`; returns the bytes
/// and the number of frames written.
pub fn animation_bytes(
    ws: &ResolvedWorksheet,
    output: &PlannedOutput,
    scale: u32,
    format: OutputFormat,
) -> Result<(Vec<u8>, usize), WebpError> {
    let animation = Animation::new(ws, output.collection, &output.id)
        .expect("the export plan checked the time model");
    if format == OutputFormat::Webp {
        let frames: Vec<(Image, u128)> = animation
            .ms_frames()
            .into_iter()
            .map(|(image, ms)| (image.upscale(scale), ms))
            .collect();
        let webp_frames: Vec<WebpFrame> = frames
            .iter()
            .map(|(image, duration_ms)| WebpFrame {
                image,
                duration_ms: *duration_ms,
            })
            .collect();
        return webp::animated(&webp_frames);
    }
    let frames: Vec<(Image, Vec<(u16, u16)>)> = animation
        .frames()
        .into_iter()
        .map(|(image, ticks)| {
            let delays =
                time::delays(ticks, animation.clock).expect("the export plan checked every delay");
            (image.upscale(scale), delays)
        })
        .collect();
    let apng_frames: Vec<encode::ApngFrame> = frames
        .iter()
        .flat_map(|(image, delays)| {
            delays
                .iter()
                .map(move |&delay| encode::ApngFrame { image, delay })
        })
        .collect();
    Ok((encode::apng(&apng_frames), apng_frames.len()))
}

/// Builds the manifest of a plan. `frames` is filled in by [`write`].
pub fn manifest(plan: &Plan, options: &RenderOptions) -> Manifest {
    let mut files: Vec<ManifestEntry> = plan.outputs.iter().map(ManifestEntry::new).collect();
    files.sort_by(|a, b| a.id.cmp(&b.id));
    Manifest {
        options: ManifestOptions {
            format: options.format.extension(),
            scale: options.scale,
            safe_build: options.safe_build,
            single_frames: options.single_frames,
        },
        files,
    }
}

/// Renders a frame collection and writes one still image per frame under
/// its directory; returns the manifest frames in playback order.
fn write_frames(
    ws: &ResolvedWorksheet,
    output: &PlannedOutput,
    options: &RenderOptions,
    counts: &mut WriteCounts,
) -> Result<Vec<ManifestFrame>, RenderError> {
    let animation = Animation::new(ws, output.collection, &output.id)
        .expect("the export plan checked the time model");
    let frames = animation.exact_frames();
    // One color space for the whole collection: PNG frames share the encoding
    // (and palette) chosen from all frames together, as APNG does. Upscaling
    // adds no colors, so the unscaled frames are enough.
    let shared = encode::choose_all(frames.iter().map(|(image, _)| image));
    let ext = options.format.extension();
    let mut out = Vec::with_capacity(frames.len());
    for (index, (image, ticks)) in frames.iter().enumerate() {
        let image = image.upscale(options.scale);
        let bytes = match options.format {
            OutputFormat::Png => encode::png_with(&image, shared.clone()),
            OutputFormat::Webp => webp::still(&image),
        };
        let path = format!("{}/{}.{ext}", output.path, frame_name(index, frames.len()));
        counts.add(write_file(&options.output_path.join(&path), &bytes)?);
        let (num, den) = time::reduced(*ticks, animation.clock);
        out.push(ManifestFrame {
            index,
            path,
            duration: Fraction { num, den },
        });
    }
    Ok(out)
}

/// What [`write_file`] did with a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Written {
    /// The file was created or its content replaced.
    Changed,
    /// The file already had exactly these bytes and was left untouched.
    Unchanged,
}

/// Files written and left unchanged by a render.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct WriteCounts {
    written: usize,
    unchanged: usize,
}

impl WriteCounts {
    fn add(&mut self, outcome: Written) {
        match outcome {
            Written::Changed => self.written += 1,
            Written::Unchanged => self.unchanged += 1,
        }
    }
}

/// Writes `bytes` to `path` in place, creating its directories, unless the
/// file already has exactly these bytes: then it is left untouched, keeping
/// its modification time, so tools that reload changed files see only real
/// changes.
fn write_file(path: &Path, bytes: &[u8]) -> Result<Written, RenderError> {
    let io = |source| RenderError::Io {
        path: path.to_owned(),
        source,
    };
    let same_length = std::fs::metadata(path).is_ok_and(|m| m.len() == bytes.len() as u64);
    if same_length && std::fs::read(path).is_ok_and(|old| old == bytes) {
        return Ok(Written::Unchanged);
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(io)?;
    }
    std::fs::write(path, bytes).map_err(io)?;
    Ok(Written::Changed)
}

/// Writes every output of `plan`, then `manifest.json`.
pub fn write(
    ws: &ResolvedWorksheet,
    plan: &Plan,
    options: &RenderOptions,
) -> Result<Manifest, RenderError> {
    write_counted(ws, plan, options).map(|(manifest, _)| manifest)
}

fn write_counted(
    ws: &ResolvedWorksheet,
    plan: &Plan,
    options: &RenderOptions,
) -> Result<(Manifest, WriteCounts), RenderError> {
    let mut counts = WriteCounts::default();
    let mut manifest = manifest(plan, options);
    for output in &plan.outputs {
        let entry = manifest
            .files
            .binary_search_by(|e| e.id.as_str().cmp(&output.pointer))
            .expect("every output has a manifest entry");
        let bytes = match output.kind {
            EntryKind::FrameCollection => {
                manifest.files[entry].frames =
                    Some(write_frames(ws, output, options, &mut counts)?);
                continue;
            }
            EntryKind::Animation => {
                let (bytes, frames) = animation_bytes(ws, output, options.scale, options.format)
                    .map_err(|source| RenderError::Encode {
                        pointer: output.pointer.clone(),
                        source,
                    })?;
                manifest.files[entry].frame_count = Some(frames);
                bytes
            }
            EntryKind::Static => match options.format {
                OutputFormat::Png => encode::png(&image(ws, output, options.scale)),
                OutputFormat::Webp => webp::still(&image(ws, output, options.scale)),
            },
        };
        counts.add(write_file(&options.output_path.join(&output.path), &bytes)?);
    }
    let mut json = serde_json::to_string_pretty(&manifest).expect("manifest serializes");
    json.push('\n');
    counts.add(write_file(
        &options.output_path.join("manifest.json"),
        json.as_bytes(),
    )?);
    Ok((manifest, counts))
}

/// The result of a render, with how many files it wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderReport {
    pub plan: Plan,
    pub manifest: Manifest,
    /// Files created or whose content changed, `manifest.json` included.
    pub written: usize,
    /// Files that already had exactly the new content and were left untouched.
    pub unchanged: usize,
}

/// Plans and writes a render.
pub fn render(
    ws: &ResolvedWorksheet,
    options: &RenderOptions,
) -> Result<(Plan, Manifest), RenderError> {
    render_report(ws, options).map(|r| (r.plan, r.manifest))
}

/// Plans and writes a render, reporting how many files were written.
pub fn render_report(
    ws: &ResolvedWorksheet,
    options: &RenderOptions,
) -> Result<RenderReport, RenderError> {
    let plan = plan(ws, options).map_err(RenderError::Plan)?;
    let (manifest, counts) = write_counted(ws, &plan, options)?;
    Ok(RenderReport {
        plan,
        manifest,
        written: counts.written,
        unchanged: counts.unchanged,
    })
}
