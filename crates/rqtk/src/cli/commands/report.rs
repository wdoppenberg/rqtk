use std::{error::Error, path::PathBuf};

use rqtk_core::RequirementSet;

use crate::cli::output::{self, Ctx, Exit};

pub fn run(ctx: &Ctx, out: Option<PathBuf>) -> Result<Exit, Box<dyn Error>> {
    ctx.require_text("report")?;
    let (set, _) = RequirementSet::load_from_repo_root(&ctx.root)?.validate();
    let (links, evidence) = super::load_links_and_evidence(&set)?;
    let verification = set.verification_status(&links, &evidence);
    let commit = set.git().head_commit().map(|c| c.full().to_owned());
    let markdown = rqtk_report::render_report(&rqtk_report::ReportInput {
        set: &set,
        verification: &verification,
        evidence: &evidence,
        links: &links,
        commit: commit.as_deref(),
        date: chrono::Local::now().date_naive(),
    });
    match out {
        Some(path) => {
            std::fs::write(&path, markdown)?;
            output::success("Report written", &[("output", &path.display().to_string())]);
        }
        None => print!("{markdown}"),
    }
    Ok(Exit::Ok)
}
