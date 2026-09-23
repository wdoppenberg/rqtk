use std::error::Error;

use rqtk_core::{ChangeKind, RequirementId, RequirementSet};
use serde::Serialize;

use crate::output::{self, Ctx, Exit};

#[derive(Serialize)]
struct Modified<'a> {
    id: &'a RequirementId,
    change: &'static str,
}

#[derive(Serialize)]
struct Report<'a> {
    from: &'a str,
    to: &'a str,
    added: &'a [RequirementId],
    removed: &'a [RequirementId],
    modified: Vec<Modified<'a>>,
}

pub fn run(ctx: &Ctx, from: String, to: String) -> Result<Exit, Box<dyn Error>> {
    let set = RequirementSet::load_from_repo_root(&ctx.root)?;
    let diff = set.git().diff_refs(&from, &to, set.requirements_dir())?;
    let modified: Vec<Modified> = diff
        .modified
        .iter()
        .map(|m| Modified {
            id: &m.id,
            change: match m.change_kind {
                ChangeKind::Semantic => "semantic",
                _ => "cosmetic",
            },
        })
        .collect();

    if ctx.json() {
        output::json(&Report {
            from: &from,
            to: &to,
            added: &diff.added,
            removed: &diff.removed,
            modified,
        })?;
        return Ok(Exit::Ok);
    }

    let total = diff.added.len() + diff.removed.len() + modified.len();
    if total == 0 {
        output::success(&format!("No changes between {from} and {to}"), &[]);
        return Ok(Exit::Ok);
    }
    output::section(
        "Diff",
        &format!(
            "{} → {}  ({} added, {} removed, {} modified)",
            diff.from,
            diff.to,
            diff.added.len(),
            diff.removed.len(),
            modified.len()
        ),
    );
    for (icon, title, ids) in [("+", "Added", &diff.added), ("-", "Removed", &diff.removed)] {
        if !ids.is_empty() {
            output::subsection(icon, title);
            for id in ids {
                output::item(id.as_ref());
            }
        }
    }
    if !modified.is_empty() {
        output::subsection("~", "Modified");
        for m in &modified {
            let tag = if m.change == "semantic" {
                "[semantic]"
            } else {
                "[admin]"
            };
            output::item(&format!("{}  {}", m.id, tag));
        }
    }
    println!();
    Ok(Exit::Ok)
}
