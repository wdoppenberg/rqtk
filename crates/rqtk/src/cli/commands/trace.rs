use crate::cli::error::CliError;
use crate::cli::output::{self, Ctx, Exit};

pub fn run(ctx: &Ctx, id: String) -> Result<Exit, CliError> {
    let repo = super::open(ctx)?;
    let doc = repo.trace(&id)?;
    if ctx.json() {
        output::json(&doc)?;
        return Ok(Exit::Ok);
    }
    let set = repo.requirement_set();
    let req_id = &doc.id;
    let consumes = &doc.consumes;
    let title = |id: &str| {
        set.requirements()
            .get(&rqtk_core::RequirementId(id.to_owned()))
            .map_or_else(|| id.to_owned(), |r| format!("{id}  {}", r.title))
    };
    output::section("Traceability", &title(req_id));
    if doc.ancestors.is_empty() {
        output::subsection("▲", "Parents: none, a top-level requirement");
    } else {
        output::subsection("▲", &format!("Parents ({})", doc.ancestors.len()));
        for id in &doc.ancestors {
            output::item(&title(id));
        }
    }
    if doc.descendants.is_empty() {
        output::subsection("▼", "Children: none, no requirement derives from it");
    } else {
        output::subsection("▼", &format!("Children ({})", doc.descendants.len()));
        for id in &doc.descendants {
            output::item(&title(id));
        }
    }
    if !consumes.is_empty() {
        output::subsection("◆", &format!("Consumes ({})", consumes.len()));
        for c in consumes {
            output::item(&output::consumed_line(c));
        }
    }
    println!();
    Ok(Exit::Ok)
}
