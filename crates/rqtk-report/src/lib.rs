//! Renders a requirement set as a single self-contained Markdown document.

use chrono::NaiveDate;
use rqtk_core::{
    ActivityState, ClosureStatus, Evidence, Requirement, RequirementId, RequirementSet,
    RequirementVerification, SatisfactionStatus, SourceLink, SuspectReason, Validated,
    VerificationActivity,
};
use std::collections::BTreeMap;
use std::fmt::Write;

/// What a report is rendered from.
pub struct ReportInput<'a> {
    pub set: &'a RequirementSet<Validated>,
    /// From [`RequirementSet::verification_status`].
    pub verification: &'a BTreeMap<RequirementId, RequirementVerification>,
    /// Recorded test evidence and reviews.
    pub evidence: &'a Evidence,
    /// `verifies` links found in source.
    pub links: &'a [SourceLink],
    /// Commit the report describes, if known.
    pub commit: Option<&'a str>,
    /// Requirement files differ from `commit` (uncommitted changes).
    pub uncommitted: bool,
    /// The latest baseline, if any.
    pub baseline: Option<BaselineInfo>,
    /// Printed in the document header.
    pub date: NaiveDate,
}

/// The latest baseline and how far the requirements have moved from it.
pub struct BaselineInfo {
    /// The tag, e.g. `rqtk/1.0.0`.
    pub tag: String,
    pub date: Option<NaiveDate>,
    /// Requirement and need files changed since the baseline.
    pub changed_since: usize,
}

/// Render the full requirements report.
pub fn render_report(input: &ReportInput<'_>) -> String {
    let set = input.set;
    let closure: BTreeMap<RequirementId, ClosureStatus> = input
        .verification
        .iter()
        .map(|(id, v)| (id.clone(), v.status))
        .collect();
    let mut out = String::with_capacity(64 * 1024);
    write_header(&mut out, input);
    write_summary(&mut out, set, &closure);
    write_attention(&mut out, set, input.verification);
    write_categories(&mut out, input);
    write_stakeholders(&mut out, set);
    write_needs(&mut out, set);
    write_trace_matrix(&mut out, set);
    write_verification_matrix(&mut out, input);
    write_verification_summary(&mut out, set, &closure);
    out
}

