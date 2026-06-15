use std::{error::Error, path::Path};

use rqtk_core::{RequirementSet, RqtkError};

pub fn run(repo_root: &Path, format: String) -> Result<(), Box<dyn Error>> {
    let set = RequirementSet::load_from_repo_root(repo_root)?;
    let (set, _) = set.validate();
    match format.as_str() {
        "dot" => println!("{}", set.to_dot()),
        "graphml" => println!("{}", set.to_graphml()),
        _ => return Err(Box::new(RqtkError::UnsupportedExportFormat(format))),
    }
    Ok(())
}
