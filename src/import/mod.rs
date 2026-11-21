// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 The pixel-perfect Authors

//! `pixel-perfect import`: adds a PNG to a worksheet as a palette and a
//! sprite, losslessly and without disturbing the rest of the file.
//!
//! Pipeline: validate the original worksheet; decode and downscale the PNG
//! ([`image`]); build the palette and lines ([`entries`]); insert them
//! ([`crate::edit`]); verify the result four ways; then write it atomically.
//! Nothing touches the disk before every check has passed.

pub mod alphabet;
pub mod entries;
pub mod image;

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use crate::edit::{self, Addition, EditError, Node};
use crate::error::{Diagnostic, LoadError};
use crate::input::{self, Format};
use crate::raster::Image;
use crate::{load_str, render};

/// Default `--max-colors`.
pub const DEFAULT_MAX_COLORS: usize = 256;
/// Bound on what the PNG decoder may allocate: the default of
/// `--max-image-size`.
pub const DEFAULT_MAX_DECODE_BYTES: usize = crate::export::DEFAULT_MAX_IMAGE_SIZE as usize;

/// What to import and how.
#[derive(Debug, Clone)]
pub struct ImportOptions {
    pub worksheet: PathBuf,
    pub png: PathBuf,
    /// Id of the new sprite.
    pub id: String,
    /// Reuse this palette instead of creating one.
    pub palette: Option<String>,
    /// Id of the new palette (default: `id`).
    pub palette_id: Option<String>,
    pub scale: u32,
    pub description: Option<String>,
    /// Append `/sprites/<id>` to an existing `exports` list.
    pub export: bool,
    pub max_colors: usize,
    pub dry_run: bool,
}

impl ImportOptions {
    pub fn new(
        worksheet: impl Into<PathBuf>,
        png: impl Into<PathBuf>,
        id: impl Into<String>,
    ) -> Self {
        ImportOptions {
            worksheet: worksheet.into(),
            png: png.into(),
            id: id.into(),
            palette: None,
            palette_id: None,
            scale: 1,
            description: None,
            export: false,
            max_colors: DEFAULT_MAX_COLORS,
            dry_run: false,
        }
    }
}

/// A successful import.
#[derive(Debug, Clone)]
pub struct Imported {
    /// The new worksheet text (written to disk unless `dry_run`).
    pub text: String,
    /// One line describing what was added.
    pub summary: String,
}

/// Why an import did not happen. The worksheet is unchanged in every case.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    /// Bad options, unreadable file, unsupported extension, write failure.
    #[error("{0}")]
    Usage(String),
    /// The original worksheet is invalid; diagnostics as `validate` reports.
    #[error("{} error(s) in worksheet", diagnostics.len())]
    InvalidWorksheet { diagnostics: Vec<Diagnostic> },
    /// The image or the worksheet cannot be imported as asked.
    #[error("{0}")]
    Rejected(String),
}

/// The editor used to insert additions; replaceable in tests.
pub type Editor = fn(&str, Format, &[Addition]) -> Result<String, EditError>;

/// Runs an import with the standard editors.
pub fn run(options: &ImportOptions) -> Result<Imported, ImportError> {
    run_with(options, edit::add)
}

