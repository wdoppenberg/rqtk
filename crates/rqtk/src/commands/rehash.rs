use std::{error::Error, path::Path};

use rqtk_core::RequirementSet;

use crate::output;

pub fn run(repo_root: &Path) -> Result<(), Box<dyn Error>> {
    let mut set = RequirementSet::load_from_repo_root(repo_root)?;
    let updated = set.rehash()?;
    if updated == 0 {
        output::success("All content hashes are current", &[]);
    } else {
        output::success("Content hashes updated", &[("count", &updated.to_string())]);
    }
    Ok(())
}
