//! The `rqtk` command line. Public only so the binary and the Python package can run it;
//! not part of the library API.

mod commands;
mod output;

use clap::{Parser, Subcommand};
use commands::skills::AgentTarget;
use output::{Ctx, Exit, Format, Usage};
use rqtk_export::ExportFormat;
use std::error::Error;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

const AFTER_HELP: &str = "\
Exit codes:
  0  success, nothing to report
  1  findings: lint errors, failed or suspect verification, stale evidence
  2  usage error: bad arguments or unsupported option
  3  error: configuration, I/O or git failure";

#[derive(Debug, Parser)]
#[command(name = "rqtk", version, about = "Requirements Toolkit", after_help = AFTER_HELP)]
struct Cli {
    /// Repository root (the directory containing `.rqtk/`).
    #[arg(long, global = true, default_value = ".")]
    repo_root: PathBuf,
    /// Print one JSON document to stdout instead of text. Errors go to stderr as JSON.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Scaffold a new requirement set in the current repo.
    Init {
        #[arg(long)]
        requirements_dir: Option<String>,
        /// Overwrite an existing `.rqtk/config.toml`.
        #[arg(long)]
        force: bool,
        /// Also install the agent skills (see `rqtk skills install`).
        #[arg(long)]
        agents: bool,
        /// Report the files that would be created without writing them.
        #[arg(long)]
        dry_run: bool,
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
        /// Print the file that would be created without writing it.
        #[arg(long)]
        dry_run: bool,
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
        /// Print the file that would be created without writing it.
        #[arg(long)]
        dry_run: bool,
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
        /// Print the file that would be created without writing it.
        #[arg(long)]
        dry_run: bool,
    },
    /// Check every file against the schema, the config and the `verifies` links in source.
    Lint,
    /// Everything relevant to one requirement, need or stakeholder: links, tests, status,
    /// evidence and lint findings. Designed as the briefing for working on an item.
    Context { id: String },
    /// What changed since a git revision, and which requirements and activities it affects.
    Impact {
        /// Branch, tag, SHA, `HEAD~N` or baseline name to compare the working tree against.
        base: String,
    },
    /// Show the parent/child traceability chain of a requirement.
    Trace { id: String },
    /// Report need satisfaction and verification status (verified / suspect / failed / …).
    Coverage {
        /// Exit 1 unless every requirement is Verified and every need is satisfied.
        #[arg(long)]
        strict: bool,
        /// With --strict, also accept requirements in this state (repeatable), e.g. for
        /// requirements written ahead of their implementation.
        #[arg(long, value_enum, requires = "strict")]
        allow: Vec<commands::coverage::Allow>,
        /// Print only the one-line summary.
        #[arg(short, long)]
        short: bool,
    },
    /// Print the traceability graph of stakeholders, needs and requirements.
    Graph {
        #[arg(long, value_enum, default_value_t = commands::graph::GraphFormat::Dot)]
        format: commands::graph::GraphFormat,
    },
    /// Stamp all requirements and create a git tag baseline for HEAD.
    Baseline {
        version: String,
        /// Report what would be stamped and tagged without changing anything.
        #[arg(long)]
        dry_run: bool,
    },
    /// Export requirements to a file in the given format.
    Export {
        #[arg(long)]
        format: ExportFormat,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Show requirements that changed between two git revisions or baselines.
    Diff { from: String, to: String },
    /// List `verifies` links between source code and verification activities.
    Scan,
    /// Record test results as verification evidence in `.rqtk/evidence.toml`.
    ///
    /// Reads JUnit XML (cargo-nextest, pytest --junitxml, go-junit-report, jest-junit, …),
    /// matches test cases to `verifies` links and records each activity's outcome against the
    /// requirement's current content hash.
    Verify {
        /// JUnit XML result files.
        #[arg(long = "results", required = true, num_args = 1..)]
        results: Vec<PathBuf>,
        /// Do not write; exit 1 if the evidence file is out of date.
        #[arg(long, conflicts_with = "dry_run")]
        check: bool,
        /// Report what would be recorded without writing.
        #[arg(long)]
        dry_run: bool,
    },
    /// Confirm a requirement still holds after something it depends on changed.
    ///
    /// Settles a Suspect requirement whose tests passed again unchanged after it was
    /// reworded, or whose ancestor or need changed. Recorded in `.rqtk/evidence.toml`.
    Review {
        /// Requirement ID.
        id: String,
        /// Why the requirement still holds, kept with the review.
        #[arg(long)]
        note: Option<String>,
        /// Report what would be recorded without writing.
        #[arg(long)]
        dry_run: bool,
    },
    /// Search requirements, needs and stakeholders by substring.
    Search {
        /// Pattern to search for.
        pattern: String,
        /// Case-insensitive matching.
        #[arg(short = 'i', long)]
        ignore_case: bool,
        /// Restrict search to specific fields: id, title, statement, rationale, notes, keywords.
        #[arg(short = 'f', long, value_delimiter = ',')]
        field: Option<Vec<String>>,
    },
    /// Open a requirement file in $EDITOR.
    Open { id: String },
    /// Show the git commit history for a single requirement.
    Log { id: String },
    /// Install a git pre-commit hook that runs `rqtk rehash` and `rqtk lint`.
    InstallHook,
    /// Recompute and write content hashes for all requirements and needs.
    Rehash {
        /// Report stale hashes without writing.
        #[arg(long)]
        dry_run: bool,
    },
    /// Generate a Markdown requirements report.
    Report {
        /// Write to this file instead of stdout.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Print the JSON Schema of a file kind, or list the kinds.
    Schema {
        /// config, requirement, need, stakeholder or evidence.
        kind: Option<String>,
    },
    /// Explain a lint rule and how to fix it, or list all rules.
    Explain { code: Option<String> },
    /// Agent skills for working with rqtk in coding agents (Claude Code, Codex, …).
    #[command(subcommand)]
    Skills(SkillsCommand),
}

#[derive(Debug, Subcommand)]
enum SkillsCommand {
    /// List the bundled skills and who can invoke them.
    List,
    /// Write the skills into the repo and point AGENTS.md / CLAUDE.md at them.
    ///
    /// By default the skills go into `.agents/skills` (read by Codex, Cursor, GitHub Copilot,
    /// Gemini CLI, OpenCode, Amp and most other agents), and `.claude/skills` links to them
    /// for Claude Code. Skills you have edited are kept unless --force is given. The rqtk
    /// block goes into AGENTS.md and/or CLAUDE.md where they exist; no file is created
    /// unless named with --instructions.
    Install {
        /// Agents to install for.
        #[arg(
            long = "for",
            value_enum,
            value_delimiter = ',',
            default_values_t = [AgentTarget::Universal, AgentTarget::Claude]
        )]
        targets: Vec<AgentTarget>,
        /// Install a single copy into this directory instead (overrides --for).
        #[arg(long)]
        dir: Option<PathBuf>,
        /// Copy the skills into .claude/skills instead of linking to .agents/skills.
        #[arg(long)]
        copy: bool,
        /// Instructions file (AGENTS.md or CLAUDE.md) to add the rqtk block to; created if
        /// missing. Repeat for several.
        #[arg(long)]
        instructions: Vec<PathBuf>,
        /// Replace skills that were edited locally.
        #[arg(long)]
        force: bool,
        /// Report what would be written without writing.
        #[arg(long)]
        dry_run: bool,
    },
}

