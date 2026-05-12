use std::{error::Error, path::Path, process::Command};

use chrono::Utc;
use rqtk_core::{BaselineName, RequirementSet};
use semver::Version;

use crate::output;

pub fn run(repo_root: &Path, version: String) -> Result<(), Box<dyn Error>> {
    let parsed = Version::parse(&version)?;
    let name: BaselineName = parsed.to_string().parse()?;
    let mut set = RequirementSet::load_from_repo_root(repo_root)?;

    let by = set.git.committer_name()?;
    let date = Utc::now().date_naive();
    let stamped = set.stamp_baseline(&by, date)?;

    if !stamped.is_empty() {
        let mut add = Command::new("git");
        add.arg("add");
        for path in &stamped {
            add.arg(path);
        }
        let status = add.current_dir(repo_root).status()?;
        if !status.success() {
            return Err("git add failed".into());
        }

        let msg = format!("baseline: stamp requirements at {name}");
        let status = Command::new("git")
            .args(["commit", "-m", &msg])
            .current_dir(repo_root)
            .status()?;
        if !status.success() {
            return Err("git commit failed".into());
        }
    }

    let tag_name = format!("rqtk/{name}");
    let message = format!(
        "Requirements baseline {name}\n\nProject: {}",
        set.config.project.name
    );
    set.git.create_baseline_tag(&name, &message)?;

    output::success(
        "Baseline created",
        &[
            ("tag", tag_name.as_str()),
            ("stamped", &stamped.len().to_string()),
            ("by", &by),
        ],
    );
    Ok(())
}
