use crate::cli::error::CliError;
use crate::cli::output::{self, Ctx, Exit};

pub fn run(ctx: &Ctx) -> Result<Exit, CliError> {
    let repo = super::open(ctx)?;
    let doc = repo.lint();
    if ctx.json() {
        output::json(&doc)?;
    } else if doc.diagnostics.is_empty() {
        let set = repo.requirement_set();
        let what = [
            (set.requirements().len(), "requirement"),
            (set.needs().len(), "need"),
            (set.stakeholders().len(), "stakeholder"),
        ]
        .iter()
        .map(|(n, noun)| format!("{n} {noun}{}", if *n == 1 { "" } else { "s" }))
        .collect::<Vec<_>>()
        .join(", ");
        output::success(&format!("No lint findings ({what})"), &[]);
    } else {
        let (errors, warnings) = output::lint_table(&doc.diagnostics, std::path::Path::new(""));
        output::lint_summary(errors, warnings, doc.checked);
    }
    Ok(Exit::findings_if(doc.errors > 0))
}
