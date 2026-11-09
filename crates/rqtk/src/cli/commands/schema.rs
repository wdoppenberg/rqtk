use console::style;

use crate::cli::documents::{self, SchemaKinds};
use crate::cli::error::CliError;
use crate::cli::output::{self, Ctx, Exit, Usage};

/// Print the JSON Schema of a file kind, a `--json` document or a client type. Without a
/// kind, list them. The schema is JSON either way; `--json` only changes the listing.
pub fn run(ctx: &Ctx, kind: Option<&str>) -> Result<Exit, CliError> {
    let all = documents::all();
    let Some(kind) = kind else {
        if ctx.json() {
            output::document(
                "schema-kinds",
                &SchemaKinds {
                    kinds: all.into_iter().map(|(k, _)| k).collect(),
                },
            )?;
        } else {
            for (k, _) in &all {
                println!(
                    "  {:<20} {:<9} {}",
                    style(&k.kind).bold(),
                    serde_json::to_value(k.category)?
                        .as_str()
                        .unwrap_or_default(),
                    style(&k.describes).dim()
                );
            }
        }
        return Ok(Exit::Ok);
    };
    let Some((_, schema)) = all.into_iter().find(|(k, _)| k.kind == kind) else {
        return Err(Usage(format!(
            "unknown schema kind `{kind}`; run `rqtk schema` to list them"
        ))
        .into());
    };
    output::json(&schema)?;
    Ok(Exit::Ok)
}
