use std::error::Error;

use console::style;
use rqtk_core::query::{Context, Summary};
use rqtk_core::{ActivityState, RequirementSet};

use crate::cli::output::{self, Ctx, Exit, Usage};

pub fn run(ctx: &Ctx, id: &str) -> Result<Exit, Box<dyn Error>> {
    let (set, mut diagnostics) = RequirementSet::load_from_repo_root(&ctx.root)?.validate();
    let (links, evidence) = super::load_links_and_evidence(&set)?;
    diagnostics.extend(set.link_diagnostics(&links));
    for d in &mut diagnostics {
        if let Some(loc) = d.location.as_mut() {
            loc.path = output::relative(&loc.path, &ctx.root);
        }
    }
    let verification = set.verification_status(&links, &evidence);
    let context = set
        .context(id, &links, &evidence, &verification, &diagnostics)
        .ok_or_else(|| {
            Usage(format!(
                "no requirement, need or stakeholder with ID `{id}`"
            ))
        })?;

    if ctx.json() {
        output::json(&context)?;
        return Ok(Exit::Ok);
    }

    match &context {
        Context::Requirement(c) => {
            let r = c.requirement;
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
                    let arrow = if l.direction == "outgoing" {
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
            if let Some(v) = &c.verification {
                output::section("Verification", &format!("{:?}", v.status));
                for (activity, state) in &v.activities {
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
        Context::Stakeholder(c) => {
            let s = c.stakeholder;
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
