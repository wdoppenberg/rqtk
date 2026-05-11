use std::{error::Error, path::Path};

use crate::output;

pub fn run(
    repo_root: &Path,
    requirements_dir: Option<&str>,
    force: bool,
) -> Result<(), Box<dyn Error>> {
    let req_dir = requirements_dir.unwrap_or("requirements");
    scaffold_repository(repo_root, req_dir, force)?;
    let config_path = repo_root.join("rqtk.toml");
    let req_path = repo_root.join(req_dir);
    output::success(
        "Repository initialized",
        &[
            ("config", &config_path.display().to_string()),
            ("requirements", &req_path.display().to_string()),
        ],
    );
    Ok(())
}

fn scaffold_repository(
    repo_root: &Path,
    requirements_dir: &str,
    force: bool,
) -> Result<(), Box<dyn Error>> {
    std::fs::create_dir_all(repo_root)?;

    let config_path = repo_root.join("rqtk.toml");
    if config_path.exists() && !force {
        return Err(format!(
            "refusing to overwrite {}; pass --force to replace it",
            config_path.display()
        )
        .into());
    }

    std::fs::write(&config_path, default_rqtk_toml(requirements_dir))?;
    std::fs::create_dir_all(repo_root.join(requirements_dir).join("SYS"))?;
    Ok(())
}

fn default_rqtk_toml(requirements_dir: &str) -> String {
    format!(
        r#"[repository]
requirements_dir = "{requirements_dir}"
required_files = []
required_dirs = []

[project]
name = "Example Project"
version = "0.1.0"

[identification]
id_pattern = "^REQ-(SYS)-\\d{{4}}$"
id_separator = "-"
prefix = "REQ"
zero_padding = 4

[categories.SYS]
name = "System"
level = 1
is_root = true

[types]
allowed = ["Functional", "Performance", "Interface", "Constraint", "Design"]

[verification]
methods = ["Test", "Analysis", "Inspection", "Demonstration"]
levels = ["Unit", "Integration", "System", "Acceptance"]
phases = ["Development", "Pre-release", "Release"]

[lifecycle]
states = ["Draft", "Review", "Approved", "Implemented", "Verified", "Deprecated"]
default_state = "Draft"

[priority]
levels = ["Critical", "High", "Medium", "Low"]

[criticality]
levels = ["Safety-Critical", "Mission-Critical", "Non-Critical"]

[change_control]
ccb_required_after = "Approved"
require_signoff = false
approvers = []

[validation]
require_rationale = true
require_verification_method = true
require_parent_for_levels = []
forbid_orphans = false
forbid_circular_traces = true
allow_tbd = false
allow_tbr = false
shall_keywords = ["shall"]
forbidden_keywords = []

[export]
formats = ["json", "csv", "markdown"]
default_output_dir = "exports"
"#
    )
}
