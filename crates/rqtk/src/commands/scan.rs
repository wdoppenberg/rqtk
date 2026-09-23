use std::{collections::BTreeMap, error::Error};

use console::style;
use rqtk_core::{Diagnostic, RequirementSet, SourceLink, scan};
use serde::Serialize;

use crate::output::{self, Ctx, Exit};

#[derive(Serialize)]
struct Report<'a> {
    links: &'a [SourceLink],
    unlinked_activities: Vec<&'a str>,
    diagnostics: &'a [Diagnostic],
}

/// List every `verifies` link in source, grouped by activity, then any link problems.
pub fn run(ctx: &Ctx) -> Result<Exit, Box<dyn Error>> {
    let (set, _) = RequirementSet::load_from_repo_root(&ctx.root)?.validate();
    let links = scan::scan(&set.repo_root, &set.config.scan)?;
    let mut by_activity: BTreeMap<&str, Vec<&SourceLink>> = BTreeMap::new();
    for link in &links {
        by_activity.entry(&link.activity).or_default().push(link);
    }
    let unlinked: Vec<&str> = set
        .requirements
        .values()
        .flat_map(|r| &r.verification.activities)
        .map(|a| a.id.as_str())
        .filter(|id| !by_activity.contains_key(id))
        .collect();
    let mut issues = set.link_diagnostics(&links);
    for issue in &mut issues {
        if let Some(loc) = issue.location.as_mut() {
            loc.path = output::relative(&loc.path, &ctx.root);
        }
    }
    let exit = Exit::findings_if(issues.iter().any(Diagnostic::is_error));

    if ctx.json() {
        output::json(&Report {
            links: &links,
            unlinked_activities: unlinked,
            diagnostics: &issues,
        })?;
        return Ok(exit);
    }

    for (activity, links) in &by_activity {
        println!("\n  {}", style(activity).bold());
        for link in links {
            println!(
                "     {} {}:{}  {}",
                style("·").dim(),
                link.path.display(),
                link.line,
                link.test_name.as_deref().map_or_else(
                    || style("(no function)".to_owned()).yellow().to_string(),
                    |t| style(t.to_owned()).cyan().to_string()
                ),
            );
        }
    }
    println!(
        "\n  {} links to {} activities  ·  {} activities not linked to tests",
        style(links.len()).bold(),
        style(by_activity.len()).bold(),
        style(unlinked.len()).bold(),
    );
    if !issues.is_empty() {
        println!();
        output::lint_table(&issues, &ctx.root);
    }
    Ok(exit)
}
