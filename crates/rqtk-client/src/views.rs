//! The views the CLI prints with `--json`, as typed documents. Each carries a `schema` field
//! naming its shape and version, e.g. `rqtk/coverage/2`.

use rqtk_core::query::{ConsumedSummary, Context, Impact};
use rqtk_core::{ClosureStatus, Diagnostic, RequirementVerification};
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::BTreeMap;

/// The major version of every document's shape. Fields are only added within it.
pub const SCHEMA_VERSION: u32 = 2;

/// `rqtk/<name>/<SCHEMA_VERSION>`, the `schema` field of document `name`.
pub fn schema_id(name: &str) -> String {
    format!("rqtk/{name}/{SCHEMA_VERSION}")
}

/// Everything relevant to one item (`rqtk context <ID> --json`).
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ContextDoc {
    pub schema: String,
    #[serde(flatten)]
    pub context: Context,
}

/// A requirement's ancestors and descendants through `trace.parents` (`rqtk trace`).
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct TraceDoc {
    pub schema: String,
    pub id: String,
    /// Nearest first.
    pub ancestors: Vec<String>,
    pub descendants: Vec<String>,
    /// Vendored interface requirements it consumes.
    pub consumes: Vec<ConsumedSummary>,
}

/// One requirement's verification status.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct RequirementStatus {
    pub id: String,
    #[serde(flatten)]
    pub verification: RequirementVerification,
}

/// Which requirements satisfy a need.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct NeedStatus {
    pub id: String,
    pub satisfied_by: Vec<String>,
}

/// A consumed interface with the provider's status and its consumers here.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ConsumedStatus {
    #[serde(flatten)]
    pub interface: ConsumedSummary,
    pub consumers: Vec<String>,
}

/// Need satisfaction and verification status (`rqtk coverage`).
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct CoverageDoc {
    pub schema: String,
    /// Number of requirements per status.
    pub summary: BTreeMap<ClosureStatus, usize>,
    pub requirements: Vec<RequirementStatus>,
    pub needs: Vec<NeedStatus>,
    pub unsatisfied_needs: usize,
    /// Vendored interface requirements consumed here, with the provider's status. Not
    /// counted in `summary`.
    pub consumed: Vec<ConsumedStatus>,
}

/// Lint findings (`rqtk lint`).
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct LintDoc {
    pub schema: String,
    pub errors: usize,
    pub warnings: usize,
    /// Requirements, needs and stakeholders checked.
    pub checked: usize,
    pub diagnostics: Vec<Diagnostic>,
}

/// What changed since a revision and what that puts at risk (`rqtk impact`).
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ImpactDoc {
    pub schema: String,
    #[serde(flatten)]
    pub impact: Impact,
}

/// How a requirement changed between two revisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Modification {
    /// What it demands or how it is verified changed.
    Semantic,
    /// Only other fields changed.
    Cosmetic,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Modified {
    pub id: String,
    pub change: Modification,
}

/// Requirements added, removed and modified between two revisions (`rqtk diff`).
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct DiffDoc {
    pub schema: String,
    pub from: String,
    pub to: String,
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub modified: Vec<Modified>,
}

/// A baseline: an `rqtk/<name>` tag.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct BaselineEntry {
    pub name: String,
    pub tag: String,
    /// RFC 3339.
    pub timestamp: Option<String>,
    pub message: String,
}

/// Every baseline, oldest first (`rqtk baseline --list`).
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct BaselinesDoc {
    pub schema: String,
    pub baselines: Vec<BaselineEntry>,
}

/// A commit that changed a requirement's file.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Commit {
    pub hash: String,
    pub author: String,
    /// RFC 3339.
    pub timestamp: Option<String>,
    pub message: String,
}

/// The commits that changed a requirement, newest first (`rqtk log`).
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct HistoryDoc {
    pub schema: String,
    pub id: String,
    pub commits: Vec<Commit>,
}
