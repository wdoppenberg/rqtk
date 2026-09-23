use std::error::Error;

use rqtk_core::{Diagnostic, RequirementSet, Severity};
use serde::Serialize;

use crate::cli::output::{self, Ctx, Exit};

#[derive(Serialize)]
struct Report<'a> {
    errors: usize,
    warnings: usize,
    checked: usize,
    diagnostics: &'a [Diagnostic],
}

pub fn run(ctx: &Ctx) -> Result<Exit, Box<dyn Error>> {
    let set = RequirementSet::load_from_repo_root(&ctx.root)?;
    let checked = set.requirements().len() + set.needs().len() + set.stakeholders().len();
    let (set, mut issues) = set.validate();
    let links = rqtk_core::scan::scan(set.repo_root(), &set.config().scan)?;
    issues.extend(set.link_diagnostics(&links));
    for issue in &mut issues {
        if let Some(loc) = issue.location.as_mut() {
            loc.path = output::relative(&loc.path, &ctx.root);
        }
    }

    let errors = issues
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .count();
    if ctx.json() {
        output::json(&Report {
            errors,
            warnings: issues.len() - errors,
            checked,
            diagnostics: &issues,
        })?;
    } else if issues.is_empty() {
        output::success(
            &format!("All requirements passed lint ({checked} checked)"),
            &[],
        );
    } else {
        let (errors, warnings) = output::lint_table(&issues, &ctx.root);
        output::lint_summary(errors, warnings, checked);
    }
    Ok(Exit::findings_if(errors > 0))
}
