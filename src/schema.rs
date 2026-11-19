//! The embedded worksheet JSON Schema and structural validation.

use std::sync::OnceLock;

use jsonschema::Validator;
use serde_json::Value;

use crate::error::Diagnostic;

/// The worksheet JSON Schema (draft 2020-12), embedded at build time.
pub const WORKSHEET_SCHEMA: &str = include_str!("../schema/worksheet.schema.json");

fn validator() -> &'static Validator {
    static VALIDATOR: OnceLock<Validator> = OnceLock::new();
    VALIDATOR.get_or_init(|| {
        let schema: Value =
            serde_json::from_str(WORKSHEET_SCHEMA).expect("embedded schema is valid JSON");
        jsonschema::draft202012::new(&schema).expect("embedded schema compiles")
    })
}

/// Validates `instance` against the worksheet schema, returning every violation.
pub fn validate(instance: &Value) -> Vec<Diagnostic> {
    validator()
        .iter_errors(instance)
        .map(|error| Diagnostic::at(error.instance_path().to_string(), error.to_string()))
        .collect()
}
