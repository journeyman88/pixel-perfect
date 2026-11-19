//! Diagnostics reported while loading a worksheet.

use std::fmt;
use std::path::PathBuf;

/// Where a diagnostic points inside the worksheet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Location {
    /// A JSON Pointer into the worksheet data model (`""` is the root).
    Pointer(String),
    /// A 1-based line and column in the source text (syntax errors).
    LineColumn { line: usize, column: usize },
    /// No precise location is available.
    Unknown,
}

impl Location {
    /// Builds a pointer location from already-escaped segments.
    pub fn pointer(path: impl Into<String>) -> Self {
        Location::Pointer(path.into())
    }
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Location::Pointer(p) if p.is_empty() => f.write_str("(root)"),
            Location::Pointer(p) => f.write_str(p),
            Location::LineColumn { line, column } => write!(f, "{line}:{column}"),
            Location::Unknown => f.write_str("?"),
        }
    }
}

/// One problem found in a worksheet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub location: Location,
    pub message: String,
}

impl Diagnostic {
    pub fn new(location: Location, message: impl Into<String>) -> Self {
        Diagnostic {
            location,
            message: message.into(),
        }
    }

    pub fn at(pointer: impl Into<String>, message: impl Into<String>) -> Self {
        Diagnostic::new(Location::pointer(pointer), message)
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.location, self.message)
    }
}

/// Why a worksheet could not be loaded.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    /// The file could not be read.
    #[error("{}: cannot read file: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// The file extension does not map to a supported input format.
    #[error(
        "{}: unsupported extension {ext:?} (supported: .json, .yaml, .yml, .toml)",
        path.display()
    )]
    UnsupportedExtension { path: PathBuf, ext: String },
    /// The worksheet was read but is not valid.
    #[error("{} error(s) in worksheet", diagnostics.len())]
    Invalid { diagnostics: Vec<Diagnostic> },
}

impl LoadError {
    /// Diagnostics of an invalid worksheet; empty for the other variants.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        match self {
            LoadError::Invalid { diagnostics } => diagnostics,
            _ => &[],
        }
    }
}

impl From<Vec<Diagnostic>> for LoadError {
    fn from(diagnostics: Vec<Diagnostic>) -> Self {
        LoadError::Invalid { diagnostics }
    }
}
