use std::{error::Error, path::PathBuf};

use rqtk_core::RequirementSet;
use rqtk_export::{ExportFormat, default_export_path, export_set};
use serde::Serialize;

use crate::output::{self, Ctx, Exit};

#[derive(Serialize)]
struct Report<'a> {
    format: &'a ExportFormat,
    path: PathBuf,
    requirements: usize,
}

pub fn run(ctx: &Ctx, format: ExportFormat, out: Option<PathBuf>) -> Result<Exit, Box<dyn Error>> {
    let (set, _) = RequirementSet::load_from_repo_root(&ctx.root)?.validate();
    let out = out.unwrap_or_else(|| default_export_path(set.requirements_dir(), &format));
    export_set(&set, &format, &out)?;
    let report = Report {
        format: &format,
        path: output::relative(&out, &ctx.root),
        requirements: set.requirements().len(),
    };
    if ctx.json() {
        output::json(&report)?;
    } else {
        output::success(
            "Requirements exported",
            &[
                ("format", &serde_json::to_string(&format)?),
                ("output", &report.path.display().to_string()),
                ("count", &report.requirements.to_string()),
            ],
        );
    }
    Ok(Exit::Ok)
}
