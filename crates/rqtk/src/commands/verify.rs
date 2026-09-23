use std::{collections::BTreeMap, error::Error, path::PathBuf};

use console::style;
use rqtk_core::evidence::{self, ActivityRun, EVIDENCE_PATH};
use rqtk_core::{Evidence, EvidenceChange, Outcome, RequirementSet, scan};
use serde::Serialize;

use crate::output::{self, Ctx, Exit};

pub struct VerifyArgs {
    pub results: Vec<PathBuf>,
    pub check: bool,
    pub dry_run: bool,
}

#[derive(Serialize)]
struct Report<'a> {
    test_results: usize,
    activities: &'a BTreeMap<String, ActivityRun>,
    changes: &'a [EvidenceChange],
    written: bool,
    up_to_date: bool,
}

/// Match JUnit results to `verifies` links and record the outcomes in `.rqtk/evidence.toml`.
///
/// Exits 1 if a linked test failed, or with `--check` if the evidence file is out of date.
pub fn run(ctx: &Ctx, args: VerifyArgs) -> Result<Exit, Box<dyn Error>> {
    let (set, _) = RequirementSet::load_from_repo_root(&ctx.root)?.validate();
    let links = scan::scan(set.repo_root(), &set.config().scan)?;
    let mut results = Vec::new();
    for path in &args.results {
        results.extend(evidence::read_junit(path)?);
    }

    let runs = evidence::match_results(&links, &results);
    let mut recorded = Evidence::load(set.repo_root())?;
    let commit = set.git().head_commit().map(|c| c.full().to_owned());
    let changes = recorded.apply(set.requirements(), &runs, commit.as_deref());
    let write = !changes.is_empty() && !args.check && !args.dry_run;
    if write {
        recorded.save(set.repo_root())?;
    }

    let any_failed = runs.values().any(|r| r.outcome() == Some(Outcome::Failed));
    let exit = Exit::findings_if(any_failed || (args.check && !changes.is_empty()));

    if ctx.json() {
        output::json(&Report {
            test_results: results.len(),
            activities: &runs,
            changes: &changes,
            written: write,
            up_to_date: changes.is_empty(),
        })?;
        return Ok(exit);
    }

    let count = |o: Option<Outcome>| runs.values().filter(|r| r.outcome() == o).count();
    println!(
        "\n  {} test results  ·  {} activities matched  ·  {} passed  ·  {} failed",
        style(results.len()).bold(),
        style(runs.len()).bold(),
        style(count(Some(Outcome::Passed))).bold(),
        style(count(Some(Outcome::Failed))).bold(),
    );
    let incomplete: Vec<String> = runs
        .iter()
        .filter(|(_, run)| run.outcome().is_none())
        .map(|(id, run)| format!("{id}  (no result for {})", run.missing.join(", ")))
        .collect();
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

    if changes.is_empty() {
        output::success("Evidence is up to date", &[]);
    } else if write {
        output::success(
            "Evidence recorded",
            &[
                ("file", EVIDENCE_PATH),
                ("changes", &changes.len().to_string()),
            ],
        );
    } else if args.check {
        output::failure(&format!(
            "{EVIDENCE_PATH} is out of date; run `rqtk verify` without --check and commit it"
        ));
    } else {
        output::success("Dry run: evidence not written", &[]);
    }
    Ok(exit)
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
