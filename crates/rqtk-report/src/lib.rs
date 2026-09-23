//! Renders a requirement set as a single self-contained Markdown document.

use chrono::NaiveDate;
use rqtk_core::{
    ActivityState, ClosureStatus, Requirement, RequirementId, RequirementSet,
    RequirementVerification, SatisfactionStatus, Validated,
};
use std::collections::BTreeMap;
use std::fmt::Write;

/// Render the full requirements report. `verification` comes from
/// [`RequirementSet::verification_status`]; `date` is printed in the document header.
pub fn render_report(
    set: &RequirementSet<Validated>,
    verification: &BTreeMap<RequirementId, RequirementVerification>,
    date: NaiveDate,
) -> String {
    let closure: BTreeMap<RequirementId, ClosureStatus> = verification
        .iter()
        .map(|(id, v)| (id.clone(), v.status))
        .collect();
    let mut out = String::with_capacity(64 * 1024);
    write_header(&mut out, set, date);
    write_summary(&mut out, set, &closure);
    write_categories(&mut out, set, verification);
    write_needs(&mut out, set);
    write_trace_matrix(&mut out, set);
    write_verification_summary(&mut out, set, &closure);
    out
}

fn write_header(out: &mut String, set: &RequirementSet<Validated>, date: NaiveDate) {
    let meta = &set.config.project;
    let _ = writeln!(out, "# {} — Requirements Specification\n", meta.name);
    if let Some(desc) = &meta.description {
        let _ = writeln!(out, "{}\n", inline(desc));
    }
    let org = meta.organization.as_ref().and_then(|o| {
        o.program
            .as_deref()
            .or(o.responsible_engineer.as_deref())
            .or(o.center.as_deref())
    });
    let rows = [
        ("Version", Some(meta.version.to_string())),
        ("Date", Some(date.format("%Y-%m-%d").to_string())),
        ("Classification", meta.classification.clone()),
        ("Mission phase", meta.mission_phase.clone()),
        ("Organization", org.map(str::to_owned)),
    ];
    out.push_str("| | |\n|---|---|\n");
    for (label, value) in rows {
        if let Some(value) = value {
            let _ = writeln!(out, "| **{label}** | {} |", cell(&value));
        }
    }
    out.push('\n');
}

fn write_summary(
    out: &mut String,
    set: &RequirementSet<Validated>,
    closure: &BTreeMap<RequirementId, ClosureStatus>,
) {
    out.push_str("## Summary\n\n");
    let _ = writeln!(
        out,
        "{} requirements, {} stakeholder needs, {} stakeholders.\n",
        set.requirements.len(),
        set.needs.len(),
        set.stakeholders.len()
    );

    // Lifecycle states in configured order, then any unconfigured ones.
    let mut by_state: BTreeMap<&str, usize> = BTreeMap::new();
    for req in set.requirements.values() {
        *by_state.entry(req.state.as_str()).or_default() += 1;
    }
    out.push_str("### Lifecycle state\n\n| State | Count |\n|---|---:|\n");
    for state in &set.config.lifecycle.states {
        if let Some(n) = by_state.remove(state.as_str()) {
            let _ = writeln!(out, "| {} | {n} |", cell(state));
        }
    }
    for (state, n) in by_state {
        let _ = writeln!(out, "| {} | {n} |", cell(state));
    }
    out.push('\n');

    let count = |status: ClosureStatus| closure.values().filter(|s| **s == status).count();
    out.push_str("### Verification\n\n| Status | Count |\n|---|---:|\n");
    for (label, status) in [
        ("Verified", ClosureStatus::Verified),
        ("Suspect", ClosureStatus::Suspect),
        ("Failed", ClosureStatus::Failed),
        ("In progress", ClosureStatus::InProgress),
        ("Planned", ClosureStatus::Planned),
        ("Gap", ClosureStatus::Gap),
    ] {
        let _ = writeln!(out, "| {label} | {} |", count(status));
    }
    out.push('\n');

    if !set.needs.is_empty() {
        let satisfied = set
            .satisfaction_closure()
            .values()
            .filter(|s| **s == SatisfactionStatus::Satisfied)
            .count();
        let _ = writeln!(
            out,
            "### Stakeholder needs\n\n{satisfied} of {} needs are satisfied by at least one requirement.\n",
            set.needs.len()
        );
    }

    out.push_str("### By category\n\n| Category | Name | Total | Verified | Gap |\n|---|---|---:|---:|---:|\n");
    for (key, cat) in sorted_categories(set) {
        let ids: Vec<&RequirementId> = set
            .requirements
            .iter()
            .filter(|(_, r)| &r.category == key)
            .map(|(id, _)| id)
            .collect();
        let with = |status: ClosureStatus| {
            ids.iter()
                .filter(|id| closure.get(**id) == Some(&status))
                .count()
        };
        let _ = writeln!(
            out,
            "| `{key}` | {} | {} | {} | {} |",
            cell(&cat.name),
            ids.len(),
            with(ClosureStatus::Verified),
            with(ClosureStatus::Gap)
        );
    }
    out.push('\n');
}

