use std::{error::Error, path::Path};

use rqtk_core::{NeedId, RequirementSet, RqtkError, io::write_need_file};

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
    let mut file = set.scaffold_need(id.clone(), &args.title, &args.statement);
    if let Some(stakeholders) = args.stakeholders {
        file.need.stakeholders = stakeholders;
    }

    std::fs::create_dir_all(&set.needs_root).map_err(|e| RqtkError::Io {
        path: set.needs_root.clone(),
        source: e,
    })?;
    let path = set.needs_root.join(format!("{id}.toml"));
    write_need_file(&path, &file)?;
    output::success(
        "Need created",
        &[
            ("id", &id.to_string()),
            ("file", &path.display().to_string()),
        ],
    );
    Ok(())
}
