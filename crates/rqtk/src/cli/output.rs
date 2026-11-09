use crate::cli::error::CliError;
use comfy_table::{Attribute, Cell, Color, ContentArrangement, Table, presets};
use console::style;
use rqtk_core::{Diagnostic, Severity};
use serde::Serialize;
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

impl std::error::Error for Usage {}

/// Print `value`, a document that carries its own `schema` field (the client's views), as
/// pretty JSON on stdout.
pub fn json<T: Serialize + ?Sized>(value: &T) -> Result<(), CliError> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

/// Print `value` as document `name`: pretty JSON on stdout, with `schema` set to
/// `rqtk/<name>/<version>`. `value` must serialise to a JSON object.
pub fn document<T: Serialize + ?Sized>(name: &str, value: &T) -> Result<(), CliError> {
    /// `value`'s fields with `schema` first. Serialising a value that isn't a struct or map
    /// fails, so every document is an object.
    #[derive(Serialize)]
    struct Stamped<'a, T: ?Sized> {
        schema: String,
        #[serde(flatten)]
        body: &'a T,
    }
    json(&Stamped {
        schema: rqtk_client::views::schema_id(name),
        body: value,
    })
}

/// Report a failed command on stderr, as JSON with `--json`. `code` is the client's error
/// code, when the error came from opening or querying the repository.
pub fn error(ctx: &Ctx, err: &CliError) {
    let message = err.to_string();
    if ctx.json() {
        let mut error = serde_json::json!({ "kind": err.kind(), "message": message });
        if let Some(code) = err.code() {
            error["code"] = serde_json::Value::String(code.as_str().to_owned());
        }
        let body = serde_json::json!({
            "schema": rqtk_client::views::schema_id("error"),
            "error": error,
        });
        eprintln!("{body}");
    } else {
        failure(&message);
    }
}

/// `path` relative to `root` when it lies inside it.
pub fn relative(path: &Path, root: &Path) -> PathBuf {
    path.strip_prefix(root).unwrap_or(path).to_path_buf()
}

/// `s` without control characters other than newlines and tabs. Every human-readable line
/// goes through this, because messages can quote text a provider controls (vendored
/// requirements, file names, TOML errors) and an escape sequence in it could drive the
/// terminal.
pub fn safe(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect()
}

/// [`safe`], on one line: newlines and tabs become spaces. For provider text shown as a
/// single field, such as a vendored requirement's title.
pub fn printable(s: &str) -> String {
    s.chars()
        .map(|c| if c == '\n' || c == '\t' { ' ' } else { c })
        .filter(|c| !c.is_control())
        .collect()
}

/// One line describing a consumed interface: its qualified ID, title and the provider's
/// status.
pub fn consumed_line(s: &rqtk_core::query::ConsumedSummary) -> String {
    if !s.resolved {
        return printable(&format!("{}  (not in the vendored snapshot)", s.id));
    }
    printable(&format!(
        "{}  {}  [{}]",
        s.id,
        s.title.as_deref().unwrap_or(""),
        s.provider_status.map_or("status unknown", |s| s.as_str())
    ))
}

const INDENT: &str = "  ";
const SUB: &str = "     ";

pub fn success(label: &str, pairs: &[(&str, &str)]) {
    println!(
        "{}{} {}",
        INDENT,
        style("✓").green().bold(),
        style(safe(label)).bold()
    );
    for (k, v) in pairs {
        println!(
            "{}{} {:<20}{}",
            SUB,
            style("└─").dim(),
            style(safe(k)).dim(),
            style(safe(v)).cyan()
        );
    }
}

pub fn warning(msg: &str) {
    eprintln!(
        "{}{} {}",
        INDENT,
        style("!").yellow().bold(),
        style(safe(msg)).yellow()
    );
}

pub fn failure(msg: &str) {
    eprintln!(
        "{}{} {}",
        INDENT,
        style("✗").red().bold(),
        style(safe(msg)).bold()
    );
}

pub fn section(title: &str, detail: &str) {
    if detail.is_empty() {
        println!("\n{}{}", INDENT, style(safe(title)).bold());
    } else {
        println!(
            "\n{}{}  {}",
            INDENT,
            style(safe(title)).bold(),
            style(safe(detail)).dim()
        );
    }
}

pub fn subsection(icon: &str, title: &str) {
    println!(
        "\n{}{}  {}",
        INDENT,
        style(icon).bold(),
        style(safe(title)).bold()
    );
}

pub fn item(id: &str) {
    println!("{}{}  {}", SUB, style("·").dim(), style(safe(id)).bold());
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
            Cell::new(safe(&location)).fg(Color::DarkGrey),
            Cell::new(subject).add_attribute(Attribute::Bold),
            Cell::new(sev_str).fg(sev_color),
            Cell::new(issue.code),
            Cell::new(safe(&issue.message)),
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
