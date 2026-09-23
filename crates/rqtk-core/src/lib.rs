pub mod diagnostic;
pub mod error;
pub mod git;
pub mod io;
pub mod model;
pub mod repository;
pub mod rules;
pub mod validation;
#[cfg(feature = "verification-lookup")]
pub mod verification;

pub use diagnostic::{Diagnostic, Location, Severity};
pub use error::RqtkError;
pub use git::{
    Baseline, BaselineName, ChangeKind, CommitHash, CommitInfo, GitContext, ModifiedRequirement,
    RequirementDiff,
};
pub use model::{
    Acceptance, Allocation, Approval, Category, Config, CriticalityPolicy, EntityRef,
    ExternalTrace, IdentificationScheme, LifecyclePolicy, Need, NeedId, Parameter, PriorityPolicy,
    ProjectMeta, ProjectOrganization, RepositoryLayout, Requirement, RequirementId, Risk,
    SCHEMA_VERSION, ScaffoldInput, Stakeholder, StakeholderAuthority, StakeholderConcerns,
    StakeholderId, Standard, Traceability, TypePolicy, ValidationRules, ValidationSpec,
    Verification, VerificationActivity, VerificationPolicy,
};
pub use repository::{
    ClosureStatus, Loaded, RequirementSet, SatisfactionStatus, TraceView, Validated, load_config,
};
