use std::{error::Error, path::Path};

use rqtk_core::io::write_requirement_file;
use rqtk_core::{RequirementSet, RqtkError, ScaffoldInput};

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
    let file = set.scaffold_requirement(ScaffoldInput {
        category: &args.category,
        req_type: &args.req_type,
        title: &args.title,
        statement: &args.statement,
        rationale: args.rationale.as_deref(),
    });
    let id = file.requirement.id.clone();
    let category_dir = set.category_dir(&args.category);
    std::fs::create_dir_all(&category_dir).map_err(|e| RqtkError::Io {
        path: category_dir.clone(),
        source: e,
    })?;
    let path = category_dir.join(format!("{id}.toml"));
    write_requirement_file(&path, &file)?;
    output::success(
        "Requirement created",
        &[
            ("id", &id.to_string()),
            ("file", &path.display().to_string()),
        ],
    );
    Ok(())
}