/// Runs an import with `editor` inserting the additions.
pub fn run_with(options: &ImportOptions, editor: Editor) -> Result<Imported, ImportError> {
    let ws_path = &options.worksheet;
    let ws_name = ws_path.display().to_string();
    let png_name = options.png.display().to_string();
    let usage = |msg: String| ImportError::Usage(msg);
    let rejected = |file: &str, msg: String| ImportError::Rejected(format!("{file}: {msg}"));

    if options.palette.is_some() && options.palette_id.is_some() {
        return Err(usage(
            "--palette and --palette-id cannot be used together".into(),
        ));
    }
    if options.max_colors == 0 || options.max_colors > alphabet::len() {
        return Err(usage(format!(
            "--max-colors must be between 1 and {}",
            alphabet::len()
        )));
    }
    if !(1..=64).contains(&options.scale) {
        return Err(usage("--scale must be between 1 and 64".into()));
    }
    let format = Format::from_path(ws_path).map_err(|ext| {
        usage(format!(
            "{ws_name}: unsupported extension {ext:?} (supported: .json, .yaml, .yml, .toml)"
        ))
    })?;
    let text = fs::read_to_string(ws_path)
        .map_err(|e| usage(format!("{ws_name}: cannot read file: {e}")))?;
    let png =
        fs::read(&options.png).map_err(|e| usage(format!("{png_name}: cannot read file: {e}")))?;

    // The original must be valid, or the checks below would mean nothing.
    let original = load_str(&text, format).map_err(|e| match e {
        LoadError::Invalid { diagnostics } => ImportError::InvalidWorksheet { diagnostics },
        other => usage(format!("{ws_name}: {other}")),
    })?;
    let model = &original.model;

    // Ids.
    let id = options.id.as_str();
    if id.is_empty() {
        return Err(usage("--id must not be empty".into()));
    }
    if let Some(i) = model.sprites.iter().position(|s| s.id == id) {
        return Err(rejected(
            &ws_name,
            format!("/sprites/{i}: sprite {id:?} already exists"),
        ));
    }
    let new_palette_id = match &options.palette {
        Some(_) => None,
        None => Some(options.palette_id.clone().unwrap_or_else(|| id.to_owned())),
    };
    if let Some(pid) = &new_palette_id {
        if pid.is_empty() {
            return Err(usage("--palette-id must not be empty".into()));
        }
        if let Some(i) = model.palettes.iter().position(|p| &p.id == pid) {
            return Err(rejected(
                &ws_name,
                format!(
                    "/palettes/{i}: palette {pid:?} already exists; reuse it with --palette {pid} \
                     or name the new palette with --palette-id"
                ),
            ));
        }
    }
    let export = options.export && model.exports.is_some();
    if export && id.contains('/') {
        return Err(rejected(
            &ws_name,
            format!("sprite id {id:?} contains '/' and cannot be exported"),
        ));
    }

    // Image.
    let decoded =
        image::decode(&png, DEFAULT_MAX_DECODE_BYTES).map_err(|m| rejected(&png_name, m))?;
    let image = image::downscale(&decoded, options.scale).map_err(|m| rejected(&png_name, m))?;

    // Entries.
    let (palette_used, palette_entries, lines) = match &options.palette {
        Some(pid) => {
            let entries = entries::resolved_entries(model, pid).ok_or_else(|| {
                rejected(&ws_name, format!("no palette {pid:?} in the worksheet"))
            })?;
            let lines = entries::reuse_palette(&image, &entries, pid)
                .map_err(|m| rejected(&png_name, m))?;
            (pid.clone(), None, lines)
        }
        None => {
            let pid = new_palette_id.clone().expect("set without --palette");
            let (entries, lines) = entries::new_palette(&image, options.max_colors)
                .map_err(|m| rejected(&png_name, m))?;
            (pid, Some(entries), lines)
        }
    };

    let mut additions = Vec::new();
    if let Some(entries) = &palette_entries {
        additions.push(Addition {
            collection: "palettes".into(),
            items: vec![entries::palette_node(&palette_used, entries)],
        });
    }
    additions.push(Addition {
        collection: "sprites".into(),
        items: vec![entries::sprite_node(
            id,
            options.description.as_deref(),
            &palette_used,
            &lines,
        )],
    });
    if export {
        additions.push(Addition {
            collection: "exports".into(),
            items: vec![Node::Quoted(format!("/sprites/{id}"))],
        });
    }

    // Insert.
    let new_text = editor(&text, format, &additions)
        .map_err(|e| rejected(&ws_name, format!("cannot add to this worksheet: {e}")))?;

    // Checks.
    verify(&text, &new_text, format, &additions, id, &image).map_err(|m| rejected(&ws_name, m))?;

    let colors = palette_entries.as_ref().map_or_else(
        || {
            let mut seen: Vec<_> = image.pixels().to_vec();
            seen.retain(|p| *p != crate::raster::TRANSPARENT);
            seen.sort_by_key(|p| p.0);
            seen.dedup();
            seen.len()
        },
        Vec::len,
    );
    let mut pointers = Vec::new();
    if palette_entries.is_some() {
        pointers.push(format!("/palettes/{palette_used}"));
    }
    pointers.push(format!("/sprites/{id}"));
    let summary = format!(
        "{ws_name}: added {} ({}\u{d7}{}, {colors} color{}){}",
        pointers.join(", "),
        image.width(),
        image.height(),
        if colors == 1 { "" } else { "s" },
        if export { ", exported" } else { "" },
    );

    if !options.dry_run {
        write_atomically(ws_path, &new_text)
            .map_err(|e| usage(format!("{ws_name}: cannot write: {e}")))?;
    }
    Ok(Imported {
        text: new_text,
        summary,
    })
}

