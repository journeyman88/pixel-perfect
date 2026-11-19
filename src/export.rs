//! Export planning, file naming, writing and the manifest.
//!
//! Rendering runs in two phases: [`plan`] decides every output and collects
//! all errors without touching the disk; [`write`] renders and writes only a
//! clean plan, so an invalid request never leaves a half-written output.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use blake2::{Blake2b, Digest, digest::consts::U8};
use serde::Serialize;

use crate::error::Diagnostic;
use crate::model::ElementSource;
use crate::raster::{self, Image};
use crate::semantic::ResolvedWorksheet;
use crate::{compose, encode, render};

/// Collections that can be exported, in worksheet order.
const COLLECTIONS: [&str; 5] = ["sprites", "flipbooks", "panels", "timelines", "scenes"];

/// Largest image side a PNG can store.
const MAX_SIDE: u64 = i32::MAX as u64;

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
}

impl OutputFormat {
    pub fn extension(self) -> &'static str {
        match self {
            OutputFormat::Png => "png",
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
    /// Largest image, in bytes of RGBA8, the engine may build; `None` for no
    /// limit. See [`DEFAULT_MAX_IMAGE_SIZE`].
    pub max_image_size: Option<u64>,
}

/// One file to produce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedOutput {
    /// Export pointer, e.g. `/sprites/hero`.
    pub pointer: String,
    pub collection: &'static str,
    pub id: String,
    /// Path relative to the output path, with `/` separators.
    pub path: String,
    /// Size after scaling.
    pub width: u32,
    pub height: u32,
}

/// Everything a render will do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub outputs: Vec<PlannedOutput>,
    /// Informational messages, e.g. resources skipped because unsupported.
    pub notices: Vec<String>,
}

/// The `manifest.json` written next to the outputs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub format: &'static str,
    pub scale: u32,
    pub safe_build: bool,
    pub files: BTreeMap<String, ManifestEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ManifestEntry {
    pub path: String,
    pub width: u32,
    pub height: u32,
}

/// Why a render failed.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    /// The export request is invalid; nothing was written.
    #[error("{} error(s) in export plan", .0.len())]
    Plan(Vec<Diagnostic>),
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
        "panels" => {
            let i = m.panels.iter().position(|p| p.id == id)?;
            Some((i, (m.panels[i].width, m.panels[i].height)))
        }
        "scenes" => {
            let i = m.scenes.iter().position(|s| s.id == id)?;
            Some((i, (m.scenes[i].width, m.scenes[i].height)))
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
        _ => Vec::new(),
    }
}

