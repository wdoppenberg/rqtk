use std::error::Error;

use rqtk_core::{RequirementId, RequirementSet};
use serde::Serialize;

use crate::cli::output::{self, Ctx, Exit};

#[derive(Serialize)]
struct Report<'a> {
    id: &'a RequirementId,
    ancestors: &'a [RequirementId],
    descendants: &'a [RequirementId],
}

pub fn run(ctx: &Ctx, id: String) -> Result<Exit, Box<dyn Error>> {
    let set = RequirementSet::load_from_repo_root(&ctx.root)?;
    let req_id = RequirementId(id);
    let view = set.trace_view(&req_id)?;
    if ctx.json() {
        output::json(&Report {
            id: &req_id,
            ancestors: &view.upward,
            descendants: &view.downward,
        })?;
        return Ok(Exit::Ok);
    }
    output::section("Traceability", req_id.as_ref());
    output::subsection("▲", &format!("Parents ({})", view.upward.len()));
    if view.upward.is_empty() {
        output::item("—  no parents");
    }
    for id in &view.upward {
        output::item(id.as_ref());
    }
    output::subsection("▼", &format!("Children ({})", view.downward.len()));
    if view.downward.is_empty() {
        output::item("—  no children");
    }
    for id in &view.downward {
        output::item(id.as_ref());
    }
    println!();
    Ok(Exit::Ok)
}
