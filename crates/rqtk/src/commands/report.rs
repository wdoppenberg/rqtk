use std::{error::Error, path::Path, path::PathBuf};

use rqtk_core::RequirementSet;

use crate::output;

pub fn run(repo_root: &Path, out: Option<PathBuf>) -> Result<(), Box<dyn Error>> {
    let set = RequirementSet::load_from_repo_root(repo_root)?;
    let (set, _) = set.validate();
    let (links, evidence) = super::load_links_and_evidence(&set)?;
    let verification = set.verification_status(&links, &evidence);
    let markdown =
        rqtk_report::render_report(&set, &verification, chrono::Local::now().date_naive());
    match out {
        Some(path) => {
            std::fs::write(&path, markdown)?;
            output::success("Report written", &[("output", &path.display().to_string())]);
        }
        None => print!("{markdown}"),
    }
    Ok(())
}
