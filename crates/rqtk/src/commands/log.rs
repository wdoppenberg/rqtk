use std::{error::Error, path::Path};

use rqtk_core::{RequirementId, RequirementSet, RqtkError};

use crate::output;

pub fn run(repo_root: &Path, id: String) -> Result<(), Box<dyn Error>> {
    let set = RequirementSet::load_from_repo_root(repo_root)?;
    let req_id = RequirementId(id);
    let path = set
        .files_by_id
        .get(&req_id)
        .ok_or_else(|| RqtkError::RequirementNotFound(req_id.clone()))?
        .clone();

    let commits = set.git.requirement_history(&path)?;

    if commits.is_empty() {
        output::success("No committed history found", &[("id", &req_id.to_string())]);
        return Ok(());
    }

    output::section(&req_id.to_string(), &format!("{} commits", commits.len()));
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
            c.message
        );
    }
    println!();
    Ok(())
}
