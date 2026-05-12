use std::{error::Error, path::Path};

use rqtk_core::{BaselineName, RequirementSet};
use semver::Version;

use crate::output;

pub fn run(repo_root: &Path, version: String) -> Result<(), Box<dyn Error>> {
    let parsed = Version::parse(&version)?;
    let name: BaselineName = parsed.to_string().parse()?;
    let set = RequirementSet::load_from_repo_root(repo_root)?;
    let tag_name = format!("rqtk/{name}");
    let message = format!(
        "Requirements baseline {name}\n\nProject: {}",
        set.config.project.name
    );
    set.git.create_baseline_tag(&name, &message)?;
    output::success("Baseline created", &[("tag", &tag_name)]);
    Ok(())
}
