use std::{error::Error, path::Path};

use rqtk_core::{NeedId, RequirementSet, StakeholderId, io::create_toml_file};

use crate::output;

pub struct AddNeedArgs {
    pub id: Option<String>,
    pub title: String,
    pub statement: String,
    pub stakeholders: Option<Vec<String>>,
}

pub fn run(repo_root: &Path, args: AddNeedArgs) -> Result<(), Box<dyn Error>> {
    let set = RequirementSet::load_from_repo_root(repo_root)?;
    let id = args.id.map_or_else(|| set.next_need_id(), NeedId);
    let mut need = set.scaffold_need(id.clone(), &args.title, &args.statement);
    need.stakeholders = args
        .stakeholders
        .unwrap_or_default()
        .into_iter()
        .map(StakeholderId)
        .collect();
    need.content_hash = Some(need.compute_content_hash());

    let path = set.needs_root.join(format!("{id}.toml"));
    create_toml_file(&path, &need)?;
    output::success(
        "Need created",
        &[("id", id.as_ref()), ("file", &path.display().to_string())],
    );
    Ok(())
}
