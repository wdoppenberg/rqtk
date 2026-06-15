use std::{error::Error, path::Path};

use rqtk_core::{ClosureStatus, RequirementSet, SatisfactionStatus};

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
            output::section("Unsatisfied needs", "(no requirement satisfies this need)");
            for id in &unsatisfied {
                let title = set.needs.get(id).map(|n| n.need.title.as_str()).unwrap_or("");
                output::item(&format!("{id}  {title}"));
            }
        }
    }

    // ── verification closure ──────────────────────────────────────────────────
    let ver_statuses = set.verification_closure();
    let mut verified = Vec::new();
    let mut in_progress = Vec::new();
    let mut planned = Vec::new();
    let mut gap = Vec::new();

    for (id, status) in &ver_statuses {
        match status {
            ClosureStatus::Verified => verified.push(id),
            ClosureStatus::InProgress => in_progress.push(id),
            ClosureStatus::Planned => planned.push(id),
            ClosureStatus::Gap => gap.push(id),
        }
    }

    let prefix = if has_needs { "  Reqs   " } else { "\n  " };
    println!(
        "{}Verified {}  ·  In Progress {}  ·  Planned {}  ·  Gap {}  ({} total)",
        prefix,
        fmt_count(verified.len()),
        fmt_count(in_progress.len()),
        fmt_count(planned.len()),
        fmt_count(gap.len()),
        fmt_count(req_total),
    );

    if !short {
        if !gap.is_empty() {
            output::section("Gap", "(no activities or success criteria defined)");
            for id in &gap {
                output::item(&id.to_string());
            }
        }
        if !planned.is_empty() {
            output::section("Planned", "(activities defined, none executed)");
            for id in &planned {
                output::item(&id.to_string());
            }
        }
        if !in_progress.is_empty() {
            output::section("In Progress", "(partially executed, not all terminal)");
            for id in &in_progress {
                output::item(&id.to_string());
            }
        }
    }

    println!();

    if strict && (!gap.is_empty() || needs_gap) {
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
