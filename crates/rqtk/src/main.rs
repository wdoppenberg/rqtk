use clap::{Parser, Subcommand};
use rqtk::{bump_baseline_version, format_lint, load_requirements, write_requirement_file};
use rqtk_core::{RequirementId, RqtkError, ScaffoldInput};
use semver::Version;
use std::error::Error;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "rqtk", version, about = "Requirements Toolkit")]
struct Cli {
    #[arg(long, default_value = "requirements")]
    requirements_dir: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
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
        Command::New {
            category,
            req_type,
            title,
            statement,
            rationale,
        } => {
            let set = load_requirements(&cli.requirements_dir)?;
            let file = set.scaffold_requirement(ScaffoldInput {
                category: &category,
                req_type: &req_type,
                title: &title,
                statement: &statement,
                rationale: rationale.as_deref(),
            });
            let id = file.requirement.id.clone();
            let path = cli.requirements_dir.join(format!("{id}.toml"));
            write_requirement_file(&path, &file)?;
            println!("created {}", path.display());
        }
        Command::Lint => {
            let set = load_requirements(&cli.requirements_dir)?;
            let issues = set.validate()?;
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
            let set = load_requirements(&cli.requirements_dir)?;
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
            let set = load_requirements(&cli.requirements_dir)?;
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
            let set = load_requirements(&cli.requirements_dir)?;
            if format != "dot" {
                return Err(Box::new(RqtkError::UnsupportedExportFormat(format)));
            }
            println!("{}", set.to_dot());
        }
        Command::Baseline { version } => {
            let mut set = load_requirements(&cli.requirements_dir)?;
            let parsed = Version::parse(&version)?;
            bump_baseline_version(&mut set, &parsed)?;
            println!("updated baseline version to {parsed}");
        }
        Command::Export { format, output } => {
            let set = load_requirements(&cli.requirements_dir)?;
            let out = output.unwrap_or_else(|| default_export_path(&cli.requirements_dir, &format));
            export_set(&set, &format, &out)?;
            println!("exported {}", out.display());
        }
        Command::Diff { from, to } => {
            let set = load_requirements(&cli.requirements_dir)?;
            let mut changed = Vec::new();
            for (id, req) in &set.requirements {
                let has_from = req.requirement.history.iter().any(|h| h.version.to_string() == from);
                let has_to = req.requirement.history.iter().any(|h| h.version.to_string() == to);
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

fn default_export_path(requirements_dir: &std::path::Path, format: &str) -> PathBuf {
    requirements_dir.join(format!("requirements-export.{format}"))
}

fn export_set(set: &rqtk_core::RequirementSet, format: &str, out: &std::path::Path) -> Result<(), Box<dyn Error>> {
    match format {
        "json" => {
            let payload = serde_json::to_string_pretty(&set.requirements)?;
            std::fs::write(out, payload)?;
        }
        "csv" => {
            let mut csv = String::from("id,title,category,type,state,priority,verification_method\n");
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
        other => return Err(Box::new(RqtkError::UnsupportedExportFormat(other.to_owned()))),
    }
    Ok(())
}

fn escape_csv(text: &str) -> String {
    let escaped = text.replace('"', "\"\"");
    format!("\"{escaped}\"")
}
