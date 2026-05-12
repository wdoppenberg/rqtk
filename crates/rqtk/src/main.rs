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
    /// Add a new requirement to the requirement set.
    Add {
        #[arg(long)]
        category: Option<String>,
        #[arg(long = "type")]
        req_type: Option<String>,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        statement: Option<String>,
        #[arg(long)]
        rationale: Option<String>,
    },
    /// Lint the requirement set for errors and inconsistencies.
    Lint,
    /// Trace the lifecycle of a requirement by its ID.
    Trace { id: String },
    /// Assess verification coverage across the requirement set.
    Coverage,
    /// Generate a DOT graph of the requirement traceability graph.
    Graph {
        #[arg(long, default_value = "dot")]
        format: String,
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
    InstallHook {
        /// Overwrite an existing pre-commit hook.
        #[arg(long)]
        force: bool,
    },
    /// Recompute and write content hashes for all requirements.
    Rehash,
    /// Generate a PDF requirements report via the Typst typesetting system.
    #[cfg(feature = "report")]
    Report {
        /// Output path: .pdf compiles via typst CLI, .typ writes the source.
        #[arg(short, long, default_value = "requirements-report.pdf")]
        output: PathBuf,
    },
    /// Generate a C++ header with compile-time verification activity checks.
    #[cfg(feature = "cpp")]
    CodegenCppVerifies {
        #[arg(long)]
        output: PathBuf,
        #[arg(long, default_value = "VERIFIES")]
        macro_name: String,
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
        Command::Lint => {
            commands::lint::run(root)?;
        }
        Command::Trace { id } => {
            commands::trace::run(root, id)?;
        }
        Command::Coverage => {
            commands::coverage::run(root)?;
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
        Command::InstallHook { force } => {
            commands::install_hook::run(root, force)?;
        }
        Command::Rehash => {
            commands::rehash::run(root)?;
        }
        #[cfg(feature = "report")]
        Command::Report { output } => {
            commands::report::run(root, output)?;
        }
        #[cfg(feature = "cpp")]
        Command::CodegenCppVerifies { output, macro_name } => {
            commands::codegen_cpp::run(root, output, macro_name)?;
        }
    }
    Ok(())
}
