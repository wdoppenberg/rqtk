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
    let title = |id: &RequirementId| {
        set.requirements()
            .get(id)
            .map_or_else(|| id.to_string(), |r| format!("{id}  {}", r.title))
    };
    output::section("Traceability", &title(&req_id));
    if view.upward.is_empty() {
        output::subsection("▲", "Parents: none, a top-level requirement");
    } else {
        output::subsection("▲", &format!("Parents ({})", view.upward.len()));
        for id in &view.upward {
            output::item(&title(id));
        }
    }
    if view.downward.is_empty() {
        output::subsection("▼", "Children: none, no requirement derives from it");
    } else {
        output::subsection("▼", &format!("Children ({})", view.downward.len()));
        for id in &view.downward {
            output::item(&title(id));
        }
    }
    println!();
    Ok(Exit::Ok)
}
