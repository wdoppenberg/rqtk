pub mod add;
pub mod add_activity;
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
pub mod interfaces;
pub mod lint;
pub mod log;
pub mod open;
pub mod rehash;
pub mod report;
pub mod review;
pub mod scan;
pub mod schema;
pub mod search;
pub mod skills;
pub mod trace;
pub mod verify;

use rqtk_core::{Evidence, RequirementSet, RqtkError, SourceLink, Validated};

/// The repository at `--repo-root`, loaded through the client: the views the CLI prints are the
/// client's documents.
pub fn open(ctx: &crate::cli::output::Ctx) -> Result<rqtk_client::Repository, rqtk_client::Error> {
    rqtk_client::Repository::open(&ctx.root)
}

/// A client error, as a usage error when it names an item that doesn't exist.
pub fn usage_if_not_found(e: rqtk_client::Error) -> Box<dyn std::error::Error> {
    if e.code == rqtk_client::ErrorCode::NotFound {
        Box::new(crate::cli::output::Usage(e.message))
    } else {
        Box::new(e)
    }
}

/// Source links and recorded evidence, the inputs to verification status.
pub fn load_links_and_evidence(
    set: &RequirementSet<Validated>,
) -> Result<(Vec<SourceLink>, Evidence), RqtkError> {
    let links = rqtk_core::scan::scan(set.repo_root(), &set.config().scan)?;
    let evidence = Evidence::load(set.repo_root())?;
    Ok((links, evidence))
}

/// Refuse `id` when it names a vendored requirement (`<alias>:<ID>`): those belong to the
/// provider and are read-only.
pub fn refuse_vendored<S>(
    set: &RequirementSet<S>,
    id: &str,
    command: &str,
) -> Result<(), crate::cli::output::Usage> {
    match set.vendored_alias(id) {
        Some(alias) => Err(crate::cli::output::Usage(format!(
            "`{id}` is a vendored requirement of interface source `{alias}`; vendored requirements are read-only, so `rqtk {command}` does not accept it"
        ))),
        None => Ok(()),
    }
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
