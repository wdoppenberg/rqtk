use std::{error::Error, path::Path};

use rqtk_core::RequirementSet;

use crate::output;

pub fn run(repo_root: &Path, from: String, to: String) -> Result<(), Box<dyn Error>> {
    let set = RequirementSet::load_from_repo_root(repo_root)?;
    let mut changed = Vec::new();
    for (id, req) in &set.requirements {
        let has_from = req
            .requirement
            .history
            .iter()
            .any(|h| h.version.to_string() == from);
        let has_to = req
            .requirement
            .history
            .iter()
            .any(|h| h.version.to_string() == to);
        if has_from ^ has_to {
            changed.push(id.clone());
        }
    }
    if changed.is_empty() {
        output::success(&format!("No deltas between {from} and {to}"), &[]);
    } else {
        output::section(
            "Delta",
            &format!("{from} → {to}  ({} changed)", changed.len()),
        );
        for id in &changed {
            output::item(&id.to_string());
        }
        println!();
    }
    Ok(())
}
