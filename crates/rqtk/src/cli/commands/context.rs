use console::style;
use rqtk_core::ActivityState;
use rqtk_core::query::{Context, Summary};

use crate::cli::error::CliError;
use crate::cli::output::{self, Ctx, Exit, consumed_line, printable};

pub fn run(ctx: &Ctx, id: &str) -> Result<Exit, CliError> {
    let repo = super::open(ctx)?;
    let doc = repo.context(id).map_err(CliError::usage_if_not_found)?;

    if ctx.json() {
        output::json(&doc)?;
        return Ok(Exit::Ok);
    }

    let context = &doc.context;
    match &context {
        Context::Requirement(c) => {
            let r = &c.requirement;
            header(&r.id.0, &r.title, &c.path.display().to_string());
            println!(
                "  {}",
                style(format!(
                    "{} · {} · {} · {}",
                    r.category, r.req_type, r.state, r.priority
                ))
                .dim()
            );
            field("Statement", &r.statement);
            if let Some(rationale) = &r.rationale {
                field("Rationale", rationale);
            }
            list("Ancestors", &c.ancestors);
            list("Children", &c.children);
            if !c.links.is_empty() {
                output::section("Links", "");
                for l in &c.links {
                    let arrow = if l.direction == rqtk_core::query::Direction::Outgoing {
                        "→"
                    } else {
                        "←"
                    };
                    output::item(&format!(
                        "{} {arrow} {}  {}",
                        l.field,
                        l.id,
                        l.title.as_deref().unwrap_or("")
                    ));
                }
            }
            list("Satisfies", &c.needs);
            if !c.consumes.is_empty() {
                output::section("Consumes", "(vendored interfaces; provider status)");
                for s in &c.consumes {
                    output::item(&consumed_line(s));
                }
            }
            if let Some(v) = &c.verification {
                output::section("Verification", &format!("{:?}", v.status));
                for rqtk_core::ActivityStatus {
                    id: activity,
                    state,
                } in &v.activities
                {
                    let tests: Vec<String> = c
                        .tests
                        .iter()
                        .filter(|t| &t.activity == activity)
                        .map(|t| {
                            format!(
                                "{}:{} {}",
                                t.path.display(),
                                t.line,
                                t.test_name.as_deref().unwrap_or("?")
                            )
                        })
                        .collect();
                    let tests = if tests.is_empty() {
                        String::new()
                    } else {
                        format!("  ← {}", tests.join(", "))
                    };
                    output::item(&format!("{activity}  {}{tests}", state_label(state)));
                }
                if !v.suspect_reasons.is_empty() {
                    output::section("Suspect because", "");
                    for reason in &v.suspect_reasons {
                        output::item(&super::review::describe(reason));
                    }
                }
            }
            findings(&c.diagnostics);
        }
        Context::Need(c) => {
            header(&c.need.id.0, &c.need.title, &c.path.display().to_string());
            field("Statement", &c.need.statement);
            let stakeholders: Vec<Summary> = c
                .stakeholders
                .iter()
                .map(|s| Summary {
                    id: s.id.0.clone(),
                    title: s.name.clone(),
                    statement: None,
                })
                .collect();
            list("Stakeholders", &stakeholders);
            if c.satisfied_by.is_empty() {
                output::section("Satisfied by", "(nothing yet)");
            }
            list("Satisfied by", &c.satisfied_by);
            findings(&c.diagnostics);
        }
        Context::Interface(c) => {
            let r = &c.requirement;
            header(&c.id, &printable(&r.title), &c.path.display().to_string());
            println!(
                "  {}",
                style(format!(
                    "vendored from `{}` · read-only · provider status: {}",
                    c.alias,
                    c.provider_status.map_or("unknown", |s| s.as_str())
                ))
                .dim()
            );
            field("Statement", &printable(&r.statement));
            if let Some(interface) = &r.interface {
                output::section("Specs", "");
                for s in &interface.specs {
                    let element = s
                        .element
                        .as_deref()
                        .map(|e| format!("  {e}"))
                        .unwrap_or_default();
                    output::item(&printable(&format!("{} {}{element}", s.format, s.path)));
                }
            }
            if c.consumers.is_empty() {
                output::section("Consumed by", "(nothing yet)");
            }
            list("Consumed by", &c.consumers);
        }
        Context::Stakeholder(c) => {
            let s = &c.stakeholder;
            header(&s.id.0, &s.name, &c.path.display().to_string());
            if let Some(role) = &s.role {
                field("Role", role);
            }
            list("Needs", &c.needs);
            findings(&c.diagnostics);
        }
    }
    println!();
    Ok(Exit::Ok)
}

fn header(id: &str, title: &str, path: &str) {
    println!(
        "\n  {}  {}  {}",
        style(id).bold(),
        style(title).bold(),
        style(path).dim()
    );
}

fn field(label: &str, value: &str) {
    println!("\n  {:<10} {}", style(label).cyan(), value.trim());
}

fn list(title: &str, items: &[Summary]) {
    if items.is_empty() {
        return;
    }
    output::section(title, "");
    for s in items {
        output::item(&format!("{}  {}", s.id, s.title));
    }
}

fn findings(diagnostics: &[rqtk_core::Diagnostic]) {
    if diagnostics.is_empty() {
        return;
    }
    output::section("Findings", "");
    for d in diagnostics {
        output::item(&format!("{}  {}", d.code, d.message));
    }
}

fn state_label(state: &ActivityState) -> String {
    match state {
        ActivityState::Passed => "passed".into(),
        ActivityState::Failed => "FAILED".into(),
        ActivityState::Suspect => "SUSPECT (requirement changed since tests passed)".into(),
        ActivityState::NotRun => "not run".into(),
        ActivityState::Manual(Some(s)) => format!("{s} (manual)"),
        ActivityState::Manual(None) => "planned (manual)".into(),
    }
}
