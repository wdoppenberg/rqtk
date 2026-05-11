use std::{error::Error, path::Path};

use rqtk_core::RequirementSet;
use rqtk_core::io::bump_baseline_version;
use semver::Version;

use crate::output;

pub fn run(repo_root: &Path, version: String) -> Result<(), Box<dyn Error>> {
    let mut set = RequirementSet::load_from_repo_root(repo_root)?;
    let parsed = Version::parse(&version)?;
    bump_baseline_version(&mut set, &parsed)?;
    output::success("Baseline updated", &[("version", &parsed.to_string())]);
    Ok(())
}
