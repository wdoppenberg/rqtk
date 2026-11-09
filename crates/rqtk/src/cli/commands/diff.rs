use std::error::Error;

use rqtk_client::views::Modification;

use crate::cli::output::{self, Ctx, Exit};

pub fn run(ctx: &Ctx, from: String, to: String) -> Result<Exit, Box<dyn Error>> {
    let repo = super::open(ctx)?;
    let diff = repo.diff(&from, &to)?;
    if ctx.json() {
        output::json(&diff)?;
        return Ok(Exit::Ok);
    }
    let modified = &diff.modified;

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
                output::item(id);
            }
        }
    }
    if !modified.is_empty() {
        output::subsection("~", "Modified");
        for m in modified {
            let tag = if m.change == Modification::Semantic {
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
