use std::{error::Error, path::Path};

use rqtk_core::io::create_toml_file;
use rqtk_core::{RequirementSet, ScaffoldInput};

use crate::output;

pub struct AddArgs {
    pub category: String,
    pub req_type: String,
    pub title: String,
    pub statement: String,
    pub rationale: Option<String>,
}

pub fn run(repo_root: &Path, args: AddArgs) -> Result<(), Box<dyn Error>> {
    let set = RequirementSet::load_from_repo_root(repo_root)?;
    let cfg = &set.config;
    if !cfg.categories.contains_key(&args.category) {
        let known: Vec<&str> = cfg.categories.keys().map(String::as_str).collect();
        return Err(format!(
            "unknown category `{}`; expected one of: {}",
            args.category,
            known.join(", ")
        )
        .into());
    }
    if !cfg.req_types.allowed.contains(&args.req_type) {
        return Err(format!(
            "unknown type `{}`; expected one of: {}",
            args.req_type,
            cfg.req_types.allowed.join(", ")
        )
        .into());
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
    create_toml_file(&path, &req)?;
    output::success(
        "Requirement created",
        &[
            ("id", req.id.as_ref()),
            ("file", &path.display().to_string()),
        ],
    );
    Ok(())
}
