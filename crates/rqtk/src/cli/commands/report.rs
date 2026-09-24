use std::{error::Error, path::PathBuf};

use rqtk_core::RequirementSet;

use crate::cli::output::{self, Ctx, Exit};

pub fn run(ctx: &Ctx, out: Option<PathBuf>) -> Result<Exit, Box<dyn Error>> {
    ctx.require_text("report")?;
    let (set, _) = RequirementSet::load_from_repo_root(&ctx.root)?.validate();
    let (links, evidence) = super::load_links_and_evidence(&set)?;
    let verification = set.verification_status(&links, &evidence);
    let git = set.git();
    let commit = git.head_commit().map(|c| c.full().to_owned());
    // Requirement and need files that differ from `base`, when git can tell.
    let requirement_changes = |base: &str| -> Option<usize> {
        let dirs = [set.requirements_dir(), set.needs_dir()]
            .map(|d| d.strip_prefix(set.repo_root()).unwrap_or(d).to_path_buf());
        let files = git.changed_files_since(base).ok()?;
        Some(
            files
                .iter()
                .filter(|f| dirs.iter().any(|d| f.starts_with(d)))
                .count(),
        )
    };
    let uncommitted = commit.is_some() && requirement_changes("HEAD").unwrap_or(0) > 0;
    let baseline = git
        .baselines()
        .ok()
        .and_then(|b| b.into_iter().last())
        .and_then(|b| {
            let tag = format!("rqtk/{}", b.name);
            Some(rqtk_report::BaselineInfo {
                changed_since: requirement_changes(&tag)?,
                date: b.timestamp.map(|t| t.date_naive()),
                tag,
            })
        });
    let markdown = rqtk_report::render_report(&rqtk_report::ReportInput {
        set: &set,
        verification: &verification,
        evidence: &evidence,
        links: &links,
        commit: commit.as_deref(),
        uncommitted,
        baseline,
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
