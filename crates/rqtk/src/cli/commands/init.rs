use std::{error::Error, path::PathBuf};

use serde::Serialize;

use crate::cli::output::{self, Ctx, Exit, Usage};

#[derive(Serialize)]
pub struct Report {
    created: Vec<PathBuf>,
    dry_run: bool,
}

pub fn run(
    ctx: &Ctx,
    requirements_dir: Option<&str>,
    force: bool,
    dry_run: bool,
) -> Result<Exit, Box<dyn Error>> {
    let report = execute(ctx, requirements_dir, force, dry_run)?;
    if ctx.json() {
        output::json(&report)?;
    } else {
        print(&report);
    }
    Ok(Exit::Ok)
}

pub fn execute(
    ctx: &Ctx,
    requirements_dir: Option<&str>,
    force: bool,
    dry_run: bool,
) -> Result<Report, Box<dyn Error>> {
    let root = &ctx.root;
    let req_dir = requirements_dir.unwrap_or(".rqtk/requirements");
    let config_path = root.join(".rqtk/config.toml");
    if config_path.exists() && !force {
        return Err(Box::new(Usage(format!(
            "refusing to overwrite {}; pass --force to replace it",
            config_path.display()
        ))));
    }

    let files: Vec<(PathBuf, String)> = vec![
        (config_path, default_config_toml(req_dir)),
        (
            root.join(".rqtk/stakeholders/STK-001.toml"),
            EXAMPLE_STAKEHOLDER_TOML.to_owned(),
        ),
        (
            root.join(".rqtk/needs/NEED-0001.toml"),
            EXAMPLE_NEED_TOML.to_owned(),
        ),
    ];
    let dirs = [root.join(req_dir).join("SYS")];

    if !dry_run {
        for dir in &dirs {
            std::fs::create_dir_all(dir)?;
        }
        for (path, content) in &files {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(path, content)?;
        }
    }

    let created = files
        .iter()
        .map(|(p, _)| p)
        .chain(&dirs)
        .map(|p| output::relative(p, root))
        .collect();
    Ok(Report { created, dry_run })
}

pub fn print(report: &Report) {
    let label = if report.dry_run {
        "Would initialize repository"
    } else {
        "Repository initialized"
    };
    let shown: Vec<String> = report
        .created
        .iter()
        .map(|p| p.display().to_string())
        .collect();
    let pairs: Vec<(&str, &str)> = shown.iter().map(|p| ("create", p.as_str())).collect();
    output::success(label, &pairs);
}

const EXAMPLE_STAKEHOLDER_TOML: &str = r#"id = "STK-001"
name = "Example Stakeholder"
role = "System Engineer"
organization = "Example Org"

[concerns]
primary = ["Functionality", "Reliability"]
secondary = []

[authority]
sign_off_required = false
"#;

const EXAMPLE_NEED_TOML: &str = r#"id = "NEED-0001"
title = "Example Stakeholder Need"
state = "Draft"
stakeholders = ["STK-001"]
statement = "The system shall fulfil this example stakeholder need."
rationale = "Example rationale."
"#;

fn default_config_toml(requirements_dir: &str) -> String {
    format!(
        r#"schema_version = 1

[repository]
requirements_dir = "{requirements_dir}"
stakeholders_dir = ".rqtk/stakeholders"
needs_dir = ".rqtk/needs"
required_files = []
required_dirs = []

[project]
name = "Example Project"
version = "0.1.0"

[identification]
id_pattern = "^REQ-(SYS)-\\d{{4}}$"
id_separator = "-"
prefix = "REQ"
zero_padding = 4

[categories.SYS]
name = "System"
level = 1
is_root = true

[types]
allowed = ["Functional", "Performance", "Interface", "Constraint", "Design"]

[verification]
methods = ["Test", "Analysis", "Inspection", "Demonstration"]
levels = ["Unit", "Integration", "System", "Acceptance"]
phases = ["Development", "Pre-release", "Release"]

[lifecycle]
states = ["Draft", "Review", "Approved", "Implemented", "Verified", "Deprecated"]
default_state = "Draft"

[priority]
levels = ["Critical", "High", "Medium", "Low"]

[criticality]
levels = ["Safety-Critical", "Mission-Critical", "Non-Critical"]

[validation]
require_rationale = true
require_verification_method = true
require_parent_for_levels = []
forbid_orphans = false
forbid_circular_traces = true
allow_tbd = false
allow_tbr = false
shall_keywords = ["shall"]
forbidden_keywords = []
"#
    )
}
