use std::{error::Error, path::Path};

use rqtk_core::{RequirementId, RequirementSet};

use crate::output;

pub fn run(repo_root: &Path, id: String) -> Result<(), Box<dyn Error>> {
    let set = RequirementSet::load_from_repo_root(repo_root)?;
    let req_id = RequirementId(id);
    let view = set.trace_view(&req_id)?;
    output::section("Traceability", &req_id.to_string());
    output::subsection("▲", &format!("Parents ({})", view.upward.len()));
    if view.upward.is_empty() {
        output::item("—  no parents");
    } else {
        for id in &view.upward {
            output::item(&id.to_string());
        }
    }
    output::subsection("▼", &format!("Children ({})", view.downward.len()));
    output::item(&req_id.to_string());
    if view.downward.is_empty() {
        output::item("—  no children");
    } else {
        for id in &view.downward {
            output::item(&id.to_string());
        }
    }
    println!();
    Ok(())
}