/// The four checks on the edited text.
fn verify(
    old: &str,
    new: &str,
    format: Format,
    additions: &[Addition],
    id: &str,
    image: &Image,
) -> Result<(), String> {
    let fail = |n: u32, what: &str, detail: String| {
        Err(format!(
            "check {n} ({what}) failed, the worksheet was not changed: {detail}"
        ))
    };
    // 1. Valid.
    let ws = match load_str(new, format) {
        Ok(ws) => ws,
        Err(e) => {
            let detail = match &e {
                LoadError::Invalid { diagnostics } => diagnostics
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; "),
                other => other.to_string(),
            };
            return fail(1, "the result is a valid worksheet", detail);
        }
    };
    // 2. Same data model plus the additions.
    let mut expected =
        input::parse(old, format).map_err(|_| "original no longer parses".to_owned())?;
    let root = expected
        .as_object_mut()
        .expect("a valid worksheet is an object");
    for add in additions {
        let list = root
            .entry(add.collection.clone())
            .or_insert_with(|| serde_json::Value::Array(Vec::new()));
        let list = list.as_array_mut().expect("collections are arrays");
        list.extend(add.items.iter().map(Node::to_json));
    }
    let actual = input::parse(new, format).map_err(|_| "result does not parse".to_owned())?;
    if actual != expected {
        return fail(
            2,
            "only the new entries were added",
            "the edited worksheet does not equal the original plus the new entries".into(),
        );
    }
    // 3. Pixel-exact rendering.
    if render::sprite(&ws, id) != *image {
        return fail(
            3,
            "the sprite renders exactly as the image",
            "the rendered sprite differs from the image".into(),
        );
    }
    // 4. Insertions only.
    if !edit::is_subsequence(old, new) {
        return fail(
            4,
            "the original text is unchanged",
            "some original text was modified or removed".into(),
        );
    }
    Ok(())
}

/// Replaces `path` with `text` through a temporary file in the same directory
/// and a rename, keeping the original permissions. A symlink is followed, so
/// the link stays and its target is replaced.
fn write_atomically(path: &Path, text: &str) -> std::io::Result<()> {
    let target = fs::canonicalize(path)?;
    let dir = target.parent().unwrap_or(Path::new("."));
    let permissions = fs::metadata(&target)?.permissions();
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    tmp.write_all(text.as_bytes())?;
    tmp.as_file().sync_all()?;
    fs::set_permissions(tmp.path(), permissions)?;
    tmp.persist(&target).map_err(|e| e.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Rgba;

    #[test]
    fn pixel_check_catches_a_rendering_mismatch() {
        let old = "palettes:\n  - id: p\n    colors:\n      - { char: a, rgba: [1, 2, 3] }\n";
        let sprite = entries::sprite_node("s", None, "p", &["a".to_owned()]);
        let additions = vec![Addition {
            collection: "sprites".into(),
            items: vec![sprite],
        }];
        let new = edit::add(old, Format::Yaml, &additions).unwrap();
        let right = Image::new(1, 1, vec![Rgba([1, 2, 3, 255])]);
        assert_eq!(
            verify(old, &new, Format::Yaml, &additions, "s", &right),
            Ok(())
        );
        let wrong = Image::new(1, 1, vec![Rgba([1, 2, 4, 255])]);
        let err = verify(old, &new, Format::Yaml, &additions, "s", &wrong).unwrap_err();
        assert!(err.contains("check 3"), "{err}");
    }
}
