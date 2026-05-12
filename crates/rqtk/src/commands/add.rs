use std::{error::Error, path::Path};

use dialoguer::{Input, Select, theme::ColorfulTheme};
use rqtk_core::io::write_requirement_file;
use rqtk_core::{RequirementSet, RqtkError, ScaffoldInput};

use crate::output;

pub struct AddArgs {
    pub category: Option<String>,
    pub req_type: Option<String>,
    pub title: Option<String>,
    pub statement: Option<String>,
    pub rationale: Option<String>,
}

pub fn run(repo_root: &Path, args: AddArgs) -> Result<(), Box<dyn Error>> {
    let set = RequirementSet::load_from_repo_root(repo_root)?;
    let theme = ColorfulTheme::default();

    let category = match args.category {
        Some(c) => c,
        None => {
            let categories: Vec<&String> = set.config.categories.keys().collect();
            let idx = Select::with_theme(&theme)
                .with_prompt("Category")
                .items(&categories)
                .default(0)
                .interact()?;
            categories[idx].clone()
        }
    };

    let req_type = match args.req_type {
        Some(t) => t,
        None => {
            let types = &set.config.req_types.allowed;
            let idx = Select::with_theme(&theme)
                .with_prompt("Type")
                .items(types)
                .default(0)
                .interact()?;
            types[idx].clone()
        }
    };

    let title = match args.title {
        Some(t) => t,
        None => Input::with_theme(&theme)
            .with_prompt("Title (brief noun phrase, e.g. \"Telemetry downlink rate\")")
            .interact_text()?,
    };

    let statement = match args.statement {
        Some(s) => s,
        None => Input::with_theme(&theme)
            .with_prompt("Statement (single shall-sentence, e.g. \"The system shall …\")")
            .interact_text()?,
    };

    let rationale = match args.rationale {
        Some(r) => Some(r),
        None => {
            let r: String = Input::with_theme(&theme)
                .with_prompt("Rationale (why this requirement exists; empty to skip)")
                .allow_empty(true)
                .interact_text()?;
            if r.is_empty() { None } else { Some(r) }
        }
    };

    let file = set.scaffold_requirement(ScaffoldInput {
        category: &category,
        req_type: &req_type,
        title: &title,
        statement: &statement,
        rationale: rationale.as_deref(),
    });
    let id = file.requirement.id.clone();
    let category_dir = set.category_dir(&category);
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