fn write_categories(
    out: &mut String,
    set: &RequirementSet<Validated>,
    verification: &BTreeMap<RequirementId, RequirementVerification>,
) {
    out.push_str("## Requirements\n\n");
    for (key, cat) in sorted_categories(set) {
        let _ = writeln!(out, "### {key} — {}\n", inline(&cat.name));
        if let Some(desc) = &cat.description {
            let _ = writeln!(out, "_{}_\n", inline(desc));
        }
        let reqs: Vec<&Requirement> = set
            .requirements
            .values()
            .filter(|r| &r.category == key)
            .collect();
        if reqs.is_empty() {
            out.push_str("_No requirements in this category._\n\n");
        }
        for req in reqs {
            write_requirement(out, req, verification.get(&req.id));
        }
    }
}

fn write_requirement(
    out: &mut String,
    req: &Requirement,
    verification: Option<&RequirementVerification>,
) {
    let _ = writeln!(out, "#### `{}` — {}\n", req.id, inline(&req.title));

    let mut facts = vec![
        format!("**Type** {}", inline(&req.req_type)),
        format!("**State** {}", inline(&req.state)),
        format!("**Priority** {}", inline(&req.priority)),
    ];
    if let Some(c) = &req.criticality {
        facts.push(format!("**Criticality** {}", inline(c)));
    }
    if let Some(v) = verification {
        facts.push(format!("**Verification** {}", closure_label(&v.status)));
    }
    if req.tbd {
        facts.push("**TBD**".to_owned());
    }
    if req.tbr {
        facts.push("**TBR**".to_owned());
    }
    let _ = writeln!(out, "{}\n", facts.join(" · "));

    push_quote(out, &req.statement);
    if let Some(rationale) = &req.rationale {
        let _ = writeln!(out, "**Rationale.** {}\n", inline(rationale));
    }
    if let Some(notes) = &req.notes {
        let _ = writeln!(out, "**Notes.** {}\n", inline(notes));
    }
    if !req.assumptions.is_empty() {
        out.push_str("**Assumptions**\n\n");
        for a in &req.assumptions {
            let _ = writeln!(out, "- {}", inline(a));
        }
        out.push('\n');
    }
    if !req.trace.parents.is_empty() {
        let _ = writeln!(out, "**Parents** {}\n", id_list(&req.trace.parents));
    }
    if !req.trace.satisfies.is_empty() {
        let _ = writeln!(out, "**Satisfies** {}\n", id_list(&req.trace.satisfies));
    }

    if !req.parameters.is_empty() {
        out.push_str("| Parameter | Op | Value | Unit | Tolerance |\n|---|---|---|---|---|\n");
        for p in &req.parameters {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} |",
                cell(&p.name),
                cell(&p.operator),
                cell(&p.value.to_string()),
                cell(p.unit.as_deref().unwrap_or("")),
                p.tolerance.map(|t| t.to_string()).unwrap_or_default()
            );
        }
        out.push('\n');
    }

    let v = &req.verification;
    let _ = writeln!(
        out,
        "**Verification** {} · {} · {}",
        inline(&v.method),
        inline(&v.level),
        inline(&v.phase)
    );
    if let Some(criteria) = &v.success_criteria {
        let _ = writeln!(out, "— {}", inline(criteria));
    }
    out.push('\n');
    if !v.activities.is_empty() {
        out.push_str("| Activity | Name | Status | Expected result |\n|---|---|---|---|\n");
        for a in &v.activities {
            let _ = writeln!(
                out,
                "| `{}` | {} | {} | {} |",
                a.id,
                cell(&a.name),
                cell(&activity_label(verification, &a.id)),
                cell(a.expected_result.as_deref().unwrap_or(""))
            );
        }
        out.push('\n');
    }
}

