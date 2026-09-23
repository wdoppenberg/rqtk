mod commands;
mod output;

use clap::{Parser, Subcommand};
use rqtk_export::ExportFormat;
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
    /// Scaffold a new requirement set in the current repo.
    Init {
        #[arg(long)]
        requirements_dir: Option<String>,
        #[arg(long)]
        force: bool,
    },
    /// Add a new requirement with the next free ID in its category.
    Add {
        /// Category key from `.rqtk/config.toml`, e.g. SYS.
        #[arg(long)]
        category: String,
        /// Requirement type from `types.allowed`.
        #[arg(long = "type")]
        req_type: String,
        #[arg(long)]
        title: String,
        /// A single normative sentence, e.g. "The system shall …".
        #[arg(long)]
        statement: String,
        #[arg(long)]
        rationale: Option<String>,
    },
    /// Add a new stakeholder definition.
    AddStakeholder {
        /// Defaults to the next free STK-NNN.
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        name: String,
        #[arg(long)]
        role: Option<String>,
        #[arg(long)]
        organization: Option<String>,
    },
    /// Add a new stakeholder need.
    AddNeed {
        /// Defaults to the next free NEED-NNNN.
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        title: String,
        #[arg(long)]
        statement: String,
        /// Stakeholder IDs associated with this need (comma-separated).
        #[arg(long, value_delimiter = ',')]
        stakeholders: Option<Vec<String>>,
    },
    /// Lint the requirement set for errors and inconsistencies.
    Lint,
    /// Trace the lifecycle of a requirement by its ID.
    Trace { id: String },
    /// Report verification coverage status (gap / planned / in-progress / verified).
    Coverage {
        /// Exit with a non-zero code if any requirement is not fully verified.
        #[arg(long)]
        strict: bool,
        /// Print only the one-line summary.
        #[arg(short, long)]
        short: bool,
    },
    /// Print the traceability graph of stakeholders, needs and requirements.
    Graph {
        #[arg(long, value_enum, default_value_t = commands::graph::GraphFormat::Dot)]
        format: commands::graph::GraphFormat,
    },
    /// Create a git tag baseline for the current HEAD.
    Baseline { version: String },
    /// Export requirements to a file in the given format.
    Export {
        #[arg(long)]
        format: ExportFormat,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Show requirements that changed between two baseline tags.
    Diff { from: String, to: String },
    /// Search requirements by string matching across fields.
    Search {
        /// Pattern to search for.
        pattern: String,
        /// Case-insensitive matching.
        #[arg(short = 'i', long)]
        ignore_case: bool,
        /// Restrict search to specific fields: id, title, statement, rationale, notes, keywords.
        #[arg(short, long, value_delimiter = ',')]
        field: Option<Vec<String>>,
    },
    /// Open a requirement file in $EDITOR.
    Open { id: String },
    /// Show the git commit history for a single requirement.
    Log { id: String },
    /// Install a git pre-commit hook that runs `rqtk rehash` and `rqtk lint`.
    InstallHook,
    /// Recompute and write content hashes for all requirements.
    Rehash,
    /// Generate a Markdown requirements report.
    Report {
        /// Write to this file instead of stdout.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

fn main() {
    if let Err(err) = run() {
        output::failure(&err.to_string());
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    let root = &cli.repo_root;
    match cli.command {
        Command::Init {
            requirements_dir,
            force,
        } => {
            commands::init::run(root, requirements_dir.as_deref(), force)?;
        }
        Command::Add {
            category,
            req_type,
            title,
            statement,
            rationale,
        } => {
            commands::add::run(
                root,
                commands::add::AddArgs {
                    category,
                    req_type,
                    title,
                    statement,
                    rationale,
                },
            )?;
        }
        Command::AddStakeholder {
            id,
            name,
            role,
            organization,
        } => {
            commands::add_stakeholder::run(
                root,
                commands::add_stakeholder::AddStakeholderArgs {
                    id,
                    name,
                    role,
                    organization,
                },
            )?;
        }
        Command::AddNeed {
            id,
            title,
            statement,
            stakeholders,
        } => {
            commands::add_need::run(
                root,
                commands::add_need::AddNeedArgs {
                    id,
                    title,
                    statement,
                    stakeholders,
                },
            )?;
        }
        Command::Lint => {
            commands::lint::run(root)?;
        }
        Command::Trace { id } => {
            commands::trace::run(root, id)?;
        }
        Command::Coverage { strict, short } => {
            commands::coverage::run(root, strict, short)?;
        }
        Command::Graph { format } => {
            commands::graph::run(root, format)?;
        }
        Command::Baseline { version } => {
            commands::baseline::run(root, version)?;
        }
        Command::Export { format, output } => {
            commands::export::run(root, format, output)?;
        }
        Command::Diff { from, to } => {
            commands::diff::run(root, from, to)?;
        }
        Command::Search {
            pattern,
            ignore_case,
            field,
        } => {
            commands::search::run(
                root,
                commands::search::SearchArgs {
                    pattern,
                    ignore_case,
                    field,
                },
            )?;
        }
        Command::Open { id } => {
            commands::open::run(root, id)?;
        }
        Command::Log { id } => {
            commands::log::run(root, id)?;
        }
        Command::InstallHook => {
            commands::install_hook::run(root)?;
        }
        Command::Rehash => {
            commands::rehash::run(root)?;
        }
        Command::Report { output } => {
            commands::report::run(root, output)?;
        }
    }
    Ok(())
}
