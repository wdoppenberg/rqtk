//! JSON Schemas for every file kind rqtk reads.

use crate::evidence::EvidenceFile;
use crate::model::{Config, Need, Requirement, Stakeholder};
use schemars::schema_for;

/// File kinds with a schema, as `(name, file it describes)`.
pub const KINDS: &[(&str, &str)] = &[
    ("config", ".rqtk/config.toml"),
    ("requirement", ".rqtk/requirements/**/<ID>.toml"),
    ("need", ".rqtk/needs/<ID>.toml"),
    ("stakeholder", ".rqtk/stakeholders/<ID>.toml"),
    ("evidence", ".rqtk/evidence.toml"),
];

/// The JSON Schema for file kind `kind` (see [`KINDS`]), or `None` for an unknown kind.
pub fn json_schema(kind: &str) -> Option<serde_json::Value> {
    let schema = match kind {
        "config" => schema_for!(Config),
        "requirement" => schema_for!(Requirement),
        "need" => schema_for!(Need),
        "stakeholder" => schema_for!(Stakeholder),
        "evidence" => schema_for!(EvidenceFile),
        _ => return None,
    };
    serde_json::to_value(schema).ok()
}
