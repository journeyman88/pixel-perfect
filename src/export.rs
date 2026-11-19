//! Export planning, file naming, writing and the manifest.
//!
//! Rendering runs in two phases: [`plan`] decides every output and collects
//! all errors without touching the disk; [`write`] renders and writes only a
//! clean plan, so an invalid request never leaves a half-written output.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use blake2::{Blake2b, Digest, digest::consts::U8};
use serde::Serialize;

use crate::error::Diagnostic;
use crate::raster::Image;
use crate::semantic::ResolvedWorksheet;
use crate::{encode, render};

/// Collections that can be exported, in worksheet order.
const COLLECTIONS: [&str; 5] = ["sprites", "flipbooks", "panels", "timelines", "scenes"];

/// Collections this version can render.
const SUPPORTED: [&str; 1] = ["sprites"];

/// Largest image side a PNG can store.
const MAX_SIDE: u64 = i32::MAX as u64;

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
    match collection {
        "sprites" => {
            let i = ws.model.sprites.iter().position(|s| s.id == id)?;
            let s = ws.sprites.get(id)?;
            Some((i, (s.width(), s.height())))
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
                if SUPPORTED.contains(&collection) {
                    selected.push((collection, id.to_owned()));
                } else {
                    diagnostics.push(Diagnostic::at(
                        format!("/exports/{i}"),
                        format!("cannot export {pointer:?}: {collection} are not supported yet"),
                    ));
                }
            }
        }
        None => {
            for collection in COLLECTIONS {
                let ids = collection_ids(ws, collection);
                if SUPPORTED.contains(&collection) {
                    selected.extend(ids.into_iter().map(|id| (collection, id.to_owned())));
                } else if !ids.is_empty() {
                    notices.push(format!(
                        "skipped {} resource(s) in {collection}: {collection} are not supported yet",
                        ids.len()
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

    if diagnostics.is_empty() {
        Ok(Plan { outputs, notices })
    } else {
        Err(diagnostics)
    }
}

/// Renders one planned output to its final image (after scaling).
pub fn image(ws: &ResolvedWorksheet, output: &PlannedOutput, scale: u32) -> Image {
    let image = match output.collection {
        "sprites" => render::sprite(ws, &output.id),
        other => unreachable!("{other} are not supported yet"),
    };
    image.upscale(scale)
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
