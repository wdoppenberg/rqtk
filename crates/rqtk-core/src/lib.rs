pub mod diagnostic;
pub mod error;
pub mod evidence;
pub mod git;
pub mod io;
pub mod model;
pub mod query;
pub mod repository;
pub mod rules;
pub mod scan;
pub mod schema;
pub mod status;
pub mod validation;
#[cfg(feature = "verification-lookup")]
pub mod verification;

pub use diagnostic::{Diagnostic, Location, Severity};
pub use error::RqtkError;
pub use evidence::{ActivityEvidence, Evidence, EvidenceChange, Outcome, TestOutcome, TestResult};
pub use git::{
    Baseline, BaselineName, ChangeKind, CommitHash, CommitInfo, GitContext, ModifiedRequirement,
    RequirementDiff,
};
pub use model::{
    Acceptance, Allocation, Approval, Category, Config, CriticalityPolicy, EntityRef,
    ExternalTrace, IdentificationScheme, LifecyclePolicy, Need, NeedId, Parameter, PriorityPolicy,
    ProjectMeta, ProjectOrganization, RepositoryLayout, Requirement, RequirementId, Risk,
    SCHEMA_VERSION, ScaffoldInput, ScanConfig, Stakeholder, StakeholderAuthority,
    StakeholderConcerns, StakeholderId, Standard, Traceability, TypePolicy, ValidationRules,
    ValidationSpec, Verification, VerificationActivity, VerificationPolicy,
};
pub use repository::{
    Loaded, RequirementSet, SatisfactionStatus, StaleHash, TraceView, Validated, load_config,
};
pub use scan::SourceLink;
pub use status::{ActivityState, ClosureStatus, RequirementVerification};
