use std::{error::Error, path::Path};

use rqtk_core::{RequirementSet, StakeholderId, io::create_toml_file};

use crate::output;

pub struct AddStakeholderArgs {
    pub id: Option<String>,
    pub name: String,
    pub role: Option<String>,
    pub organization: Option<String>,
}

pub fn run(repo_root: &Path, args: AddStakeholderArgs) -> Result<(), Box<dyn Error>> {
    let set = RequirementSet::load_from_repo_root(repo_root)?;
    let id = args
        .id
        .map_or_else(|| set.next_stakeholder_id(), StakeholderId);
    let mut stakeholder = set.scaffold_stakeholder(id.clone(), &args.name);
    stakeholder.role = args.role;
    stakeholder.organization = args.organization;

    let path = set.stakeholders_root.join(format!("{id}.toml"));
    create_toml_file(&path, &stakeholder)?;
    output::success(
        "Stakeholder created",
        &[("id", id.as_ref()), ("file", &path.display().to_string())],
    );
    Ok(())
}
