use std::error::Error;

use rqtk_core::{RequirementId, RequirementSet, RqtkError};

use crate::output::{Ctx, Exit};

pub fn run(ctx: &Ctx, id: String) -> Result<Exit, Box<dyn Error>> {
    ctx.require_text("open")?;
    let set = RequirementSet::load_from_repo_root(&ctx.root)?;
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
    Ok(Exit::Ok)
}
