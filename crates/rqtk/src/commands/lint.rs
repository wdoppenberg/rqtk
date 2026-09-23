use std::{error::Error, path::Path};

use rqtk_core::RequirementSet;

use crate::output;

pub fn run(repo_root: &Path) -> Result<(), Box<dyn Error>> {
    let set = RequirementSet::load_from_repo_root(repo_root)?;
    let total = set.requirements.len() + set.needs.len() + set.stakeholders.len();
    let (_, issues) = set.validate();
    if issues.is_empty() {
        output::success(
            &format!("All requirements passed lint ({total} checked)"),
            &[],
        );
    } else {
        let (errors, warnings) = output::lint_table(&issues, repo_root);
        output::lint_summary(errors, warnings, total);
        if errors > 0 {
            std::process::exit(2);
        }
    }
    Ok(())
}
