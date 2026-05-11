use std::{error::Error, path::Path};

use rqtk_core::RequirementSet;

use crate::output;

pub fn run(repo_root: &Path) -> Result<(), Box<dyn Error>> {
    let set = RequirementSet::load_from_repo_root(repo_root)?;
    let total = set.requirements.len();
    let (set, _) = set.validate();
    let gaps = set.coverage_gaps();
    if gaps.is_empty() {
        output::success(
            &format!("Full coverage ({total} of {total} requirements traced)"),
            &[],
        );
    } else {
        output::section(
            "Coverage gaps",
            &format!("({} of {total} untraced)", gaps.len()),
        );
        for id in &gaps {
            output::item(&id.to_string());
        }
        println!();
        std::process::exit(3);
    }
    Ok(())
}
