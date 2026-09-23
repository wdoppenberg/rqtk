use std::{error::Error, path::Path};

use dialoguer::{Input, theme::ColorfulTheme};
use rqtk_core::{RequirementSet, RqtkError, io::write_stakeholder_file};

use crate::output;

pub struct AddStakeholderArgs {
    pub id: Option<String>,
    pub name: Option<String>,
    pub role: Option<String>,
    pub organization: Option<String>,
}

pub fn run(repo_root: &Path, args: AddStakeholderArgs) -> Result<(), Box<dyn Error>> {
    let set = RequirementSet::load_from_repo_root(repo_root)?;
    let theme = ColorfulTheme::default();

    let id = match args.id {
        Some(id) => id,
        None => {
            let auto = set.next_stakeholder_id();
            Input::with_theme(&theme)
                .with_prompt("ID")
                .default(auto)
                .interact_text()?
        }
    };

    let name = match args.name {
        Some(n) => n,
        None => Input::with_theme(&theme)
            .with_prompt("Name")
            .interact_text()?,
    };

    let role = match args.role {
        Some(r) => Some(r),
        None => {
            let r: String = Input::with_theme(&theme)
                .with_prompt("Role (empty to skip)")
                .allow_empty(true)
                .interact_text()?;
            if r.is_empty() { None } else { Some(r) }
        }
    };

    let organization = match args.organization {
        Some(o) => Some(o),
        None => {
            let o: String = Input::with_theme(&theme)
                .with_prompt("Organization (empty to skip)")
                .allow_empty(true)
                .interact_text()?;
            if o.is_empty() { None } else { Some(o) }
        }
    };

    let mut file = set.scaffold_stakeholder(&id, &name);
    file.stakeholder.role = role;
    file.stakeholder.organization = organization;

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
