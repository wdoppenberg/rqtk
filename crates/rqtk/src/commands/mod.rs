pub mod add;
pub mod add_need;
pub mod add_stakeholder;
pub mod baseline;
pub mod context;
pub mod coverage;
pub mod diff;
pub mod explain;
pub mod export;
pub mod graph;
pub mod impact;
pub mod init;
pub mod install_hook;
pub mod lint;
pub mod log;
pub mod open;
pub mod rehash;
pub mod report;
pub mod scan;
pub mod schema;
pub mod search;
pub mod trace;
pub mod verify;

use rqtk_core::{Evidence, RequirementSet, RqtkError, SourceLink, Validated};

/// Source links and recorded evidence, the inputs to verification status.
pub fn load_links_and_evidence(
    set: &RequirementSet<Validated>,
) -> Result<(Vec<SourceLink>, Evidence), RqtkError> {
    let links = rqtk_core::scan::scan(&set.repo_root, &set.config.scan)?;
    let evidence = Evidence::load(&set.repo_root)?;
    Ok((links, evidence))
}
