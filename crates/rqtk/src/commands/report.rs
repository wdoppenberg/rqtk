use std::{error::Error, path::Path, path::PathBuf};

use rqtk_core::RequirementSet;

use crate::output;

pub fn run(repo_root: &Path, out: PathBuf) -> Result<(), Box<dyn Error>> {
    let set = RequirementSet::load_from_repo_root(repo_root)?;
    let (set, _) = set.validate();
    rqtk_report::generate_report(&set, &out)?;
    output::success("Report written", &[("output", &out.display().to_string())]);
    Ok(())
}
