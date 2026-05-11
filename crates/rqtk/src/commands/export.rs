use std::{error::Error, path::Path, path::PathBuf};

use rqtk_core::RequirementSet;
use rqtk_export::{ExportFormat, default_export_path, export_set};

use crate::output;

pub fn run(
    repo_root: &Path,
    format: ExportFormat,
    out: Option<PathBuf>,
) -> Result<(), Box<dyn Error>> {
    let set = RequirementSet::load_from_repo_root(repo_root)?;
    let count = set.requirements.len();
    let (set, _) = set.validate();
    let out = out.unwrap_or_else(|| default_export_path(&set.root, &format));
    export_set(&set, &format, &out)?;
    output::success(
        "Requirements exported",
        &[
            ("format", &serde_json::to_string(&format)?),
            ("output", &out.display().to_string()),
            ("count", &count.to_string()),
        ],
    );
    Ok(())
}
