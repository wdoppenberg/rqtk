use std::{error::Error, path::PathBuf};

use serde::Serialize;

use crate::cli::output::{self, Ctx, Exit, Usage};

#[derive(Serialize)]
pub struct Report {
    created: Vec<PathBuf>,
    dry_run: bool,
    /// The project name written to the config, taken from the project manifest or the
    /// directory name.
    project: String,
    warnings: Vec<String>,
}

#[derive(Default)]
pub struct InitArgs<'a> {
    pub requirements_dir: Option<&'a str>,
    pub force: bool,
    /// Also create an example stakeholder and need.
    pub example: bool,
    pub dry_run: bool,
}

pub fn run(ctx: &Ctx, args: &InitArgs<'_>) -> Result<Exit, Box<dyn Error>> {
    let report = execute(ctx, args)?;
    if ctx.json() {
        output::json(&report)?;
    } else {
        print(&report);
    }
    Ok(Exit::Ok)
}

pub fn execute(ctx: &Ctx, args: &InitArgs<'_>) -> Result<Report, Box<dyn Error>> {
    let root = &ctx.root;
    let dry_run = args.dry_run;
    let req_dir = args.requirements_dir.unwrap_or(".rqtk/requirements");
    let config_path = root.join(".rqtk/config.toml");
    if config_path.exists() && !args.force {
        return Err(Box::new(Usage(format!(
            "refusing to overwrite {}; pass --force to replace it",
            config_path.display()
        ))));
    }

    let project = project_name(root);
    let mut files: Vec<(PathBuf, String)> =
        vec![(config_path, default_config_toml(req_dir, &project))];
    if args.example {
        files.push((
            root.join(".rqtk/stakeholders/STK-001.toml"),
            EXAMPLE_STAKEHOLDER_TOML.to_owned(),
        ));
        files.push((
            root.join(".rqtk/needs/NEED-0001.toml"),
            EXAMPLE_NEED_TOML.to_owned(),
        ));
    }
    let mut warnings = Vec::new();
    if !root.ancestors().any(|dir| dir.join(".git").exists()) {
        warnings.push(
            "not inside a git repository: `verify`, `impact`, `diff` and `baseline` need git history"
                .to_owned(),
        );
    }
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
    Ok(Report {
        created,
        dry_run,
        project,
        warnings,
    })
}

/// The project's name from its manifest (Cargo.toml, pyproject.toml, package.json, go.mod),
/// or else the directory name.
fn project_name(root: &std::path::Path) -> String {
    let read = |file: &str| std::fs::read_to_string(root.join(file)).ok();
    let from_toml = |file: &str, keys: &[&str]| -> Option<String> {
        let value: toml::Value = toml::from_str(&read(file)?).ok()?;
        let mut v = &value;
        for key in keys {
            v = v.get(key)?;
        }
        v.as_str().map(str::to_owned)
    };
    from_toml("Cargo.toml", &["package", "name"])
        .or_else(|| from_toml("pyproject.toml", &["project", "name"]))
        .or_else(|| from_toml("pyproject.toml", &["tool", "poetry", "name"]))
        .or_else(|| {
            let json: serde_json::Value = serde_json::from_str(&read("package.json")?).ok()?;
            json.get("name")?.as_str().map(str::to_owned)
        })
        .or_else(|| {
            let module = read("go.mod")?
                .lines()
                .find_map(|l| l.strip_prefix("module "))?
                .trim()
                .to_owned();
            Some(module.rsplit('/').next().unwrap_or(&module).to_owned())
        })
        .or_else(|| {
            root.canonicalize()
                .ok()?
                .file_name()?
                .to_str()
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "Project".to_owned())
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
    let mut pairs: Vec<(&str, &str)> = vec![("project", report.project.as_str())];
    pairs.extend(shown.iter().map(|p| ("create", p.as_str())));
    output::success(label, &pairs);
    for warning in &report.warnings {
        output::warning(warning);
    }
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

fn default_config_toml(requirements_dir: &str, project: &str) -> String {
    let project = project.replace('\\', "\\\\").replace('"', "\\\"");
    format!(
        r#"schema_version = 1

[repository]
requirements_dir = "{requirements_dir}"
stakeholders_dir = ".rqtk/stakeholders"
needs_dir = ".rqtk/needs"
required_files = []
required_dirs = []

[project]
name = "{project}"
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
require_parent_for_categories = []
forbid_orphans = false
forbid_circular_traces = true
allow_tbd = false
allow_tbr = false
shall_keywords = ["shall"]
forbidden_keywords = []
"#
    )
}
