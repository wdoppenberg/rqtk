//! JSON Schemas for the client's documents and data types, as published with rqtk.

use crate::entries::{ActivityEntry, NeedEntry, RequirementEntry, StakeholderEntry};
use crate::error::Error;
use crate::filter::Filter;
use crate::repository::RepositoryInfo;
use crate::views::{
    BaselinesDoc, ContextDoc, CoverageDoc, DiffDoc, HistoryDoc, ImpactDoc, LintDoc, TraceDoc,
    schema_id,
};
use schemars::generate::SchemaSettings;
use schemars::{JsonSchema, schema_for};
use serde_json::Value;

/// The schema of `T` as rqtk writes it: a field left out when empty is optional.
pub fn output_schema<T: JsonSchema>() -> Value {
    let schema = SchemaSettings::draft2020_12()
        .for_serialize()
        .into_generator()
        .into_root_schema_for::<T>();
    serde_json::to_value(schema).expect("a schema serialises")
}

/// The schema of `T`, a document whose `schema` field must be `rqtk/<name>/<version>`.
pub fn document_schema<T: JsonSchema>(name: &str) -> Value {
    pin_schema_field(output_schema::<T>(), name)
}

/// Require `schema: "rqtk/<name>/<version>"` in a document's JSON Schema.
pub fn pin_schema_field(mut schema: Value, name: &str) -> Value {
    let object = schema.as_object_mut().expect("a schema is an object");
    object.insert(
        "$id".to_owned(),
        Value::String(format!(
            "https://rqtk.dev/schema/{}.json",
            schema_id(name).trim_start_matches("rqtk/")
        )),
    );
    let properties = object
        .entry("properties")
        .or_insert_with(|| Value::Object(Default::default()));
    properties["schema"] = serde_json::json!({ "type": "string", "const": schema_id(name) });
    let required = object
        .entry("required")
        .or_insert_with(|| Value::Array(Vec::new()));
    if let Value::Array(required) = required
        && !required.iter().any(|r| r == "schema")
    {
        required.insert(0, Value::String("schema".to_owned()));
    }
    schema
}

/// The views a [`crate::Repository`] returns, which are also what `rqtk <command> --json`
/// prints: `(document name, schema)`.
pub fn view_schemas() -> Vec<(&'static str, Value)> {
    vec![
        ("context", document_schema::<ContextDoc>("context")),
        ("trace", document_schema::<TraceDoc>("trace")),
        ("coverage", document_schema::<CoverageDoc>("coverage")),
        ("lint", document_schema::<LintDoc>("lint")),
        ("impact", document_schema::<ImpactDoc>("impact")),
        ("diff", document_schema::<DiffDoc>("diff")),
        ("baselines", document_schema::<BaselinesDoc>("baselines")),
        ("history", document_schema::<HistoryDoc>("history")),
    ]
}

/// The other types the client returns or accepts: `(type name, schema)`.
pub fn type_schemas() -> Vec<(&'static str, Value)> {
    vec![
        // The one input: what callers may pass.
        (
            "filter",
            serde_json::to_value(schema_for!(Filter)).expect("a schema serialises"),
        ),
        ("requirement-entry", output_schema::<RequirementEntry>()),
        ("need-entry", output_schema::<NeedEntry>()),
        ("stakeholder-entry", output_schema::<StakeholderEntry>()),
        ("activity-entry", output_schema::<ActivityEntry>()),
        ("repository-info", output_schema::<RepositoryInfo>()),
        ("client-error", output_schema::<Error>()),
    ]
}
