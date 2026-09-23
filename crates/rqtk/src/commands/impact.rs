use std::error::Error;

use rqtk_core::RequirementSet;
use rqtk_core::query::{ChangeKind, ItemChange, ReverifyReason};

use crate::output::{self, Ctx, Exit};

/// Informational: always exits 0 unless the comparison itself fails.
pub fn run(ctx: &Ctx, base: &str) -> Result<Exit, Box<dyn Error>> {
    let (set, _) = RequirementSet::load_from_repo_root(&ctx.root)?.validate();
    let links = rqtk_core::scan::scan(&set.repo_root, &set.config.scan)?;
    let changed_files = set.git.changed_files_since(base)?;
    let impact = set.impact(base, &links, &changed_files)?;

    if ctx.json() {
        output::json(&impact)?;
        return Ok(Exit::Ok);
    }
    if impact.is_empty() {
        output::success(&format!("No requirement changes since {base}"), &[]);
        return Ok(Exit::Ok);
    }
    output::section("Impact", &format!("since {base}"));
    changes("Requirements", &impact.requirements);
    changes("Needs", &impact.needs);
    if !impact.downstream.is_empty() {
        output::section("Downstream", "(depend on a changed item; review them)");
        for d in &impact.downstream {
            output::item(&format!("{}  via {}", d.id, d.via.join(", ")));
        }
    }
    if !impact.reverify.is_empty() {
        output::section("Re-verify", "(run these tests, then `rqtk verify`)");
        for r in &impact.reverify {
            let reasons: Vec<String> = r
                .reasons
                .iter()
                .map(|reason| match reason {
                    ReverifyReason::RequirementChanged => "requirement changed".to_owned(),
                    ReverifyReason::TestChanged { path } => {
                        format!("test changed: {}", path.display())
                    }
                })
                .collect();
            output::item(&format!(
                "{}  {}  {}",
                r.activity,
                r.requirement,
                reasons.join("; ")
            ));
        }
    }
    println!();
    Ok(Exit::Ok)
}

fn changes(title: &str, items: &[ItemChange]) {
    if items.is_empty() {
        return;
    }
    output::section(title, "");
    for c in items {
        let (icon, label) = match c.change {
            ChangeKind::Added => ("+", "added"),
            ChangeKind::Removed => ("-", "removed"),
            ChangeKind::Semantic => ("~", "semantic"),
            ChangeKind::Cosmetic => ("~", "cosmetic"),
        };
        output::item(&format!("{icon} {}  {label}", c.id));
    }
}
