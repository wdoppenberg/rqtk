use std::{error::Error, path::Path};

use rqtk_core::RequirementSet;

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum GraphFormat {
    /// Graphviz DOT.
    Dot,
}

pub fn run(repo_root: &Path, format: GraphFormat) -> Result<(), Box<dyn Error>> {
    let set = RequirementSet::load_from_repo_root(repo_root)?;
    let (set, _) = set.validate();
    match format {
        GraphFormat::Dot => print!("{}", set.to_dot()),
    }
    Ok(())
}
