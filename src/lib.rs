// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 The pixel-perfect Authors

//! pixel-perfect: renders lossless pixel art from declarative worksheets.
//!
//! Loading pipeline: [`input`] parses JSON/YAML/TOML into a JSON value,
//! [`schema`] validates it against the embedded schema, [`model`] gives it
//! types, and [`semantic`] enforces the remaining rules and resolves
//! inheritance (via [`resolve`]).
//!
//! Output pipeline: [`render`] rasterizes resources into [`raster::Image`]s
//! (composing panels and scenes with [`compose`]),
//! [`encode`] turns final images into PNG, and [`export`] plans file names,
//! writes the files and the manifest.

pub mod animate;
pub mod compose;
pub mod delta;
pub mod encode;
pub mod error;
pub mod export;
pub mod input;
pub mod model;
pub mod raster;
pub mod render;
pub mod resolve;
pub mod schema;
pub mod semantic;
pub mod time;
pub mod webp;

use std::path::Path;

pub use error::{Diagnostic, LoadError, Location};
pub use input::Format;
pub use semantic::ResolvedWorksheet;

/// Loads, validates and resolves the worksheet at `path`.
pub fn load_file(path: &Path) -> Result<ResolvedWorksheet, LoadError> {
    let format = Format::from_path(path).map_err(|ext| LoadError::UnsupportedExtension {
        path: path.to_owned(),
        ext,
    })?;
    let text = std::fs::read_to_string(path).map_err(|source| LoadError::Io {
        path: path.to_owned(),
        source,
    })?;
    load_str(&text, format)
}

/// Loads, validates and resolves a worksheet from text.
pub fn load_str(text: &str, format: Format) -> Result<ResolvedWorksheet, LoadError> {
    let value = input::parse(text, format)?;
    load_value(value)
}

/// Validates and resolves a worksheet already in the JSON data model.
pub fn load_value(value: serde_json::Value) -> Result<ResolvedWorksheet, LoadError> {
    let schema_errors = schema::validate(&value);
    if !schema_errors.is_empty() {
        return Err(schema_errors.into());
    }
    let model: model::Worksheet = serde_json::from_value(value).map_err(|e| {
        vec![Diagnostic::new(
            Location::Unknown,
            format!("schema-valid worksheet does not match the model: {e}"),
        )]
    })?;
    Ok(semantic::check(model)?)
}
