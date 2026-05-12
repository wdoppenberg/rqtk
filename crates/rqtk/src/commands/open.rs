use std::{error::Error, path::Path};

use rqtk_core::{RequirementId, RequirementSet, RqtkError};

pub fn run(repo_root: &Path, id: String) -> Result<(), Box<dyn Error>> {
    let set = RequirementSet::load_from_repo_root(repo_root)?;
    let req_id = RequirementId(id);

    let path = set
        .files_by_id
        .get(&req_id)
        .ok_or_else(|| RqtkError::RequirementNotFound(req_id.clone()))?;

    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());

    let status = std::process::Command::new(&editor).arg(path).status()?;

    if !status.success() {
        return Err(format!("editor `{editor}` exited with status {status}").into());
    }

    Ok(())
}
