use std::{collections::BTreeMap, error::Error, path::Path};

use rqtk_core::{ActivityState, ClosureStatus, RequirementSet, SatisfactionStatus};

use crate::output;

pub fn run(repo_root: &Path, strict: bool, short: bool) -> Result<(), Box<dyn Error>> {
    let set = RequirementSet::load_from_repo_root(repo_root)?;
    let req_total = set.requirements.len();
    let has_needs = !set.needs.is_empty();
    let (set, _) = set.validate();

    // ── needs satisfaction ────────────────────────────────────────────────────
    let mut needs_gap = false;
    if has_needs {
        let satisfaction = set.satisfaction_closure();
        let mut satisfied = Vec::new();
        let mut unsatisfied = Vec::new();
        for (id, status) in &satisfaction {
            match status {
                SatisfactionStatus::Satisfied => satisfied.push(id),
                SatisfactionStatus::Unsatisfied => unsatisfied.push(id),
            }
        }
        needs_gap = !unsatisfied.is_empty();

        println!(
            "\n  Needs  Satisfied {}  ·  Unsatisfied {}  ({} total)",
            fmt_count(satisfied.len()),
            fmt_count(unsatisfied.len()),
            fmt_count(satisfaction.len()),
        );

        if !short && needs_gap {
            // Group unsatisfied needs by stakeholder; ungrouped under "(none)".
            let mut by_stakeholder: BTreeMap<String, Vec<String>> = BTreeMap::new();
            for id in &unsatisfied {
                let stks = set
                    .needs
                    .get(id)
                    .map(|n| {
                        n.stakeholders
                            .iter()
                            .map(|s| s.0.clone())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let title = set.needs.get(id).map(|n| n.title.as_str()).unwrap_or("");
                let entry = format!("{id}  {title}");
                if stks.is_empty() {
                    by_stakeholder
                        .entry("(none)".to_owned())
                        .or_default()
                        .push(entry);
                } else {
                    for stk in stks {
                        by_stakeholder.entry(stk).or_default().push(entry.clone());
                    }
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
    }

    // ── verification closure ──────────────────────────────────────────────────
    let (links, evidence) = super::load_links_and_evidence(&set)?;
    let statuses = set.verification_status(&links, &evidence);
    let mut by_status: BTreeMap<ClosureStatus, Vec<String>> = BTreeMap::new();
    for (id, v) in &statuses {
        // Name the activities behind a Failed or Suspect status.
        let flagged: Vec<String> = v
            .activities
            .iter()
            .filter_map(|(activity, state)| match state {
                ActivityState::Failed => Some(format!("{activity} failed")),
                ActivityState::Manual(Some(s)) if s == "Failed" => {
                    Some(format!("{activity} failed"))
                }
                ActivityState::Suspect => Some(format!("{activity} suspect")),
                _ => None,
            })
            .collect();
        let entry = if flagged.is_empty()
            || !matches!(v.status, ClosureStatus::Failed | ClosureStatus::Suspect)
        {
            id.to_string()
        } else {
            format!("{id}  ({})", flagged.join(", "))
        };
        by_status.entry(v.status).or_default().push(entry);
    }
    let count = |s: ClosureStatus| fmt_count(by_status.get(&s).map_or(0, Vec::len));

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
        fmt_count(req_total),
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
            if let Some(ids) = by_status.get(&status) {
                output::section(title, detail);
                for id in ids {
                    output::item(id);
                }
            }
        }
    }

    println!();

    let blocking = [
        ClosureStatus::Gap,
        ClosureStatus::Failed,
        ClosureStatus::Suspect,
    ];
    if strict && (blocking.iter().any(|s| by_status.contains_key(s)) || needs_gap) {
        std::process::exit(1);
    }

    Ok(())
}

fn fmt_count(n: usize) -> String {
    use console::style;
    if n == 0 {
        format!("{}", style(n).dim())
    } else {
        format!("{}", style(n).bold())
    }
}
