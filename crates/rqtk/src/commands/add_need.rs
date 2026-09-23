use std::{error::Error, path::Path};

use dialoguer::{Input, theme::ColorfulTheme};
use rqtk_core::{NeedId, RequirementSet, RqtkError, io::write_need_file};

use crate::output;

pub struct AddNeedArgs {
    pub id: Option<String>,
    pub title: Option<String>,
    pub statement: Option<String>,
    pub stakeholders: Option<Vec<String>>,
}

pub fn run(repo_root: &Path, args: AddNeedArgs) -> Result<(), Box<dyn Error>> {
    let set = RequirementSet::load_from_repo_root(repo_root)?;
    let theme = ColorfulTheme::default();

    let id = match args.id {
        Some(id) => NeedId(id),
        None => {
            let auto = set.next_need_id();
            let s: String = Input::with_theme(&theme)
                .with_prompt("ID")
                .default(auto.to_string())
                .interact_text()?;
            NeedId(s)
        }
    };

    let title = match args.title {
        Some(t) => t,
        None => Input::with_theme(&theme)
            .with_prompt("Title")
            .interact_text()?,
    };

    let statement = match args.statement {
        Some(s) => s,
        None => Input::with_theme(&theme)
            .with_prompt("Statement")
            .interact_text()?,
    };

    let mut file = set.scaffold_need(id.clone(), &title, &statement);
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
        &[("id", &id.to_string()), ("file", &path.display().to_string())],
    );
    Ok(())
}
