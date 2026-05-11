use std::{error::Error, path::Path};

use rqtk_core::{RequirementSet, RqtkError};

pub fn run(repo_root: &Path, format: String) -> Result<(), Box<dyn Error>> {
    let set = RequirementSet::load_from_repo_root(repo_root)?;
    if format != "dot" {
        return Err(Box::new(RqtkError::UnsupportedExportFormat(format)));
    }
    let (set, _) = set.validate();
    println!("{}", set.to_dot());
    Ok(())
}