fn write_needs(out: &mut String, set: &RequirementSet<Validated>) {
    if set.needs.is_empty() {
        return;
    }
    let mut satisfied_by: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (id, req) in &set.requirements {
        for need in &req.trace.satisfies {
            satisfied_by.entry(&need.0).or_default().push(&id.0);
        }
    }
    out.push_str("## Stakeholder needs\n\n| Need | Title | State | Stakeholders | Satisfied by |\n|---|---|---|---|---|\n");
    for (id, need) in &set.needs {
        let stakeholders: Vec<&str> = need.stakeholders.iter().map(|s| s.0.as_str()).collect();
        let by = satisfied_by.get(id.0.as_str()).map(|v| v.join(", "));
        let _ = writeln!(
            out,
            "| `{id}` | {} | {} | {} | {} |",
            cell(&need.title),
            cell(&need.state),
            cell(&stakeholders.join(", ")),
            by.as_deref().map_or("**unsatisfied**".to_owned(), cell)
        );
    }
    out.push('\n');
}

fn write_trace_matrix(out: &mut String, set: &RequirementSet<Validated>) {
    out.push_str("## Traceability matrix\n\n| ID | Title | Category | Parents | Satisfies |\n|---|---|---|---|---|\n");
    for (id, req) in &set.requirements {
        let _ = writeln!(
            out,
            "| `{id}` | {} | {} | {} | {} |",
            cell(&req.title),
            cell(&req.category),
            dash_if_empty(id_list(&req.trace.parents)),
            dash_if_empty(id_list(&req.trace.satisfies))
        );
    }
    out.push('\n');
}

fn write_verification_summary(
    out: &mut String,
    set: &RequirementSet<Validated>,
    closure: &BTreeMap<RequirementId, ClosureStatus>,
) {
    out.push_str("## Verification summary\n\n| ID | Title | Method | Level | Activities | Status |\n|---|---|---|---|---:|---|\n");
    for (id, req) in &set.requirements {
        let _ = writeln!(
            out,
            "| `{id}` | {} | {} | {} | {} | {} |",
            cell(&req.title),
            cell(&req.verification.method),
            cell(&req.verification.level),
            req.verification.activities.len(),
            closure.get(id).map_or("", closure_label)
        );
    }
}

fn sorted_categories(set: &RequirementSet<Validated>) -> Vec<(&String, &rqtk_core::Category)> {
    let mut cats: Vec<_> = set.config.categories.iter().collect();
    cats.sort_by_key(|(key, c)| (c.level, key.as_str()));
    cats
}

fn closure_label(status: &ClosureStatus) -> &'static str {
    match status {
        ClosureStatus::Verified => "Verified",
        ClosureStatus::Suspect => "**Suspect**",
        ClosureStatus::Failed => "**Failed**",
        ClosureStatus::InProgress => "In progress",
        ClosureStatus::Planned => "Planned",
        ClosureStatus::Gap => "**Gap**",
    }
}

fn activity_label(verification: Option<&RequirementVerification>, activity: &str) -> String {
    let state = verification.and_then(|v| v.activities.iter().find(|(id, _)| id == activity));
    match state.map(|(_, s)| s) {
        Some(ActivityState::Passed) => "Passed (tests)".to_owned(),
        Some(ActivityState::Failed) => "**Failed** (tests)".to_owned(),
        Some(ActivityState::Suspect) => "**Suspect** (tests passed on older content)".to_owned(),
        Some(ActivityState::NotRun) => "Not run".to_owned(),
        Some(ActivityState::Manual(Some(s))) => s.clone(),
        Some(ActivityState::Manual(None)) | None => "Planned".to_owned(),
    }
}

fn id_list<T: std::fmt::Display>(ids: &[T]) -> String {
    ids.iter()
        .map(|id| format!("`{id}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn dash_if_empty(s: String) -> String {
    if s.is_empty() { "—".to_owned() } else { s }
}

/// Collapse whitespace so free text stays on one Markdown line.
fn inline(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Text safe for a Markdown table cell.
fn cell(s: &str) -> String {
    inline(s).replace('|', "\\|")
}

fn push_quote(out: &mut String, text: &str) {
    for line in text.trim().lines() {
        let _ = writeln!(out, "> {line}");
    }
    out.push('\n');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_escapes_pipes_and_newlines() {
        assert_eq!(cell("a | b\nc"), "a \\| b c");
    }

    #[test]
    fn id_list_formats_code_spans() {
        assert_eq!(id_list(&["A", "B"]), "`A`, `B`");
        assert_eq!(dash_if_empty(id_list::<&str>(&[])), "—");
    }
}
