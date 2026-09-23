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
pub mod skills;
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

/// Insert or replace the region between `begin` and `end` marker lines in `existing`,
/// leaving everything else untouched. A new region is appended after a blank line.
pub fn splice_managed(existing: &str, begin: &str, end: &str, body: &str) -> String {
    let region = format!("{begin}\n{body}\n{end}\n");
    if let (Some(start), Some(stop)) = (existing.find(begin), existing.find(end))
        && start < stop
    {
        let after = existing[stop..]
            .find('\n')
            .map_or(existing.len(), |i| stop + i + 1);
        return format!("{}{region}{}", &existing[..start], &existing[after..]);
    }
    let trimmed = existing.trim_end_matches('\n');
    if trimmed.is_empty() {
        region
    } else {
        format!("{trimmed}\n\n{region}")
    }
}
