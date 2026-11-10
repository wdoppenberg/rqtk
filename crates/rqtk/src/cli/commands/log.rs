use crate::cli::error::CliError;
use crate::cli::output::{self, Ctx, Exit};

pub fn run(ctx: &Ctx, id: String) -> Result<Exit, CliError> {
    let repo = super::open(ctx)?;
    let doc = repo.history(&id)?;
    if ctx.json() {
        output::json(&doc)?;
        return Ok(Exit::Ok);
    }
    let commits = &doc.commits;
    let req_id = &doc.id;

    if commits.is_empty() {
        output::success("No committed history found", &[("id", req_id.as_str())]);
        return Ok(Exit::Ok);
    }
    output::section(req_id, &format!("{} commits", commits.len()));
    for c in commits {
        let date = c
            .timestamp
            .as_deref()
            .and_then(|t| t.get(..10))
            .unwrap_or("unknown")
            .to_owned();
        println!(
            "  {} {}  {}  {}",
            console::style(&c.hash[..c.hash.len().min(8)]).yellow(),
            console::style(date).dim(),
            console::style(&c.author).cyan(),
            c.message.trim_end()
        );
    }
    println!();
    Ok(Exit::Ok)
}
