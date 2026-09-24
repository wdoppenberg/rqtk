//! Link discovery and result matching against real test runner output.
//!
//! Each directory under `tests/fixtures/ecosystems/` holds test sources with `rqtk: verifies`
//! tags, the JUnit XML its runner actually wrote for them (captured once, not generated), and
//! `expected*.toml` with the outcome each activity must get. The sources include the cases
//! that went wrong in practice: parameterised and table-driven tests, `it.each`, disabled
//! tests, and unrelated tests that share a linked test's name.

use rqtk_core::evidence::{Outcome, match_results, read_junit};
use rqtk_core::model::ScanConfig;
use rqtk_core::scan::scan;
use rqtk_macros::verifies;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(serde::Deserialize)]
struct Expected {
    results: Vec<PathBuf>,
    outcomes: BTreeMap<String, String>,
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ecosystems")
}

/// The outcome of every activity tagged in `dir`, as named in `expected*.toml`.
fn outcomes(dir: &Path, results: &[PathBuf]) -> BTreeMap<String, String> {
    let links = scan(dir, &ScanConfig::default()).unwrap();
    let mut parsed = Vec::new();
    for file in results {
        parsed.extend(read_junit(&dir.join(file)).unwrap());
    }
    let runs = match_results(&links, &parsed);
    links
        .iter()
        .map(|link| {
            let outcome = match runs.get(&link.activity) {
                None => "not_run",
                Some(run) if !run.ambiguous.is_empty() => "ambiguous",
                Some(run) => match run.outcome() {
                    Some(Outcome::Passed) => "passed",
                    Some(Outcome::Failed) => "failed",
                    None => "incomplete",
                },
            };
            (link.activity.clone(), outcome.to_owned())
        })
        .collect()
}

#[verifies("VA-CORE-006-03")]
#[test]
fn every_ecosystem_matches_its_expected_outcomes() {
    let mut checked = 0;
    let mut failures = Vec::new();
    for entry in std::fs::read_dir(fixtures()).unwrap() {
        let dir = entry.unwrap().path();
        for file in std::fs::read_dir(&dir).unwrap() {
            let file = file.unwrap().path();
            let name = file.file_name().unwrap().to_string_lossy().into_owned();
            if !(name.starts_with("expected") && name.ends_with(".toml")) {
                continue;
            }
            let expected: Expected =
                toml::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
            let actual = outcomes(&dir, &expected.results);
            if actual != expected.outcomes {
                failures.push(format!(
                    "{}/{name}\n  expected {:?}\n  actual   {:?}",
                    dir.file_name().unwrap().to_string_lossy(),
                    expected.outcomes,
                    actual
                ));
            }
            checked += 1;
        }
    }
    assert!(checked >= 9, "only {checked} fixtures found");
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}
