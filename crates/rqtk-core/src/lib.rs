pub mod diagnostic;
pub mod error;
pub mod evidence;
pub mod git;
pub mod io;
pub mod model;
pub(crate) mod openapi;
pub mod query;
pub mod repository;
pub mod rules;
pub mod scan;
pub mod schema;
pub mod spec;
pub mod status;
#[cfg(feature = "network")]
pub mod sync;
pub mod validation;
pub mod vendor;
#[cfg(feature = "verification-lookup")]
pub mod verification;

pub use diagnostic::{Diagnostic, Location, Severity};
pub use error::RqtkError;
pub use evidence::{
    ActivityEvidence, Evidence, EvidenceChange, Outcome, Review, TestOutcome, TestResult,
};
pub use git::{
    Baseline, BaselineName, ChangeKind, CommitHash, CommitInfo, GitContext, ModifiedRequirement,
    RequirementDiff,
};
pub use model::{
    Acceptance, Allocation, Approval, Category, Config, ConsumedRef, CriticalityPolicy, EntityRef,
    ExternalTrace, IdentificationScheme, Interface, InterfaceSource, LifecyclePolicy, Need, NeedId,
    Parameter, PriorityPolicy, ProjectMeta, ProjectOrganization, RepositoryLayout, Requirement,
    RequirementId, Risk, SCHEMA_VERSION, ScaffoldInput, ScanConfig, SpecEntry, Stakeholder,
    StakeholderAuthority, StakeholderConcerns, StakeholderId, Standard, Traceability, TypePolicy,
    ValidationRules, ValidationSpec, Verification, VerificationActivity, VerificationPolicy,
};
pub use repository::{
    Loaded, RequirementSet, SatisfactionStatus, StaleHash, TraceView, Validated, load_config,
};
pub use scan::SourceLink;
pub use status::{
    ActivityState, ActivityStatus, ClosureStatus, RequirementVerification, SuspectReason,
};
