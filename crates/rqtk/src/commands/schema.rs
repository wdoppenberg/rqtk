use std::error::Error;

use console::style;
use rqtk_core::schema::{KINDS, json_schema};

use crate::output::{self, Ctx, Exit, Usage};

/// Print the JSON Schema for a file kind. Without a kind, list the kinds.
/// The schema is JSON either way; `--json` only changes the listing.
pub fn run(ctx: &Ctx, kind: Option<&str>) -> Result<Exit, Box<dyn Error>> {
    let Some(kind) = kind else {
        if ctx.json() {
            let kinds: Vec<_> = KINDS
                .iter()
                .map(|(kind, file)| serde_json::json!({ "kind": kind, "file": file }))
                .collect();
            output::json(&kinds)?;
        } else {
            for (kind, file) in KINDS {
                println!("  {:<12} {}", style(kind).bold(), style(file).dim());
            }
        }
        return Ok(Exit::Ok);
    };
    let schema = json_schema(kind).ok_or_else(|| {
        let known: Vec<&str> = KINDS.iter().map(|(k, _)| *k).collect();
        Usage(format!(
            "unknown schema kind `{kind}`; expected one of: {}",
            known.join(", ")
        ))
    })?;
    output::json(&schema)?;
    Ok(Exit::Ok)
}
