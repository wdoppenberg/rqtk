use std::{error::Error, path::Path, path::PathBuf};

use console::style;
use rqtk_core::evidence::{self, EVIDENCE_PATH};
use rqtk_core::{Evidence, EvidenceChange, Outcome, RequirementSet, scan};

use crate::output;

pub struct VerifyArgs {
    pub results: Vec<PathBuf>,
    pub check: bool,
}

/// Match JUnit results to `verifies` links and record the outcomes in `.rqtk/evidence.toml`.
///
/// Exit codes: 1 if a linked test failed, or with `--check` if the evidence file is out of
/// date; 0 otherwise.
pub fn run(repo_root: &Path, args: VerifyArgs) -> Result<(), Box<dyn Error>> {
    let (set, _) = RequirementSet::load_from_repo_root(repo_root)?.validate();
    let links = scan::scan(&set.repo_root, &set.config.scan)?;
    let mut results = Vec::new();
    for path in &args.results {
        results.extend(evidence::read_junit(path)?);
    }

    let runs = evidence::match_results(&links, &results);
    let mut recorded = Evidence::load(&set.repo_root)?;
    let commit = set.git.head_commit().map(|c| c.full().to_owned());
    let changes = recorded.apply(&set.requirements, &runs, commit.as_deref());

    let failed: Vec<&str> = runs
        .iter()
        .filter(|(_, run)| run.outcome() == Some(Outcome::Failed))
        .map(|(id, _)| id.as_str())
        .collect();
    let incomplete: Vec<String> = runs
        .iter()
        .filter(|(_, run)| run.outcome().is_none())
        .map(|(id, run)| format!("{id}  (no result for {})", run.missing.join(", ")))
        .collect();

    println!(
        "\n  {} test results  ·  {} activities matched  ·  {} passed  ·  {} failed",
        style(results.len()).bold(),
        style(runs.len()).bold(),
        style(runs.len() - failed.len() - incomplete.len()).bold(),
        style(failed.len()).bold(),
    );
    if !incomplete.is_empty() {
        output::section(
            "Incomplete",
            "(not all linked tests ran; evidence unchanged)",
        );
        for line in &incomplete {
            output::item(line);
        }
    }
    if !changes.is_empty() {
        output::section("Evidence changes", "");
        for change in &changes {
            output::item(&describe(change));
        }
    }
    println!();

    if args.check && !changes.is_empty() {
        output::failure(&format!(
            "{EVIDENCE_PATH} is out of date; run `rqtk verify` without --check and commit it"
        ));
        std::process::exit(1);
    } else if !changes.is_empty() {
        recorded.save(&set.repo_root)?;
        output::success(
            "Evidence recorded",
            &[
                ("file", EVIDENCE_PATH),
                ("changes", &changes.len().to_string()),
            ],
        );
    } else {
        output::success("Evidence is up to date", &[]);
    }

    if !failed.is_empty() {
        std::process::exit(1);
    }
    Ok(())
}

fn describe(change: &EvidenceChange) -> String {
    let outcome = |o: Outcome| match o {
        Outcome::Passed => "passed",
        Outcome::Failed => "failed",
    };
    match change {
        EvidenceChange::Added(e) => format!("+ {}  {}", e.id, outcome(e.outcome)),
        EvidenceChange::Updated { before, after } if before.outcome != after.outcome => format!(
            "~ {}  {} → {}",
            after.id,
            outcome(before.outcome),
            outcome(after.outcome)
        ),
        EvidenceChange::Updated { after, .. } => {
            format!("~ {}  {} (re-verified)", after.id, outcome(after.outcome))
        }
        EvidenceChange::Removed(e) => format!("- {}  (activity no longer exists)", e.id),
    }
}
