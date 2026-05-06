use clap::{Parser, Subcommand};
use rqtk::{bump_baseline_version, format_lint, load_requirements, write_requirement_file};
use rqtk_core::{RequirementId, RequirementSet, RqtkError, ScaffoldInput, Validated};
use semver::Version;
use std::error::Error;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "rqtk", version, about = "Requirements Toolkit")]
struct Cli {
    #[arg(long, default_value = ".")]
    repo_root: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Init {
        #[arg(long)]
        requirements_dir: Option<String>,
        #[arg(long)]
        force: bool,
    },
    New {
        #[arg(long)]
        category: String,
        #[arg(long = "type")]
        req_type: String,
        #[arg(long)]
        title: String,
        #[arg(long)]
        statement: String,
        #[arg(long)]
        rationale: Option<String>,
    },
    Lint,
    Trace {
        id: String,
    },
    Coverage,
    Graph {
        #[arg(long, default_value = "dot")]
        format: String,
    },
    Baseline {
        version: String,
    },
    Export {
        #[arg(long)]
        format: String,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    Diff {
        from: String,
        to: String,
    },
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    match cli.command {
        Command::Init {
            requirements_dir,
            force,
        } => {
            scaffold_repository(&cli.repo_root, requirements_dir.as_deref(), force)?;
            println!("initialized {}", cli.repo_root.display());
        }
        Command::New {
            category,
            req_type,
            title,
            statement,
            rationale,
        } => {
            let set = load_requirements(&cli.repo_root)?;
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
            println!("created {}", path.display());
        }
        Command::Lint => {
            let set = load_requirements(&cli.repo_root)?;
            let (_, issues) = set.validate();
            let (errors, warnings, lines) = format_lint(&issues);
            for line in lines {
                println!("{line}");
            }
            println!("lint summary: {errors} error(s), {warnings} warning(s)");
            if errors > 0 {
                std::process::exit(2);
            }
        }
        Command::Trace { id } => {
            let set = load_requirements(&cli.repo_root)?;
            let view = set.trace_view(&RequirementId(id))?;
            println!("upward:");
            for req_id in view.upward {
                println!("  {req_id}");
            }
            println!("downward:");
            for req_id in view.downward {
                println!("  {req_id}");
            }
        }
        Command::Coverage => {
            let set = load_requirements(&cli.repo_root)?;
            let (set, _) = set.validate();
            let gaps = set.coverage_gaps();
            if gaps.is_empty() {
                println!("all requirements have verification success criteria and activities");
            } else {
                println!("requirements with verification coverage gaps:");
                for id in gaps {
                    println!("  {id}");
                }
                std::process::exit(3);
            }
        }
        Command::Graph { format } => {
            let set = load_requirements(&cli.repo_root)?;
            if format != "dot" {
                return Err(Box::new(RqtkError::UnsupportedExportFormat(format)));
            }
            let (set, _) = set.validate();
            println!("{}", set.to_dot());
        }
        Command::Baseline { version } => {
            let mut set = load_requirements(&cli.repo_root)?;
            let parsed = Version::parse(&version)?;
            bump_baseline_version(&mut set, &parsed)?;
            println!("updated baseline version to {parsed}");
        }
        Command::Export { format, output } => {
            let set = load_requirements(&cli.repo_root)?;
            let (set, _) = set.validate();
            let out = output.unwrap_or_else(|| default_export_path(&set.root, &format));
            export_set(&set, &format, &out)?;
            println!("exported {}", out.display());
        }
        Command::Diff { from, to } => {
            let set = load_requirements(&cli.repo_root)?;
            let mut changed = Vec::new();
            for (id, req) in &set.requirements {
                let has_from = req
                    .requirement
                    .history
                    .iter()
                    .any(|h| h.version.to_string() == from);
                let has_to = req
                    .requirement
                    .history
                    .iter()
                    .any(|h| h.version.to_string() == to);
                if has_from ^ has_to {
                    changed.push(id.clone());
                }
            }
            if changed.is_empty() {
                println!("no requirement history deltas found between {from} and {to}");
            } else {
                println!("requirements with history changes between {from} and {to}:");
                for id in changed {
                    println!("  {id}");
                }
            }
        }
    }
    Ok(())
}

fn scaffold_repository(
    repo_root: &std::path::Path,
    requirements_dir: Option<&str>,
    force: bool,
) -> Result<(), Box<dyn Error>> {
    std::fs::create_dir_all(repo_root)?;

    let requirements_dir_name = requirements_dir.unwrap_or("requirements");
    let config_path = repo_root.join("rqtk.toml");
    if config_path.exists() && !force {
        return Err(format!(
            "refusing to overwrite {}; pass --force to replace it",
            config_path.display()
        )
        .into());
    }

    let config = default_rqtk_toml(requirements_dir_name);
    std::fs::write(&config_path, config)?;

    let requirements_root = repo_root.join(requirements_dir_name);
    std::fs::create_dir_all(requirements_root.join("SYS"))?;
    Ok(())
}

fn default_rqtk_toml(requirements_dir: &str) -> String {
    format!(
        r#"[repository]
requirements_dir = "{requirements_dir}"
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

[change_control]
ccb_required_after = "Approved"
require_signoff = false
approvers = []

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

[export]
formats = ["json", "csv", "markdown"]
default_output_dir = "exports"
"#
    )
}

fn default_export_path(requirements_dir: &std::path::Path, format: &str) -> PathBuf {
    requirements_dir.join(format!("requirements-export.{format}"))
}

fn export_set(
    set: &RequirementSet<Validated>,
    format: &str,
    out: &std::path::Path,
) -> Result<(), Box<dyn Error>> {
    match format {
        "json" => {
            let payload = serde_json::to_string_pretty(&set.requirements)?;
            std::fs::write(out, payload)?;
        }
        "csv" => {
            let mut csv =
                String::from("id,title,category,type,state,priority,verification_method\n");
            for (id, req) in &set.requirements {
                let r = &req.requirement;
                csv.push_str(&format!(
                    "{},{},{},{},{},{},{}\n",
                    id.0,
                    escape_csv(&r.title),
                    r.category,
                    r.req_type,
                    r.status.state,
                    r.status.priority,
                    r.verification.method
                ));
            }
            std::fs::write(out, csv)?;
        }
        "markdown" => {
            let mut md = String::from("# Requirements Export\n\n");
            for (id, req) in &set.requirements {
                let r = &req.requirement;
                md.push_str(&format!(
                    "## {}\n\n- title: {}\n- category: {}\n- type: {}\n- state: {}\n- statement: {}\n\n",
                    id.0, r.title, r.category, r.req_type, r.status.state, r.statement.text
                ));
            }
            std::fs::write(out, md)?;
        }
        other => {
            return Err(Box::new(RqtkError::UnsupportedExportFormat(
                other.to_owned(),
            )));
        }
    }
    Ok(())
}

fn escape_csv(text: &str) -> String {
    let escaped = text.replace('"', "\"\"");
    format!("\"{escaped}\"")
}
