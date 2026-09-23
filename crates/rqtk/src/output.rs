use comfy_table::{Attribute, Cell, Color, ContentArrangement, Table, presets};
use console::style;
use rqtk_core::{Diagnostic, Severity};
use serde::Serialize;
use std::error::Error;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Text,
    Json,
}

/// What every command gets: where the repository is and how to print.
pub struct Ctx {
    pub root: PathBuf,
    pub format: Format,
}

impl Ctx {
    pub fn json(&self) -> bool {
        self.format == Format::Json
    }

    /// Reject `--json` for commands whose output is a document in another format.
    pub fn require_text(&self, command: &str) -> Result<(), Usage> {
        if self.json() {
            return Err(Usage(format!("`rqtk {command}` does not support --json")));
        }
        Ok(())
    }
}

/// How a command that ran to completion wants the process to exit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    /// Exit 0.
    Ok,
    /// Exit 1: the command worked and found problems (lint errors, failed verification, …).
    Findings,
}

impl Exit {
    pub fn findings_if(condition: bool) -> Self {
        if condition { Exit::Findings } else { Exit::Ok }
    }
}

/// An error in how the command was invoked; exits 2.
#[derive(Debug)]
pub struct Usage(pub String);

impl std::fmt::Display for Usage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for Usage {}

/// Print `value` as pretty JSON on stdout.
pub fn json<T: Serialize + ?Sized>(value: &T) -> Result<(), Box<dyn Error>> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

/// Report a failed command on stderr, as JSON with `--json`.
pub fn error(ctx: &Ctx, message: &str, usage: bool) {
    if ctx.json() {
        let kind = if usage { "usage" } else { "error" };
        let body = serde_json::json!({ "error": { "kind": kind, "message": message } });
        eprintln!("{body}");
    } else {
        failure(message);
    }
}

/// `path` relative to `root` when it lies inside it.
pub fn relative(path: &Path, root: &Path) -> PathBuf {
    path.strip_prefix(root).unwrap_or(path).to_path_buf()
}

const INDENT: &str = "  ";
const SUB: &str = "     ";

pub fn success(label: &str, pairs: &[(&str, &str)]) {
    println!(
        "{}{} {}",
        INDENT,
        style("✓").green().bold(),
        style(label).bold()
    );
    for (k, v) in pairs {
        println!(
            "{}{} {:<20}{}",
            SUB,
            style("└─").dim(),
            style(*k).dim(),
            style(*v).cyan()
        );
    }
}

pub fn failure(msg: &str) {
    eprintln!(
        "{}{} {}",
        INDENT,
        style("✗").red().bold(),
        style(msg).bold()
    );
}

pub fn section(title: &str, detail: &str) {
    if detail.is_empty() {
        println!("\n{}{}", INDENT, style(title).bold());
    } else {
        println!(
            "\n{}{}  {}",
            INDENT,
            style(title).bold(),
            style(detail).dim()
        );
    }
}

pub fn subsection(icon: &str, title: &str) {
    println!(
        "\n{}{}  {}",
        INDENT,
        style(icon).bold(),
        style(title).bold()
    );
}

pub fn item(id: &str) {
    println!("{}{}  {}", SUB, style("·").dim(), style(id).bold());
}

/// Print diagnostics as a table; returns (errors, warnings).
pub fn lint_table(issues: &[Diagnostic], repo_root: &Path) -> (usize, usize) {
    let mut errors = 0usize;
    let mut warnings = 0usize;

    let mut table = Table::new();
    table
        .load_preset(presets::NOTHING)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            Cell::new("Location").add_attribute(Attribute::Bold),
            Cell::new("ID").add_attribute(Attribute::Bold),
            Cell::new("Severity").add_attribute(Attribute::Bold),
            Cell::new("Code").add_attribute(Attribute::Bold),
            Cell::new("Message").add_attribute(Attribute::Bold),
        ]);

    for issue in issues {
        let (sev_str, sev_color) = match issue.severity {
            Severity::Error => {
                errors += 1;
                ("error", Color::Red)
            }
            Severity::Warning => {
                warnings += 1;
                ("warning", Color::Yellow)
            }
        };
        let location = issue.location.as_ref().map_or_else(
            || "-".to_owned(),
            |loc| {
                let path = loc.path.strip_prefix(repo_root).unwrap_or(&loc.path);
                match loc.line {
                    Some(line) => format!("{}:{line}", path.display()),
                    None => path.display().to_string(),
                }
            },
        );
        let subject = issue
            .subject
            .as_ref()
            .map_or_else(|| "-".to_owned(), ToString::to_string);
        table.add_row(vec![
            Cell::new(location).fg(Color::DarkGrey),
            Cell::new(subject).add_attribute(Attribute::Bold),
            Cell::new(sev_str).fg(sev_color),
            Cell::new(issue.code),
            Cell::new(&issue.message),
        ]);
    }

    for line in table.to_string().lines() {
        println!("{}{}", INDENT, line);
    }

    (errors, warnings)
}

pub fn lint_summary(errors: usize, warnings: usize, total: usize) {
    let err_text = if errors > 0 {
        format!(
            "{}",
            style(format!(
                "{errors} error{}",
                if errors == 1 { "" } else { "s" }
            ))
            .red()
            .bold()
        )
    } else {
        format!("{}", style("0 errors").dim())
    };
    let warn_text = if warnings > 0 {
        format!(
            "{}",
            style(format!(
                "{warnings} warning{}",
                if warnings == 1 { "" } else { "s" }
            ))
            .yellow()
            .bold()
        )
    } else {
        format!("{}", style("0 warnings").dim())
    };
    println!(
        "\n{}{}  ·  {}  {}",
        INDENT,
        err_text,
        warn_text,
        style(format!("({total} checked)")).dim()
    );
}
