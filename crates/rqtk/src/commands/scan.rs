use std::{collections::BTreeMap, error::Error, path::Path};

use console::style;
use rqtk_core::{RequirementSet, SourceLink, scan};

use crate::output;

/// List every `verifies` link in source, grouped by activity, then any link problems.
pub fn run(repo_root: &Path) -> Result<(), Box<dyn Error>> {
    let (set, _) = RequirementSet::load_from_repo_root(repo_root)?.validate();
    let links = scan::scan(&set.repo_root, &set.config.scan)?;

    let mut by_activity: BTreeMap<&str, Vec<&SourceLink>> = BTreeMap::new();
    for link in &links {
        by_activity.entry(&link.activity).or_default().push(link);
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

    let unlinked: Vec<&str> = set
        .requirements
        .values()
        .flat_map(|r| &r.verification.activities)
        .map(|a| a.id.as_str())
        .filter(|id| !by_activity.contains_key(id))
        .collect();
    println!(
        "\n  {} links to {} activities  ·  {} activities not linked to tests",
        style(links.len()).bold(),
        style(by_activity.len()).bold(),
        style(unlinked.len()).bold(),
    );

    let issues = set.link_diagnostics(&links);
    if !issues.is_empty() {
        println!();
        let (errors, _) = output::lint_table(&issues, repo_root);
        if errors > 0 {
            std::process::exit(2);
        }
    }
    Ok(())
}
