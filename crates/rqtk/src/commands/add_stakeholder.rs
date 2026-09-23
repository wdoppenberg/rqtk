use std::{error::Error, path::Path};

use rqtk_core::{RequirementSet, RqtkError, io::write_stakeholder_file};

use crate::output;

pub struct AddStakeholderArgs {
    pub id: Option<String>,
    pub name: String,
    pub role: Option<String>,
    pub organization: Option<String>,
}

pub fn run(repo_root: &Path, args: AddStakeholderArgs) -> Result<(), Box<dyn Error>> {
    let set = RequirementSet::load_from_repo_root(repo_root)?;
    let id = args.id.unwrap_or_else(|| set.next_stakeholder_id());
    let mut file = set.scaffold_stakeholder(&id, &args.name);
    file.stakeholder.role = args.role;
    file.stakeholder.organization = args.organization;

    std::fs::create_dir_all(&set.stakeholders_root).map_err(|e| RqtkError::Io {
        path: set.stakeholders_root.clone(),
        source: e,
    })?;
    let path = set.stakeholders_root.join(format!("{id}.toml"));
    write_stakeholder_file(&path, &file)?;
    output::success(
        "Stakeholder created",
        &[("id", &id), ("file", &path.display().to_string())],
    );
    Ok(())
}
