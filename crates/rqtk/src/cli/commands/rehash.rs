use std::{error::Error, path::PathBuf};

use rqtk_core::{EntityRef, RequirementSet};
use serde::Serialize;

use crate::cli::output::{self, Ctx, Exit};

#[derive(Serialize)]
struct Updated {
    subject: EntityRef,
    path: PathBuf,
    hash: String,
}

#[derive(Serialize)]
struct Report {
    updated: Vec<Updated>,
    dry_run: bool,
}

pub fn run(ctx: &Ctx, dry_run: bool) -> Result<Exit, Box<dyn Error>> {
    let mut set = RequirementSet::load_from_repo_root(&ctx.root)?;
    let stale = if dry_run {
        set.stale_hashes()
    } else {
        set.rehash()?
    };
    let updated: Vec<Updated> = stale
        .into_iter()
        .map(|s| Updated {
            path: output::relative(&s.path, &ctx.root),
            subject: s.subject,
            hash: s.hash,
        })
        .collect();

    if ctx.json() {
        output::json(&Report { updated, dry_run })?;
    } else if updated.is_empty() {
        output::success("All content hashes are current", &[]);
    } else {
        let label = if dry_run {
            "Content hashes would be updated"
        } else {
            "Content hashes updated"
        };
        let ids: Vec<String> = updated.iter().map(|u| u.subject.to_string()).collect();
        let pairs: Vec<(&str, &str)> = ids.iter().map(|id| ("id", id.as_str())).collect();
        output::success(label, &pairs);
    }
    Ok(Exit::Ok)
}
