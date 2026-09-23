use std::{error::Error, path::PathBuf};

use rqtk_core::io::create_toml_file;
use rqtk_core::{RequirementSet, ScaffoldInput};
use serde::Serialize;

use crate::output::{self, Ctx, Exit, Usage};

pub struct AddArgs {
    pub category: String,
    pub req_type: String,
    pub title: String,
    pub statement: String,
    pub rationale: Option<String>,
    pub dry_run: bool,
}

/// JSON shape shared by the `add*` commands.
#[derive(Serialize)]
pub struct Created<'a, T: Serialize> {
    pub id: &'a str,
    pub path: PathBuf,
    pub dry_run: bool,
    pub item: &'a T,
}

/// Write `item` to `path` (unless dry-running) and report it.
pub fn finish<T: Serialize>(
    ctx: &Ctx,
    kind: &str,
    id: &str,
    path: PathBuf,
    item: &T,
    dry_run: bool,
) -> Result<Exit, Box<dyn Error>> {
    if !dry_run {
        create_toml_file(&path, item)?;
    }
    let path = output::relative(&path, &ctx.root);
    if ctx.json() {
        output::json(&Created {
            id,
            path,
            dry_run,
            item,
        })?;
    } else if dry_run {
        println!(
            "# would create {}\n{}",
            path.display(),
            toml::to_string_pretty(item)?
        );
    } else {
        output::success(
            &format!("{kind} created"),
            &[("id", id), ("file", &path.display().to_string())],
        );
    }
    Ok(Exit::Ok)
}

pub fn run(ctx: &Ctx, args: AddArgs) -> Result<Exit, Box<dyn Error>> {
    let set = RequirementSet::load_from_repo_root(&ctx.root)?;
    let cfg = &set.config;
    if !cfg.categories.contains_key(&args.category) {
        let known: Vec<&str> = cfg.categories.keys().map(String::as_str).collect();
        return Err(Box::new(Usage(format!(
            "unknown category `{}`; expected one of: {}",
            args.category,
            known.join(", ")
        ))));
    }
    if !cfg.req_types.allowed.contains(&args.req_type) {
        return Err(Box::new(Usage(format!(
            "unknown type `{}`; expected one of: {}",
            args.req_type,
            cfg.req_types.allowed.join(", ")
        ))));
    }

    let req = set.scaffold_requirement(ScaffoldInput {
        category: &args.category,
        req_type: &args.req_type,
        title: &args.title,
        statement: &args.statement,
        rationale: args.rationale.as_deref(),
    });
    let path = set
        .category_dir(&args.category)
        .join(format!("{}.toml", req.id));
    finish(ctx, "Requirement", &req.id.0, path, &req, args.dry_run)
}