/// Why the resource `id` of `collection` cannot be rendered by this version,
/// if it cannot.
fn unsupported(ws: &ResolvedWorksheet, collection: &str, id: &str) -> Option<String> {
    let animated = match collection {
        "sprites" => false,
        "panels" => compose::find_panel(ws, id).is_some_and(compose::is_animated_panel),
        "scenes" => compose::find_scene(ws, id).is_some_and(|s| compose::is_animated_scene(ws, s)),
        _ => return Some(format!("{collection} are not supported yet")),
    };
    animated.then(|| format!("animated {collection} are not supported yet"))
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

    fn panel(&mut self, ws: &ResolvedWorksheet, index: usize) {
        let panel = &ws.model.panels[index];
        for (e, element) in panel.elements.iter().enumerate() {
            let ElementSource::Sprite(sprite) = element.source() else {
                continue; // hidden flipbooks are not drawn
            };
            if !element.visible {
                continue;
            }
            let base = format!("/panels/{index}/elements/{e}");
            let mut size = self.sprite(ws, sprite, base.clone());
            for (t, transform) in element.transforms.iter().enumerate() {
                size = raster::output_size(size, transform);
                self.check(
                    format!("{base}/transforms/{t}"),
                    "the result of this transform",
                    wide(size),
                );
            }
        }
        self.check(
            format!("/panels/{index}"),
            &format!("panel {:?}", panel.id),
            wide((panel.width, panel.height)),
        );
    }

    fn scene(&mut self, ws: &ResolvedWorksheet, index: usize) {
        let scene = &ws.model.scenes[index];
        for layer in &scene.layers {
            if let Some(id) = &layer.panel
                && let Some(p) = ws.model.panels.iter().position(|p| &p.id == id)
            {
                self.panel(ws, p);
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

/// Decides every output of a render, or returns every problem found.
pub fn plan(ws: &ResolvedWorksheet, options: &RenderOptions) -> Result<Plan, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut notices = Vec::new();

    // Selection: (collection, id).
    let mut selected: Vec<(&'static str, String)> = Vec::new();
    match &ws.model.exports {
        Some(exports) => {
            for (i, pointer) in exports.iter().enumerate() {
                let (collection, id) = pointer[1..].split_once('/').unwrap_or_default();
                let Some(&collection) = COLLECTIONS.iter().find(|&&c| c == collection) else {
                    continue; // unreachable after validation
                };
                match unsupported(ws, collection, id) {
                    None => selected.push((collection, id.to_owned())),
                    Some(reason) => diagnostics.push(Diagnostic::at(
                        format!("/exports/{i}"),
                        format!("cannot export {pointer:?}: {reason}"),
                    )),
                }
            }
        }
        None => {
            for collection in COLLECTIONS {
                let mut skipped = Vec::new();
                for id in collection_ids(ws, collection) {
                    match unsupported(ws, collection, id) {
                        None => selected.push((collection, id.to_owned())),
                        Some(reason) => skipped.push(reason),
                    }
                }
                if let Some(reason) = skipped.first() {
                    notices.push(format!(
                        "skipped {} resource(s) in {collection}: {reason}",
                        skipped.len()
                    ));
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
        if sw > MAX_SIDE || sh > MAX_SIDE {
            diagnostics.push(Diagnostic::at(
                id_pointer,
                format!("scaled image {sw}x{sh} is too large for PNG"),
            ));
            continue;
        }
        outputs.push(PlannedOutput {
            pointer: format!("/{collection}/{id}"),
            collection,
            path: format!("{collection}/{name}.{ext}"),
            id,
            width: sw as u32,
            height: sh as u32,
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
                "panels" => sizes.panel(ws, index),
                "scenes" => sizes.scene(ws, index),
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

/// Builds the manifest of a plan.
pub fn manifest(plan: &Plan, options: &RenderOptions) -> Manifest {
    Manifest {
        format: options.format.extension(),
        scale: options.scale,
        safe_build: options.safe_build,
        files: plan
            .outputs
            .iter()
            .map(|o| {
                (
                    o.pointer.clone(),
                    ManifestEntry {
                        path: o.path.clone(),
                        width: o.width,
                        height: o.height,
                    },
                )
            })
            .collect(),
    }
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<(), RenderError> {
    let io = |source| RenderError::Io {
        path: path.to_owned(),
        source,
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(io)?;
    }
    std::fs::write(path, bytes).map_err(io)
}

/// Writes every output of `plan`, then `manifest.json`.
pub fn write(
    ws: &ResolvedWorksheet,
    plan: &Plan,
    options: &RenderOptions,
) -> Result<Manifest, RenderError> {
    for output in &plan.outputs {
        let image = image(ws, output, options.scale);
        let bytes = match options.format {
            OutputFormat::Png => encode::png(&image),
        };
        write_file(&options.output_path.join(&output.path), &bytes)?;
    }
    let manifest = manifest(plan, options);
    let mut json = serde_json::to_string_pretty(&manifest).expect("manifest serializes");
    json.push('\n');
    write_file(&options.output_path.join("manifest.json"), json.as_bytes())?;
    Ok(manifest)
}

/// Plans and writes a render.
pub fn render(
    ws: &ResolvedWorksheet,
    options: &RenderOptions,
) -> Result<(Plan, Manifest), RenderError> {
    let plan = plan(ws, options).map_err(RenderError::Plan)?;
    let manifest = write(ws, &plan, options)?;
    Ok((plan, manifest))
}