fn write_header(out: &mut String, input: &ReportInput<'_>) {
    let (set, date) = (input.set, input.date);
    let meta = &set.config().project;
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
        (
            "Commit",
            input.commit.map(|c| {
                let short = &c[..c.len().min(12)];
                if input.uncommitted {
                    format!("`{short}` plus uncommitted changes to requirements")
                } else {
                    format!("`{short}`")
                }
            }),
        ),
        (
            "Baseline",
            input.baseline.as_ref().map(|b| {
                let date = b.date.map(|d| format!(" ({d})")).unwrap_or_default();
                let since = match b.changed_since {
                    0 => "no requirement changes since".to_owned(),
                    1 => "1 requirement file changed since".to_owned(),
                    n => format!("{n} requirement files changed since"),
                };
                format!("`{}`{date}, {since}", b.tag)
            }),
        ),
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
        set.requirements().len(),
        set.needs().len(),
        set.stakeholders().len()
    );

    // Lifecycle states in configured order, then any unconfigured ones.
    let mut by_state: BTreeMap<&str, usize> = BTreeMap::new();
    for req in set.requirements().values() {
        *by_state.entry(req.state.as_str()).or_default() += 1;
    }
    out.push_str("### Lifecycle state\n\n| State | Count |\n|---|---:|\n");
    for state in &set.config().lifecycle.states {
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

    if !set.needs().is_empty() {
        let satisfied = set
            .satisfaction_closure()
            .values()
            .filter(|s| **s == SatisfactionStatus::Satisfied)
            .count();
        let _ = writeln!(
            out,
            "### Stakeholder needs\n\n{satisfied} of {} needs are satisfied by at least one requirement.\n",
            set.needs().len()
        );
    }

    out.push_str("### By category\n\n| Category | Name | Total | Verified | Gap |\n|---|---|---:|---:|---:|\n");
    for (key, cat) in sorted_categories(set) {
        let ids: Vec<&RequirementId> = set
            .requirements()
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

/// Failed and Suspect requirements, first, with the reason for each.
fn write_attention(
    out: &mut String,
    set: &RequirementSet<Validated>,
    verification: &BTreeMap<RequirementId, RequirementVerification>,
) {
    let open: Vec<(&RequirementId, &RequirementVerification)> = ordered_ids(set)
        .into_iter()
        .filter_map(|id| verification.get(id).map(|v| (id, v)))
        .filter(|(_, v)| matches!(v.status, ClosureStatus::Failed | ClosureStatus::Suspect))
        .collect();
    if open.is_empty() {
        return;
    }
    out.push_str("## Needs attention\n\n| Requirement | Status | Why |\n|---|---|---|\n");
    for (id, v) in open {
        let mut why: Vec<String> = v
            .activities
            .iter()
            .filter(|(_, s)| {
                matches!(s, ActivityState::Failed)
                    || matches!(s, ActivityState::Manual(Some(m)) if m == "Failed")
            })
            .map(|(a, _)| format!("`{a}` failed"))
            .collect();
        why.extend(v.suspect_reasons.iter().map(suspect_text));
        let _ = writeln!(
            out,
            "| `{id}` | {} | {} |",
            closure_label(&v.status),
            cell(&why.join("; "))
        );
    }
    out.push('\n');
}

fn suspect_text(reason: &SuspectReason) -> String {
    match reason {
        SuspectReason::RequirementChanged { activities } => format!(
            "changed since {} passed",
            activities
                .iter()
                .map(|a| format!("`{a}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        SuspectReason::TestsUnchanged { activities } => format!(
            "{} passed again with unchanged tests after it changed",
            activities
                .iter()
                .map(|a| format!("`{a}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        SuspectReason::UpstreamChanged { items } => {
            format!("upstream changed: {}", id_list(items))
        }
    }
}

fn write_categories(out: &mut String, input: &ReportInput<'_>) {
    let set = input.set;
    out.push_str("## Requirements\n\n");
    for (key, cat) in sorted_categories(set) {
        let _ = writeln!(out, "### {key} — {}\n", inline(&cat.name));
        if let Some(desc) = &cat.description {
            let _ = writeln!(out, "_{}_\n", inline(desc));
        }
        let reqs: Vec<&Requirement> = set
            .requirements()
            .values()
            .filter(|r| &r.category == key)
            .collect();
        if reqs.is_empty() {
            out.push_str("_No requirements in this category._\n\n");
        }
        for req in reqs {
            write_requirement(out, input, req);
        }
    }
}

fn write_requirement(out: &mut String, input: &ReportInput<'_>, req: &Requirement) {
    let verification = input.verification.get(&req.id);
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
        out.push_str("| Activity | Name | Status | Evidence |\n|---|---|---|---|\n");
        for a in &v.activities {
            let _ = writeln!(
                out,
                "| `{}` | {} | {} | {} |",
                a.id,
                cell(&a.name),
                cell(&activity_label(verification, &a.id)),
                evidence_cell(input, a)
            );
        }
        out.push('\n');
    }
    if let Some(review) = input.evidence.reviews.get(&req.id) {
        let note = review.note.as_deref().map(|n| format!(": {}", inline(n)));
        let _ = writeln!(
            out,
            "**Reviewed** {}{}\n",
            review.date,
            note.unwrap_or_default()
        );
    }
}

/// What backs an activity's status: the tests and commit recorded by `rqtk verify`, or the
/// date and files of a manual activity.
fn evidence_cell(input: &ReportInput<'_>, activity: &VerificationActivity) -> String {
    if let Some(e) = input.evidence.activities.get(&activity.id) {
        let tests = e
            .tests
            .iter()
            .map(|t| format!("`{}`", t.replace('|', "\\|")))
            .collect::<Vec<_>>()
            .join("<br>");
        let commit = e
            .commit
            .as_deref()
            .map(|c| format!(" at `{}`", &c[..c.len().min(12)]))
            .unwrap_or_default();
        return format!("{tests}{commit}");
    }
    let linked: Vec<String> = input
        .links
        .iter()
        .filter(|l| l.activity == activity.id)
        .map(|l| format!("`{}:{}`", l.path.display(), l.line))
        .collect();
    if !linked.is_empty() {
        return format!("not run: {}", linked.join(", "));
    }
    let mut parts = Vec::new();
    if let Some(date) = activity.executed_at {
        parts.push(date.to_string());
    }
    parts.extend(activity.evidence.iter().map(|e| format!("`{}`", cell(e))));
    if let Some(expected) = &activity.expected_result {
        parts.push(format!("expected: {}", cell(expected)));
    }
    parts.join(", ")
}

fn write_stakeholders(out: &mut String, set: &RequirementSet<Validated>) {
    if set.stakeholders().is_empty() {
        return;
    }
    out.push_str(
        "## Stakeholders\n\n| Stakeholder | Name | Role | Organization |\n|---|---|---|---|\n",
    );
    for (id, s) in set.stakeholders() {
        let _ = writeln!(
            out,
            "| `{id}` | {} | {} | {} |",
            cell(&s.name),
            cell(s.role.as_deref().unwrap_or("")),
            cell(s.organization.as_deref().unwrap_or(""))
        );
    }
    out.push('\n');
}

fn write_needs(out: &mut String, set: &RequirementSet<Validated>) {
    if set.needs().is_empty() {
        return;
    }
    let mut satisfied_by: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (id, req) in set.requirements() {
        for need in &req.trace.satisfies {
            satisfied_by.entry(&need.0).or_default().push(&id.0);
        }
    }
    out.push_str("## Stakeholder needs\n\n| Need | Title | State | Stakeholders | Satisfied by |\n|---|---|---|---|---|\n");
    for (id, need) in set.needs() {
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
    for id in ordered_ids(set) {
        let req = &set.requirements()[id];
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

/// One row per activity: the need it ultimately serves, the requirement, and what proves it.
fn write_verification_matrix(out: &mut String, input: &ReportInput<'_>) {
    let set = input.set;
    out.push_str("## Verification traceability\n\n| Needs | Requirement | Activity | Status | Evidence |\n|---|---|---|---|---|\n");
    for id in ordered_ids(set) {
        let req = &set.requirements()[id];
        let needs: Vec<String> = set
            .upstream_hashes(id)
            .into_keys()
            .filter(|k| set.needs().keys().any(|n| n.0 == *k))
            .collect();
        for a in &req.verification.activities {
            let _ = writeln!(
                out,
                "| {} | `{id}` | `{}` | {} | {} |",
                dash_if_empty(id_list(&needs)),
                a.id,
                cell(&activity_label(input.verification.get(id), &a.id)),
                evidence_cell(input, a)
            );
        }
    }
    out.push('\n');
}

fn write_verification_summary(
    out: &mut String,
    set: &RequirementSet<Validated>,
    closure: &BTreeMap<RequirementId, ClosureStatus>,
) {
    out.push_str("## Verification summary\n\n| ID | Title | Method | Level | Activities | Status |\n|---|---|---|---|---:|---|\n");
    for id in ordered_ids(set) {
        let req = &set.requirements()[id];
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

/// Requirement IDs by category level, then ID, so system requirements come before the
/// software requirements derived from them.
fn ordered_ids(set: &RequirementSet<Validated>) -> Vec<&RequirementId> {
    let level = |req: &Requirement| {
        set.config()
            .categories
            .get(&req.category)
            .map_or(u8::MAX, |c| c.level)
    };
    let mut ids: Vec<&RequirementId> = set.requirements().keys().collect();
    ids.sort_by_key(|id| (level(&set.requirements()[*id]), *id));
    ids
}

fn sorted_categories(set: &RequirementSet<Validated>) -> Vec<(&String, &rqtk_core::Category)> {
    let mut cats: Vec<_> = set.config().categories.iter().collect();
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
