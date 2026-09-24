use std::{collections::BTreeMap, error::Error};

use console::style;
use rqtk_core::{
    ActivityState, ClosureStatus, NeedId, RequirementId, RequirementSet, RequirementVerification,
};
use serde::Serialize;

use crate::cli::output::{self, Ctx, Exit};

#[derive(Serialize)]
struct NeedStatus {
    id: NeedId,
    satisfied_by: Vec<RequirementId>,
}

#[derive(Serialize)]
struct RequirementStatus<'a> {
    id: &'a RequirementId,
    #[serde(flatten)]
    verification: &'a RequirementVerification,
}

#[derive(Serialize)]
struct Report<'a> {
    summary: BTreeMap<ClosureStatus, usize>,
    requirements: Vec<RequirementStatus<'a>>,
    needs: Vec<NeedStatus>,
    unsatisfied_needs: usize,
}

/// Requirement states `coverage --strict --allow` accepts besides Verified.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Allow {
    /// Activities defined, nothing run yet.
    Planned,
    /// Some activities done, not all.
    InProgress,
}

pub fn run(ctx: &Ctx, strict: bool, allow: &[Allow], short: bool) -> Result<Exit, Box<dyn Error>> {
    let (set, _) = RequirementSet::load_from_repo_root(&ctx.root)?.validate();
    let (links, evidence) = super::load_links_and_evidence(&set)?;
    let statuses = set.verification_status(&links, &evidence);

    let mut summary: BTreeMap<ClosureStatus, usize> = BTreeMap::new();
    for v in statuses.values() {
        *summary.entry(v.status).or_default() += 1;
    }
    let needs: Vec<NeedStatus> = set
        .satisfaction_closure()
        .into_keys()
        .map(|id| NeedStatus {
            satisfied_by: set
                .requirements()
                .iter()
                .filter(|(_, r)| r.trace.satisfies.contains(&id))
                .map(|(rid, _)| rid.clone())
                .collect(),
            id,
        })
        .collect();
    let unsatisfied = needs.iter().filter(|n| n.satisfied_by.is_empty()).count();

    let accepted = |status: &ClosureStatus| match status {
        ClosureStatus::Verified => true,
        ClosureStatus::Planned => allow.contains(&Allow::Planned),
        ClosureStatus::InProgress => allow.contains(&Allow::InProgress),
        ClosureStatus::Suspect | ClosureStatus::Failed | ClosureStatus::Gap => false,
    };
    let failing = summary.keys().any(|s| !accepted(s)) || unsatisfied > 0;
    let exit = Exit::findings_if(strict && failing);

    if ctx.json() {
        output::json(&Report {
            summary,
            requirements: statuses
                .iter()
                .map(|(id, verification)| RequirementStatus { id, verification })
                .collect(),
            needs,
            unsatisfied_needs: unsatisfied,
        })?;
        return Ok(exit);
    }

    // ── needs satisfaction ────────────────────────────────────────────────────
    let has_needs = !needs.is_empty();
    if has_needs {
        println!(
            "\n  Needs  Satisfied {}  ·  Unsatisfied {}  ({} total)",
            fmt_count(needs.len() - unsatisfied),
            fmt_count(unsatisfied),
            fmt_count(needs.len()),
        );
        if !short && unsatisfied > 0 {
            print_unsatisfied(&set, &needs);
        }
    }
    // ── verification closure ──────────────────────────────────────────────────
    let count = |s: ClosureStatus| fmt_count(summary.get(&s).copied().unwrap_or(0));
    let prefix = if has_needs { "  Reqs   " } else { "\n  " };
    println!(
        "{}Verified {}  ·  Suspect {}  ·  Failed {}  ·  In Progress {}  ·  Planned {}  ·  Gap {}  ({} total)",
        prefix,
        count(ClosureStatus::Verified),
        count(ClosureStatus::Suspect),
        count(ClosureStatus::Failed),
        count(ClosureStatus::InProgress),
        count(ClosureStatus::Planned),
        count(ClosureStatus::Gap),
        fmt_count(statuses.len()),
    );

    if !short {
        for (status, title, detail) in [
            (
                ClosureStatus::Failed,
                "Failed",
                "(a verification activity failed)",
            ),
            (
                ClosureStatus::Suspect,
                "Suspect",
                "(changed since its tests passed; run the tests and `rqtk verify`)",
            ),
            (
                ClosureStatus::Gap,
                "Gap",
                "(no activities or success criteria defined)",
            ),
            (
                ClosureStatus::Planned,
                "Planned",
                "(activities defined, none executed)",
            ),
            (
                ClosureStatus::InProgress,
                "In Progress",
                "(partially executed, not all terminal)",
            ),
        ] {
            let entries: Vec<String> = statuses
                .iter()
                .filter(|(_, v)| v.status == status)
                .map(|(id, v)| describe(id, v))
                .collect();
            if !entries.is_empty() {
                output::section(title, detail);
                for entry in &entries {
                    output::item(entry);
                }
            }
        }
    }
    println!();
    Ok(exit)
}

fn print_unsatisfied(set: &RequirementSet<rqtk_core::Validated>, needs: &[NeedStatus]) {
    // Group unsatisfied needs by stakeholder; ungrouped under "(none)".
    let mut by_stakeholder: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for status in needs.iter().filter(|n| n.satisfied_by.is_empty()) {
        let Some(need) = set.needs().get(&status.id) else {
            continue;
        };
        let entry = format!("{}  {}", status.id, need.title);
        if need.stakeholders.is_empty() {
            by_stakeholder
                .entry("(none)".to_owned())
                .or_default()
                .push(entry.clone());
        }
        for stk in &need.stakeholders {
            by_stakeholder
                .entry(stk.0.clone())
                .or_default()
                .push(entry.clone());
        }
    }
    for (stk, entries) in &by_stakeholder {
        output::section(
            &format!("Unsatisfied needs — {stk}"),
            "(no requirement satisfies this need)",
        );
        for e in entries {
            output::item(e);
        }
    }
}

/// The requirement ID, naming the activities behind a Failed or Suspect status.
fn describe(id: &RequirementId, v: &RequirementVerification) -> String {
    let flagged: Vec<String> = v
        .activities
        .iter()
        .filter_map(|(activity, state)| match state {
            ActivityState::Failed => Some(format!("{activity} failed")),
            ActivityState::Manual(Some(s)) if s == "Failed" => Some(format!("{activity} failed")),
            ActivityState::Suspect => Some(format!("{activity} suspect")),
            _ => None,
        })
        .collect();
    if flagged.is_empty() {
        id.to_string()
    } else {
        format!("{id}  ({})", flagged.join(", "))
    }
}

fn fmt_count(n: usize) -> String {
    if n == 0 {
        format!("{}", style(n).dim())
    } else {
        format!("{}", style(n).bold())
    }
}