/// Run the command line with the process's arguments.
pub fn main() -> ExitCode {
    ExitCode::from(run_with_args(std::env::args_os()))
}

/// Run the command line with `args` (the first is the program name) and return the exit
/// code. Never exits the process, so it can run inside another program, such as the Python
/// package's `rqtk` command.
pub fn run_with_args<I, T>(args: I) -> u8
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(err) => {
            // --help and --version land here too, with exit code 0; usage errors exit 2.
            let _ = err.print();
            return err.exit_code() as u8;
        }
    };
    let ctx = Ctx {
        root: cli.repo_root.clone(),
        format: if cli.json { Format::Json } else { Format::Text },
    };
    match run(&ctx, cli.command) {
        Ok(Exit::Ok) => 0,
        Ok(Exit::Findings) => 1,
        Err(err) => {
            let usage = err.is::<Usage>();
            output::error(&ctx, &err.to_string(), usage);
            if usage { 2 } else { 3 }
        }
    }
}

fn run(ctx: &Ctx, command: Command) -> Result<Exit, Box<dyn Error>> {
    match command {
        Command::Init {
            requirements_dir,
            force,
            agents,
            dry_run,
        } if agents => {
            let init = commands::init::execute(ctx, requirements_dir.as_deref(), force, dry_run)?;
            let args = commands::skills::InstallArgs {
                dry_run,
                ..Default::default()
            };
            let skills = commands::skills::execute(ctx, &args)?;
            if ctx.json() {
                output::json(&serde_json::json!({ "init": init, "skills": skills }))?;
            } else {
                commands::init::print(&init);
                println!();
                commands::skills::print(&skills);
            }
            Ok(Exit::Ok)
        }
        Command::Init {
            requirements_dir,
            force,
            dry_run,
            ..
        } => commands::init::run(ctx, requirements_dir.as_deref(), force, dry_run),
        Command::Add {
            category,
            req_type,
            title,
            statement,
            rationale,
            dry_run,
        } => commands::add::run(
            ctx,
            commands::add::AddArgs {
                category,
                req_type,
                title,
                statement,
                rationale,
                dry_run,
            },
        ),
        Command::AddStakeholder {
            id,
            name,
            role,
            organization,
            dry_run,
        } => commands::add_stakeholder::run(
            ctx,
            commands::add_stakeholder::AddStakeholderArgs {
                id,
                name,
                role,
                organization,
                dry_run,
            },
        ),
        Command::AddNeed {
            id,
            title,
            statement,
            stakeholders,
            dry_run,
        } => commands::add_need::run(
            ctx,
            commands::add_need::AddNeedArgs {
                id,
                title,
                statement,
                stakeholders,
                dry_run,
            },
        ),
        Command::Lint => commands::lint::run(ctx),
        Command::Context { id } => commands::context::run(ctx, &id),
        Command::Impact { base } => commands::impact::run(ctx, &base),
        Command::Trace { id } => commands::trace::run(ctx, id),
        Command::Coverage {
            strict,
            allow,
            short,
        } => commands::coverage::run(ctx, strict, &allow, short),
        Command::Graph { format } => commands::graph::run(ctx, format),
        Command::Baseline { version, dry_run } => commands::baseline::run(ctx, version, dry_run),
        Command::Export { format, output } => commands::export::run(ctx, format, output),
        Command::Diff { from, to } => commands::diff::run(ctx, from, to),
        Command::Scan => commands::scan::run(ctx),
        Command::Verify {
            results,
            check,
            dry_run,
        } => commands::verify::run(
            ctx,
            commands::verify::VerifyArgs {
                results,
                check,
                dry_run,
            },
        ),
        Command::Review { id, note, dry_run } => {
            commands::review::run(ctx, commands::review::ReviewArgs { id, note, dry_run })
        }
        Command::Search {
            pattern,
            ignore_case,
            field,
        } => commands::search::run(
            ctx,
            commands::search::SearchArgs {
                pattern,
                ignore_case,
                field,
            },
        ),
        Command::Open { id } => commands::open::run(ctx, id),
        Command::Log { id } => commands::log::run(ctx, id),
        Command::InstallHook => commands::install_hook::run(ctx),
        Command::Rehash { dry_run } => commands::rehash::run(ctx, dry_run),
        Command::Report { output } => commands::report::run(ctx, output),
        Command::Schema { kind } => commands::schema::run(ctx, kind.as_deref()),
        Command::Explain { code } => commands::explain::run(ctx, code.as_deref()),
        Command::Skills(SkillsCommand::List) => commands::skills::list(ctx),
        Command::Skills(SkillsCommand::Install {
            targets,
            dir,
            copy,
            instructions,
            force,
            dry_run,
        }) => commands::skills::install(
            ctx,
            &commands::skills::InstallArgs {
                targets,
                dir,
                copy,
                instructions,
                force,
                dry_run,
            },
        ),
    }
}
