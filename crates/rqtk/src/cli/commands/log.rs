use std::error::Error;

use rqtk_core::{EntityRef, RequirementId, RequirementSet, RqtkError};
use serde::Serialize;

use crate::cli::output::{self, Ctx, Exit};

#[derive(Serialize)]
struct Commit {
    hash: String,
    author: String,
    timestamp: Option<String>,
    message: String,
}

#[derive(Serialize)]
struct Report<'a> {
    id: &'a RequirementId,
    commits: Vec<Commit>,
}

pub fn run(ctx: &Ctx, id: String) -> Result<Exit, Box<dyn Error>> {
    let set = RequirementSet::load_from_repo_root(&ctx.root)?;
    let req_id = RequirementId(id);
    let path = set
        .path_of(&EntityRef::Requirement(req_id.clone()))
        .ok_or_else(|| RqtkError::RequirementNotFound(req_id.clone()))?;
    let commits = set.git().requirement_history(path)?;

    if ctx.json() {
        output::json(&Report {
            id: &req_id,
            commits: commits
                .iter()
                .map(|c| Commit {
                    hash: c.hash.full().to_owned(),
                    author: c.author.clone(),
                    timestamp: c.timestamp.map(|t| t.to_rfc3339()),
                    message: c.message.trim_end().to_owned(),
                })
                .collect(),
        })?;
        return Ok(Exit::Ok);
    }

    if commits.is_empty() {
        output::success("No committed history found", &[("id", req_id.as_ref())]);
        return Ok(Exit::Ok);
    }
    output::section(req_id.as_ref(), &format!("{} commits", commits.len()));
    for c in &commits {
        let date = c.timestamp.map_or_else(
            || "unknown".to_string(),
            |t| t.format("%Y-%m-%d").to_string(),
        );
        println!(
            "  {} {}  {}  {}",
            console::style(c.hash.short()).yellow(),
            console::style(date).dim(),
            console::style(&c.author).cyan(),
            c.message.trim_end()
        );
    }
    println!();
    Ok(Exit::Ok)
}
