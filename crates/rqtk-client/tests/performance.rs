//! RQTK-CLIENT-0020: a generated 5,000-requirement repository opens with verification status
//! in under 2 s, and every query on it answers in under 50 ms.
//!
//! The limits are for an optimised build on a GitHub-hosted Linux runner, which is how CI runs
//! this test (`cargo test --release`). Unoptimised builds check the same queries, each against
//! a limit ten times as loose, so a quadratic regression still fails everywhere.

use rqtk_client::{ClosureStatus, Filter, Repository};
use rqtk_macros::verifies;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

const REQUIREMENTS: usize = 5_000;
const SYSTEM: usize = 100;

const CONFIG: &str = r#"schema_version = 1

[project]
name = "large"
version = "0.1.0"

[identification]
prefix = "BIG"
zero_padding = 4

[categories.SYS]
name = "System"
level = 1
is_root = true

[categories.SW]
name = "Software"
level = 2

[types]
allowed = ["Functional"]

[verification]
methods = ["Test"]
levels = ["System"]
phases = ["Development"]

[lifecycle]
states = ["Draft"]
default_state = "Draft"

[priority]
levels = ["High"]

[criticality]
levels = ["Non-Critical"]

[validation]
require_rationale = false
require_verification_method = true
forbid_orphans = false
forbid_circular_traces = true
allow_tbd = false
allow_tbr = false
shall_keywords = ["shall"]
"#;

/// 100 system requirements, each with 49 software children; every requirement has one
/// activity and one linked test; 50 needs and 10 stakeholders.
fn generate(root: &Path) {
    let write = |rel: &str, text: &str| {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    };
    write(".rqtk/config.toml", CONFIG);
    for s in 1..=10 {
        write(
            &format!(".rqtk/stakeholders/STK-{s:03}.toml"),
            &format!("id = \"STK-{s:03}\"\nname = \"Stakeholder {s}\"\n"),
        );
    }
    for n in 1..=50 {
        write(
            &format!(".rqtk/needs/NEED-{n:04}.toml"),
            &format!(
                "id = \"NEED-{n:04}\"\ntitle = \"Need {n}\"\nstate = \"Draft\"\nstatement = \"Users need capability {n}.\"\nstakeholders = [\"STK-{:03}\"]\n",
                (n - 1) % 10 + 1
            ),
        );
    }
    let mut tests = String::new();
    for i in 1..=REQUIREMENTS {
        let (category, trace) = if i <= SYSTEM {
            (
                "SYS",
                format!("[trace]\nsatisfies = [\"NEED-{:04}\"]\n", (i - 1) % 50 + 1),
            )
        } else {
            (
                "SW",
                format!(
                    "[trace]\nparents = [\"BIG-SYS-{:04}\"]\n",
                    (i - 1) % SYSTEM + 1
                ),
            )
        };
        write(
            &format!(".rqtk/requirements/{category}/BIG-{category}-{i:04}.toml"),
            &format!(
                r#"id = "BIG-{category}-{i:04}"
title = "Requirement {i}"
category = "{category}"
type = "Functional"
state = "Draft"
priority = "High"
statement = "The system shall do thing {i}."
{trace}
[verification]
method = "Test"
level = "System"
phase = "Development"
success_criteria = "Thing {i} happens."

[[verification.activities]]
id = "VA-{category}-{i:04}-01"
name = "Thing {i}"
"#
            ),
        );
        let _ = writeln!(
            tests,
            "#[verifies(\"VA-{category}-{i:04}-01\")]\n#[test]\nfn thing_{i}() {{}}\n"
        );
    }
    write("tests/things.rs", &tests);
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args([
            "-c",
            "user.name=Bench",
            "-c",
            "user.email=bench@example.com",
        ])
        .args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}");
}

fn timed<T>(f: impl FnOnce() -> T) -> (T, Duration) {
    let start = Instant::now();
    let value = f();
    (value, start.elapsed())
}

#[verifies("VA-CLIENT-0020-01")]
#[test]
fn a_large_repository_opens_and_answers_quickly() {
    // Optimised builds are held to the requirement; unoptimised ones to ten times it.
    let slack = if cfg!(debug_assertions) { 10 } else { 1 };
    let open_limit = Duration::from_secs(2) * slack;
    let query_limit = Duration::from_millis(50) * slack;

    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("large");
    generate(&root);
    git(&root, &["init", "-q"]);
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "generated"]);

    let (repo, open) = timed(|| Repository::open(&root).unwrap());
    assert_eq!(repo.requirements().len(), REQUIREMENTS);
    assert!(
        open < open_limit,
        "opening took {open:?}, limit {open_limit:?}"
    );

    let filter = Filter {
        status: vec![ClosureStatus::Planned],
        ancestor: Some("BIG-SYS-0001".into()),
        ..Filter::default()
    };
    let mut slow = Vec::new();
    let mut check = |name: &str, query: &mut dyn FnMut()| {
        let ((), took) = timed(query);
        eprintln!("{name:<20} {took:?}");
        if took >= query_limit {
            slow.push(format!("{name} took {took:?}"));
        }
    };
    check("requirements", &mut || drop(repo.requirements()));
    check("needs", &mut || drop(repo.needs()));
    check("stakeholders", &mut || drop(repo.stakeholders()));
    check("activities", &mut || drop(repo.activities()));
    check("requirement", &mut || {
        drop(repo.requirement("BIG-SYS-0001"))
    });
    check("find_requirements", &mut || {
        drop(repo.find_requirements(&filter))
    });
    check("context", &mut || {
        drop(repo.context("BIG-SYS-0001").unwrap())
    });
    check("trace", &mut || drop(repo.trace("BIG-SW-4999").unwrap()));
    check("coverage", &mut || drop(repo.coverage()));
    check("lint", &mut || drop(repo.lint()));
    check("impact", &mut || drop(repo.impact("HEAD").unwrap()));
    check("diff", &mut || drop(repo.diff("HEAD", "HEAD").unwrap()));
    check("history", &mut || {
        drop(repo.history("BIG-SYS-0001").unwrap())
    });
    check("baselines", &mut || drop(repo.baselines().unwrap()));
    eprintln!("{:<20} {open:?}", "open");
    assert!(slow.is_empty(), "limit {query_limit:?}: {slow:?}");
}
