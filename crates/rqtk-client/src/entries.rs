//! Requirements, needs, stakeholders and verification activities, each with its links in both
//! directions.

use rqtk_core::query::Direction;
use rqtk_core::{
    ActivityEvidence, ActivityState, ClosureStatus, Need, Requirement, SourceLink, Stakeholder,
    VerificationActivity,
};
use schemars::JsonSchema;
use serde::Serialize;
use std::path::PathBuf;

/// What a link means, named after the field that declares it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LinkKind {
    /// `trace.parents`: the requirement derives from another.
    Parents,
    /// `trace.derived_from`.
    DerivedFrom,
    /// `trace.refines`.
    Refines,
    /// `trace.conflicts_with`.
    ConflictsWith,
    /// `trace.depends_on`.
    DependsOn,
    /// `trace.related`.
    Related,
    /// `trace.satisfies`: a requirement satisfies a need.
    Satisfies,
    /// `trace.consumes`: a requirement relies on another repository's interface
    /// (`<alias>:<ID>`).
    Consumes,
    /// A need's `stakeholders`.
    Stakeholders,
    /// A requirement's verification activities.
    Activities,
}

impl LinkKind {
    pub(crate) fn from_field(field: &str) -> Option<Self> {
        Some(match field {
            "parents" => Self::Parents,
            "derived_from" => Self::DerivedFrom,
            "refines" => Self::Refines,
            "conflicts_with" => Self::ConflictsWith,
            "depends_on" => Self::DependsOn,
            "related" => Self::Related,
            _ => return None,
        })
    }
}

/// One end of a link, seen from the item that holds it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, JsonSchema)]
pub struct Link {
    pub kind: LinkKind,
    /// `outgoing`: this item declares the link. `incoming`: the other item does, so for
    /// `parents` an incoming link is a child.
    pub direction: Direction,
    /// The other item's ID.
    pub id: String,
}

/// A requirement with its file, verification status and links.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct RequirementEntry {
    pub id: String,
    /// Path relative to the repository root.
    pub path: PathBuf,
    pub status: ClosureStatus,
    pub requirement: Requirement,
    pub links: Vec<Link>,
}

/// A need with its file and links.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct NeedEntry {
    pub id: String,
    pub path: PathBuf,
    /// Whether at least one requirement satisfies it.
    pub satisfied: bool,
    pub need: Need,
    pub links: Vec<Link>,
}

/// A stakeholder with its file and links.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct StakeholderEntry {
    pub id: String,
    pub path: PathBuf,
    pub stakeholder: Stakeholder,
    pub links: Vec<Link>,
}

/// A verification activity with its state, the tests linked to it and its recorded evidence.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ActivityEntry {
    pub id: String,
    /// The requirement it belongs to.
    pub requirement: String,
    pub activity: VerificationActivity,
    #[serde(flatten)]
    pub state: ActivityState,
    pub tests: Vec<SourceLink>,
    pub evidence: Option<ActivityEvidence>,
}
