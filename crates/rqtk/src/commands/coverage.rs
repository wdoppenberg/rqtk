use std::{error::Error, path::Path};

use rqtk_core::{ClosureStatus, RequirementSet};

use crate::output;

pub fn run(repo_root: &Path, strict: bool, short: bool) -> Result<(), Box<dyn Error>> {
    let set = RequirementSet::load_from_repo_root(repo_root)?;
    let total = set.requirements.len();
    let (set, _) = set.validate();
    let statuses = set.verification_closure();

    let mut verified = Vec::new();
    let mut in_progress = Vec::new();
    let mut planned = Vec::new();
    let mut gap = Vec::new();

    for (id, status) in &statuses {
        match status {
            ClosureStatus::Verified => verified.push(id),
            ClosureStatus::InProgress => in_progress.push(id),
            ClosureStatus::Planned => planned.push(id),
            ClosureStatus::Gap => gap.push(id),
        }
    }

    println!(
        "\n  Verified {}  ·  In Progress {}  ·  Planned {}  ·  Gap {}  ({} total)",
        fmt_count(verified.len()),
        fmt_count(in_progress.len()),
        fmt_count(planned.len()),
        fmt_count(gap.len()),
        fmt_count(total)
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

    if strict && !gap.is_empty() {
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
