//! Reading JSON, YAML and TOML worksheets into the JSON data model.

use std::path::Path;

use serde_json::{Map, Number, Value};

use crate::error::{Diagnostic, Location};

/// A supported worksheet text format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Json,
    Yaml,
    Toml,
}

impl Format {
    /// Picks the format from the file extension (case-insensitive).
    /// Returns the extension that was found when it is not supported.
    pub fn from_path(path: &Path) -> Result<Format, String> {
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().into_owned())
            .unwrap_or_default();
        match ext.to_ascii_lowercase().as_str() {
            "json" => Ok(Format::Json),
            "yaml" | "yml" => Ok(Format::Yaml),
            "toml" => Ok(Format::Toml),
            _ => Err(ext),
        }
    }
}

/// Parses `text` in `format` into a JSON value.
pub fn parse(text: &str, format: Format) -> Result<Value, Vec<Diagnostic>> {
    match format {
        Format::Json => parse_json(text),
        Format::Yaml => parse_yaml(text),
        Format::Toml => parse_toml(text),
    }
}

fn parse_json(text: &str) -> Result<Value, Vec<Diagnostic>> {
    serde_json::from_str(text).map_err(|e| {
        let location = if e.line() > 0 {
            Location::LineColumn {
                line: e.line(),
                column: e.column(),
            }
        } else {
            Location::Unknown
        };
        vec![Diagnostic::new(location, format!("invalid JSON: {e}"))]
    })
}

fn parse_yaml(text: &str) -> Result<Value, Vec<Diagnostic>> {
    let yaml: serde_norway::Value = serde_norway::from_str(text).map_err(|e| {
        let location = e
            .location()
            .map_or(Location::Unknown, |l| Location::LineColumn {
                line: l.line(),
                column: l.column(),
            });
        vec![Diagnostic::new(location, format!("invalid YAML: {e}"))]
    })?;
    let mut errors = Vec::new();
    let value = yaml_to_json(yaml, &mut String::new(), &mut errors);
    if errors.is_empty() {
        Ok(value)
    } else {
        Err(errors)
    }
}

fn yaml_to_json(
    yaml: serde_norway::Value,
    path: &mut String,
    errors: &mut Vec<Diagnostic>,
) -> Value {
    use serde_norway::Value as Y;
    match yaml {
        Y::Null => Value::Null,
        Y::Bool(b) => Value::Bool(b),
        Y::String(s) => Value::String(s),
        Y::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::from(i)
            } else if let Some(u) = n.as_u64() {
                Value::from(u)
            } else {
                float_to_json(n.as_f64().unwrap_or(f64::NAN), path, errors)
            }
        }
        Y::Sequence(items) => Value::Array(
            items
                .into_iter()
                .enumerate()
                .map(|(i, item)| {
                    with_segment(path, &i.to_string(), |p| yaml_to_json(item, p, errors))
                })
                .collect(),
        ),
        Y::Mapping(mapping) => {
            let mut object = Map::new();
            for (key, item) in mapping {
                let key = match key {
                    Y::String(s) => s,
                    Y::Number(n) if n.is_i64() || n.is_u64() => n.to_string(),
                    other => {
                        errors.push(Diagnostic::at(
                            path.clone(),
                            format!("mapping key must be a string or an integer, found {other:?}"),
                        ));
                        continue;
                    }
                };
                let converted = with_segment(path, &key, |p| yaml_to_json(item, p, errors));
                object.insert(key, converted);
            }
            Value::Object(object)
        }
        Y::Tagged(tagged) => {
            errors.push(Diagnostic::at(
                path.clone(),
                format!("YAML tag {} has no JSON equivalent", tagged.tag),
            ));
            Value::Null
        }
    }
}

fn parse_toml(text: &str) -> Result<Value, Vec<Diagnostic>> {
    let table: toml::Table = toml::from_str(text).map_err(|e| {
        let location = e
            .span()
            .map_or(Location::Unknown, |span| line_column(text, span.start));
        vec![Diagnostic::new(
            location,
            format!("invalid TOML: {}", e.message()),
        )]
    })?;
    let mut errors = Vec::new();
    let value = toml_to_json(toml::Value::Table(table), &mut String::new(), &mut errors);
    if errors.is_empty() {
        Ok(value)
    } else {
        Err(errors)
    }
}

fn toml_to_json(toml: toml::Value, path: &mut String, errors: &mut Vec<Diagnostic>) -> Value {
    use toml::Value as T;
    match toml {
        T::String(s) => Value::String(s),
        T::Integer(i) => Value::from(i),
        T::Float(f) => float_to_json(f, path, errors),
        T::Boolean(b) => Value::Bool(b),
        T::Datetime(dt) => {
            errors.push(Diagnostic::at(
                path.clone(),
                format!("date-time value {dt} has no JSON equivalent"),
            ));
            Value::Null
        }
        T::Array(items) => Value::Array(
            items
                .into_iter()
                .enumerate()
                .map(|(i, item)| {
                    with_segment(path, &i.to_string(), |p| toml_to_json(item, p, errors))
                })
                .collect(),
        ),
        T::Table(table) => Value::Object(
            table
                .into_iter()
                .map(|(key, item)| {
                    let converted = with_segment(path, &key, |p| toml_to_json(item, p, errors));
                    (key, converted)
                })
                .collect(),
        ),
    }
}

fn float_to_json(f: f64, path: &str, errors: &mut Vec<Diagnostic>) -> Value {
    match Number::from_f64(f) {
        Some(n) => Value::Number(n),
        None => {
            errors.push(Diagnostic::at(
                path,
                format!("number {f} has no JSON equivalent"),
            ));
            Value::Null
        }
    }
}

/// Runs `f` with `segment` appended to the JSON Pointer `path`, then restores it.
fn with_segment<R>(path: &mut String, segment: &str, f: impl FnOnce(&mut String) -> R) -> R {
    let len = path.len();
    path.push('/');
    path.push_str(&escape_pointer_segment(segment));
    let result = f(path);
    path.truncate(len);
    result
}

/// Escapes a JSON Pointer reference token (RFC 6901).
pub fn escape_pointer_segment(segment: &str) -> String {
    segment.replace('~', "~0").replace('/', "~1")
}

/// Converts a byte offset into a 1-based line and column (in characters).
fn line_column(text: &str, offset: usize) -> Location {
    let before = &text[..offset.min(text.len())];
    let line = before.matches('\n').count() + 1;
    let column = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
    Location::LineColumn { line, column }
}
