//! Read rqtk repositories from Rust: requirements, needs and stakeholders with their links,
//! verification status, lint findings and change history.
//!
//! ```no_run
//! use rqtk_client::{Filter, Repository};
//!
//! let repo = Repository::open(".")?;
//! let suspect = repo.find_requirements(&Filter {
//!     status: vec![rqtk_client::ClosureStatus::Suspect],
//!     ..Filter::default()
//! });
//! for entry in suspect {
//!     println!("{} {}", entry.id, entry.requirement.title);
//! }
//! # Ok::<(), rqtk_client::Error>(())
//! ```
//!
//! A [`Repository`] is a snapshot: everything is loaded and checked when it opens, and every
//! query answers from that state until [`Repository::reload`]. The client only reads; the
//! `rqtk` command line writes.
//!
//! Every view is the same document `rqtk <command> --json` prints, and its JSON Schema is
//! published with rqtk. This crate's public items follow semantic versioning; rqtk-core,
//! which it is built on, is free to change.

mod entries;
mod error;
mod filter;
#[cfg(feature = "remote")]
mod remote;
mod repository;
pub mod schemas;
pub mod views;

pub use entries::{ActivityEntry, Link, LinkKind, NeedEntry, RequirementEntry, StakeholderEntry};
pub use error::{Error, ErrorCode, Result};
pub use filter::Filter;
#[cfg(feature = "remote")]
pub use remote::{RemoteOptions, cache_dir, clear_cache};
pub use repository::{Origin, Repository, RepositoryInfo};

// The model and view types the client returns. They are part of this crate's API.
pub use rqtk_core::query::{
    ChangeKind, ConsumedSummary, Context, Direction, Downstream, Impact, InterfaceContext,
    ItemChange, NeedContext, RequirementContext, Reverify, ReverifyReason, StakeholderContext,
    Summary, TraceLink,
};
pub use rqtk_core::{
    ActivityEvidence, ActivityState, ActivityStatus, ClosureStatus, Config, Diagnostic, EntityRef,
    Location, Need, Outcome, Requirement, RequirementVerification, Severity, SourceLink,
    Stakeholder, SuspectReason, VerificationActivity,
};

// Bindings move a repository between threads and query it from several at once
// (RQTK-CLIENT-0021).
const _: fn() = || {
    fn shareable<T: Send + Sync>() {}
    shareable::<Repository>();
};
