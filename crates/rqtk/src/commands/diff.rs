use std::{error::Error, path::Path};

use rqtk_core::{ChangeKind, RequirementSet};

use crate::output;

pub fn run(repo_root: &Path, from: String, to: String) -> Result<(), Box<dyn Error>> {
    let set = RequirementSet::load_from_repo_root(repo_root)?;
    let diff = set.git.diff_refs(&from, &to, &set.root)?;

    let total = diff.added.len() + diff.removed.len() + diff.modified.len();
    if total == 0 {
        output::success(&format!("No changes between {from} and {to}"), &[]);
        return Ok(());
    }

    output::section(
        "Diff",
        &format!(
            "{} → {}  ({} added, {} removed, {} modified)",
            diff.from,
            diff.to,
            diff.added.len(),
            diff.removed.len(),
            diff.modified.len()
        ),
    );

    if !diff.added.is_empty() {
        output::subsection("+", "Added");
        for id in &diff.added {
            output::item(id.as_ref());
        }
    }
    if !diff.removed.is_empty() {
        output::subsection("-", "Removed");
        for id in &diff.removed {
            output::item(id.as_ref());
        }
    }
    if !diff.modified.is_empty() {
        output::subsection("~", "Modified");
        for m in &diff.modified {
            let tag = match m.change_kind {
                ChangeKind::Semantic => "[semantic]",
                ChangeKind::Cosmetic => "[admin]",
                _ => "[unknown]",
            };
            output::item(&format!("{}  {}", m.id, tag));
        }
    }
    println!();
    Ok(())
}
