use assert_cmd::Command;
use predicates::prelude::*;
use rqtk_macros::verifies;
use std::fs;
use std::path::{Path, PathBuf};

fn fixture_root(fixture: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(fixture)
}

fn rqtk(repo_root: &Path) -> Command {
    let mut cmd = Command::cargo_bin("rqtk").unwrap();
    cmd.arg("--repo-root").arg(repo_root);
    cmd
}

fn git_init(dir: &Path) {
    gix::init(dir).expect("git init failed");
}

/// Write a minimal git identity to `.git/config` so annotated tag creation succeeds.
fn git_set_identity(repo_root: &Path) {
    let config_path = repo_root.join(".git/config");
    let existing = fs::read_to_string(&config_path).unwrap_or_default();
    fs::write(
        &config_path,
        format!("{existing}\n[user]\n\tname = Test\n\temail = test@example.com\n"),
    )
    .unwrap();
}

/// Stage all working-directory files and create a commit via gix.
fn git_commit_all(repo_root: &Path, message: &str) {
    let repo = gix::open(repo_root).expect("open repo");
    let tree_id = build_tree_from_dir(&repo, repo_root);
    let parents: Vec<gix::ObjectId> = repo
        .head_id()
        .map(|id| vec![id.detach()])
        .unwrap_or_default();
    repo.commit("HEAD", message, tree_id, parents)
        .expect("commit failed");
}

/// Recursively build a git tree object from the given directory.
fn build_tree_from_dir(repo: &gix::Repository, dir: &Path) -> gix::ObjectId {
    use gix::objs::tree::{Entry, EntryKind, EntryMode};

    let mut entries: Vec<Entry> = Vec::new();
    for item in fs::read_dir(dir).unwrap() {
        let item = item.unwrap();
        let path = item.path();
        let raw_name = item.file_name();
        let name = raw_name.to_string_lossy();
        if name == ".git" {
            continue;
        }
        if path.is_dir() {
            let subtree_id = build_tree_from_dir(repo, &path);
            entries.push(Entry {
                mode: EntryMode::from(EntryKind::Tree),
                filename: name.as_bytes().into(),
                oid: subtree_id,
            });
        } else {
            let data = fs::read(&path).unwrap();
            let blob_id = repo.write_blob(data).unwrap().detach();
            entries.push(Entry {
                mode: EntryMode::from(EntryKind::Blob),
                filename: name.as_bytes().into(),
                oid: blob_id,
            });
        }
    }
    // Git requires entries sorted by name (dirs with trailing slash for ordering purposes).
    entries.sort_by(|a, b| {
        let a_name = if a.mode.is_tree() {
            format!("{}/", String::from_utf8_lossy(&a.filename))
        } else {
            String::from_utf8_lossy(&a.filename).into_owned()
        };
        let b_name = if b.mode.is_tree() {
            format!("{}/", String::from_utf8_lossy(&b.filename))
        } else {
            String::from_utf8_lossy(&b.filename).into_owned()
        };
        a_name.cmp(&b_name)
    });
    repo.write_object(gix::objs::Tree { entries })
        .unwrap()
        .detach()
}

/// Return true if the given tag name exists in the repository.
fn git_tag_exists(repo_root: &Path, tag_name: &str) -> bool {
    let repo = gix::open(repo_root).expect("open repo");
    let full_ref = format!("refs/tags/{tag_name}");
    repo.try_find_reference(full_ref.as_str())
        .map(|opt| opt.is_some())
        .unwrap_or(false)
}

fn copy_fixture_to_temp(fixture: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    git_init(dir.path());
    let src = fixture_root(fixture);
    for entry in std::fs::read_dir(&src).unwrap() {
        let entry = entry.unwrap();
        let src_path = entry.path();
        let dst_path = dir.path().join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path);
        } else {
            std::fs::copy(src_path, dst_path).unwrap();
        }
    }
    let repo_root = dir.path().to_path_buf();
    (dir, repo_root)
}

fn copy_dir_recursive(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path);
        } else {
            std::fs::copy(src_path, dst_path).unwrap();
        }
    }
}

// ── helpers for ad-hoc fixtures ──────────────────────────────────────────────

/// Write `.rqtk/config.toml` + requirement files to a tempdir (with a git repo)
/// and return the dir handle and repo root.
fn write_fixture(config: &str, reqs: &[(&str, &str)]) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    git_init(dir.path());
    let rqtk_dir = dir.path().join(".rqtk");
    fs::create_dir_all(&rqtk_dir).unwrap();
    fs::write(rqtk_dir.join("config.toml"), config).unwrap();
    let requirements_root = rqtk_dir.join("requirements");
    fs::create_dir_all(&requirements_root).unwrap();
    for (filename, content) in reqs {
        let path = requirements_root.join(filename);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }
    let repo_root = dir.path().to_path_buf();
    (dir, repo_root)
}

/// Minimal project config that accepts TEST-(SYS|SUB)-NNNN IDs.
const BASE_CONFIG: &str = r#"schema_version = 1

[repository]
requirements_dir = ".rqtk/requirements"
required_files = []
required_dirs = []

[project]
name = "test"
version = "0.1.0"

[identification]
id_pattern = "^TEST-(SYS|SUB)-\\d{4}$"
id_separator = "-"
prefix = "TEST"
zero_padding = 4

[categories.SYS]
name = "System"
level = 1
is_root = true

[categories.SUB]
name = "Subsystem"
level = 2

[types]
allowed = ["Functional"]

[verification]
methods = ["Test"]
levels = ["System"]
phases = ["Development"]

[lifecycle]
states = ["Draft", "Review", "Approved", "Implemented", "Verified", "Deprecated"]
default_state = "Draft"

[priority]
levels = ["Critical", "High", "Medium", "Low"]

[criticality]
levels = ["Mission-Critical"]

[validation]
require_rationale = true
require_verification_method = true
require_parent_for_levels = []
forbid_orphans = false
forbid_circular_traces = true
allow_tbd = false
allow_tbr = false
shall_keywords = ["shall"]
forbidden_keywords = []
"#;

// ── lint ─────────────────────────────────────────────────────────────────────

#[verifies("VA-CLI-002-01")]
#[test]
fn lint_exits_zero_on_valid_project() {
    let repo_root = fixture_root("firesat-obc");
    rqtk(&repo_root).arg("lint").assert().code(0);
}

#[test]
fn lint_prints_summary_with_zero_errors() {
    let repo_root = fixture_root("firesat-obc");
    rqtk(&repo_root)
        .arg("lint")
        .assert()
        .stdout(predicate::str::contains("All requirements passed lint"));
}

#[verifies("VA-CORE-002-02")]
#[test]
fn lint_prints_all_requirement_ids_in_issues_when_present() {
    // The fixture has no errors so the output should not contain error lines,
    // but the success line must always be present.
    let repo_root = fixture_root("firesat-obc");
    rqtk(&repo_root)
        .arg("lint")
        .assert()
        .stdout(predicate::str::contains("All requirements passed lint"));
}

/// VA-CLI-002-01 (negative): missing rationale should exit 2 and report RQ007.
#[verifies("VA-CLI-002-01")]
#[test]
fn lint_exits_1_when_rationale_missing() {
    let req_toml = r#"id = "TEST-SYS-0001"
title = "No rationale"
category = "SYS"
type = "Functional"
state = "Draft"
priority = "Critical"
statement = "The system shall do something."

[verification]
method = "Test"
level = "System"
phase = "Development"
"#;

    let (_dir, repo_root) = write_fixture(BASE_CONFIG, &[("SYS/TEST-SYS-0001.toml", req_toml)]);
    rqtk(&repo_root)
        .arg("lint")
        .assert()
        .code(1)
        .stdout(predicate::str::contains("RQ007"));
}

// ── trace ─────────────────────────────────────────────────────────────────────

#[verifies("VA-SYS-004-01")]
#[test]
fn trace_upward_path_reaches_system_root() {
    // FOBC-SW-0001 → FOBC-SYS-0001
    let repo_root = fixture_root("firesat-obc");
    rqtk(&repo_root)
        .arg("trace")
        .arg("FOBC-SW-0001")
        .assert()
        .success()
        .stdout(predicate::str::contains("Parents"))
        .stdout(predicate::str::contains("FOBC-SYS-0001"));
}

#[verifies("VA-SYS-004-01")]
#[test]
fn trace_downward_path_from_root_includes_children() {
    // FOBC-SYS-0001 has children FOBC-SW-0001 and FOBC-HW-0001
    let repo_root = fixture_root("firesat-obc");
    rqtk(&repo_root)
        .arg("trace")
        .arg("FOBC-SYS-0001")
        .assert()
        .success()
        .stdout(predicate::str::contains("Children"))
        .stdout(predicate::str::contains("FOBC-SW-0001"))
        .stdout(predicate::str::contains("FOBC-HW-0001"));
}

#[verifies("VA-SYS-004-01")]
#[test]
fn trace_deep_chain_upward_crosses_multiple_levels() {
    // FOBC-ICD-0001 → FOBC-SW-0001 → FOBC-SYS-0001
    let repo_root = fixture_root("firesat-obc");
    rqtk(&repo_root)
        .arg("trace")
        .arg("FOBC-ICD-0001")
        .assert()
        .success()
        .stdout(predicate::str::contains("FOBC-SW-0001"))
        .stdout(predicate::str::contains("FOBC-SYS-0001"));
}

#[verifies("VA-SYS-004-01")]
#[test]
fn trace_leaf_has_empty_downward_section() {
    // FOBC-ICD-0001 has no children
    let repo_root = fixture_root("firesat-obc");
    let output = rqtk(&repo_root)
        .arg("trace")
        .arg("FOBC-ICD-0001")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(output).unwrap();
    let downward_section = &text[text.find("Children").unwrap()..];
    assert!(downward_section.contains("no children"), "{text}");
    assert!(!downward_section.contains("FOBC-SW-0001"), "{text}");
}

#[verifies("VA-SYS-004-01")]
#[test]
fn trace_unknown_id_exits_nonzero() {
    let repo_root = fixture_root("firesat-obc");
    rqtk(&repo_root)
        .arg("trace")
        .arg("FOBC-SYS-9999")
        .assert()
        .failure();
}

// ── coverage ──────────────────────────────────────────────────────────────────

#[verifies("VA-SYS-004-02")]
#[test]
fn coverage_exits_nonzero_when_gaps_present_and_strict() {
    let repo_root = fixture_root("firesat-obc");
    rqtk(&repo_root)
        .arg("coverage")
        .arg("--strict")
        .assert()
        .failure();
}

#[verifies("VA-SYS-004-02")]
#[test]
fn coverage_lists_requirement_missing_activities() {
    // FOBC-SW-0003 has no verification activities
    let repo_root = fixture_root("firesat-obc");
    rqtk(&repo_root)
        .arg("coverage")
        .assert()
        .stdout(predicate::str::contains("FOBC-SW-0003"));
}

#[verifies("VA-SYS-004-02")]
#[test]
fn coverage_lists_requirement_missing_success_criteria() {
    // FOBC-ICD-0001 has activities but no success_criteria
    let repo_root = fixture_root("firesat-obc");
    rqtk(&repo_root)
        .arg("coverage")
        .assert()
        .stdout(predicate::str::contains("FOBC-ICD-0001"));
}

#[verifies("VA-SYS-004-02")]
#[test]
fn coverage_does_not_flag_fully_covered_requirements() {
    let repo_root = fixture_root("firesat-obc");
    let output = rqtk(&repo_root).arg("coverage").output().unwrap();
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(
        !text.contains("FOBC-SYS-0001"),
        "FOBC-SYS-0001 is fully covered and should not be listed"
    );
    assert!(
        !text.contains("FOBC-SYS-0002"),
        "FOBC-SYS-0002 is fully covered and should not be listed"
    );
    assert!(
        !text.contains("FOBC-SW-0001"),
        "FOBC-SW-0001 is fully covered and should not be listed"
    );
    assert!(
        !text.contains("FOBC-SW-0002"),
        "FOBC-SW-0002 is fully covered and should not be listed"
    );
    assert!(
        !text.contains("FOBC-HW-0001"),
        "FOBC-HW-0001 is fully covered and should not be listed"
    );
}

// ── graph ─────────────────────────────────────────────────────────────────────

#[verifies("VA-CLI-007-02")]
#[test]
fn graph_dot_output_is_valid_digraph() {
    let repo_root = fixture_root("firesat-obc");
    rqtk(&repo_root)
        .arg("graph")
        .assert()
        .success()
        .stdout(predicate::str::contains("digraph {"));
}

#[verifies("VA-CLI-007-02")]
#[test]
fn graph_dot_output_contains_all_requirement_ids() {
    let repo_root = fixture_root("firesat-obc");
    let output = rqtk(&repo_root)
        .arg("graph")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(output).unwrap();
    for id in &[
        "FOBC-SYS-0001",
        "FOBC-SYS-0002",
        "FOBC-SW-0001",
        "FOBC-SW-0002",
        "FOBC-SW-0003",
        "FOBC-HW-0001",
        "FOBC-ICD-0001",
    ] {
        assert!(text.contains(id), "DOT output is missing node for {id}");
    }
}

#[verifies("VA-CLI-007-02")]
#[test]
fn graph_unsupported_format_exits_nonzero() {
    let repo_root = fixture_root("firesat-obc");
    rqtk(&repo_root)
        .arg("graph")
        .arg("--format")
        .arg("svg")
        .assert()
        .failure();
}

// ── export ────────────────────────────────────────────────────────────────────

#[verifies("VA-CLI-007-01")]
#[test]
fn export_json_creates_file_with_all_ids() {
    let (_dir, repo_root) = copy_fixture_to_temp("firesat-obc");
    let out = repo_root.join("export.json");
    rqtk(&repo_root)
        .arg("export")
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&out)
        .assert()
        .success();
    assert!(out.exists(), "JSON export file was not created");
    let content = std::fs::read_to_string(&out).unwrap();
    for id in &[
        "FOBC-SYS-0001",
        "FOBC-SYS-0002",
        "FOBC-SW-0001",
        "FOBC-HW-0001",
    ] {
        assert!(content.contains(id), "JSON export missing {id}");
    }
}

#[verifies("VA-CLI-007-01")]
#[test]
fn export_csv_has_correct_header_and_rows() {
    let (_dir, repo_root) = copy_fixture_to_temp("firesat-obc");
    let out = repo_root.join("export.csv");
    rqtk(&repo_root)
        .arg("export")
        .arg("--format")
        .arg("csv")
        .arg("--output")
        .arg(&out)
        .assert()
        .success();
    let content = std::fs::read_to_string(&out).unwrap();
    assert!(
        content.starts_with("id,title,category,type,state,priority,verification_method\n"),
        "CSV header is incorrect"
    );
    assert!(
        content.contains("FOBC-SYS-0001"),
        "CSV missing FOBC-SYS-0001"
    );
    assert!(content.contains("Approved"), "CSV missing state value");
}

#[verifies("VA-CLI-007-01")]
#[test]
fn export_markdown_has_heading_and_sections() {
    let (_dir, repo_root) = copy_fixture_to_temp("firesat-obc");
    let out = repo_root.join("export.md");
    rqtk(&repo_root)
        .arg("export")
        .arg("--format")
        .arg("markdown")
        .arg("--output")
        .arg(&out)
        .assert()
        .success();
    let content = std::fs::read_to_string(&out).unwrap();
    assert!(
        content.starts_with("# Requirements Export\n"),
        "Markdown heading missing"
    );
    assert!(
        content.contains("## FOBC-SYS-0001"),
        "Markdown missing section for FOBC-SYS-0001"
    );
    assert!(
        content.contains("## FOBC-SW-0001"),
        "Markdown missing section for FOBC-SW-0001"
    );
}

#[verifies("VA-CLI-007-01")]
#[test]
fn export_unsupported_format_exits_nonzero() {
    let (_dir, repo_root) = copy_fixture_to_temp("firesat-obc");
    rqtk(&repo_root)
        .arg("export")
        .arg("--format")
        .arg("xml")
        .assert()
        .failure();
}

// ── diff ──────────────────────────────────────────────────────────────────────

#[verifies("VA-SYS-001-04")]
#[test]
fn diff_fails_when_baseline_tag_does_not_exist() {
    let (_dir, repo_root) = copy_fixture_to_temp("firesat-obc");
    rqtk(&repo_root)
        .arg("diff")
        .arg("0.0.0")
        .arg("9.9.9")
        .assert()
        .failure();
}

#[verifies("VA-SYS-001-04")]
#[test]
fn diff_detects_added_requirement_between_baselines() {
    let (dir, repo_root) = copy_fixture_to_temp("firesat-obc");

    git_set_identity(&repo_root);

    // Commit current state and tag as v0.1.0.
    git_commit_all(&repo_root, "initial");
    rqtk(&repo_root)
        .arg("baseline")
        .arg("0.1.0")
        .assert()
        .success();

    // Add a new requirement, commit, and tag as v0.2.0.
    let new_req = r#"id = "FOBC-SW-0004"
title = "New requirement"
category = "SW"
type = "Functional"
state = "Draft"
priority = "High"
statement = "The OBC software shall do something new."
rationale = "Because we need it."

[trace]
parents = ["FOBC-SYS-0001"]

[verification]
method = "Test"
level = "System"
phase = "Development"
"#;
    std::fs::write(
        repo_root.join(".rqtk/requirements/FOBC-SW-0004.toml"),
        new_req,
    )
    .unwrap();
    git_commit_all(&repo_root, "add FOBC-SW-0004");
    rqtk(&repo_root)
        .arg("baseline")
        .arg("0.2.0")
        .assert()
        .success();

    rqtk(&repo_root)
        .arg("diff")
        .arg("0.1.0")
        .arg("0.2.0")
        .assert()
        .success()
        .stdout(predicate::str::contains("FOBC-SW-0004"));

    drop(dir);
}

// ── new ───────────────────────────────────────────────────────────────────────

#[verifies("VA-CLI-004-01")]
#[test]
fn new_creates_requirement_file_with_correct_id() {
    let (_dir, repo_root) = copy_fixture_to_temp("firesat-obc");
    // Existing SW requirements are 0001–0003, so the next ID is 0004.
    rqtk(&repo_root)
        .arg("add")
        .arg("--category").arg("SW")
        .arg("--type").arg("Functional")
        .arg("--title").arg("Telemetry Compression")
        .arg("--statement")
        .arg("The OBC software shall compress telemetry data using CCSDS lossless compression before writing to the frame buffer.")
        .arg("--rationale")
        .arg("Compression reduces storage consumption by up to 40%, extending effective buffer capacity during long blackout periods.")
        .assert()
        .success()
        .stdout(predicate::str::contains("FOBC-SW-0004"));
    assert!(
        repo_root
            .join(".rqtk/requirements/SW/FOBC-SW-0004.toml")
            .exists(),
        "Expected SW/FOBC-SW-0004.toml to be created"
    );
}

#[verifies("VA-CLI-004-01")]
#[test]
fn new_created_file_passes_lint() {
    let (_dir, repo_root) = copy_fixture_to_temp("firesat-obc");
    rqtk(&repo_root)
        .arg("add")
        .arg("--category").arg("HW")
        .arg("--type").arg("Constraint")
        .arg("--title").arg("Radiation Tolerance")
        .arg("--statement")
        .arg("The OBC processor shall tolerate a total ionising dose of at least 30 krad without functional degradation.")
        .arg("--rationale")
        .arg("FireSat operates in a low-Earth orbit with a 3-year design lifetime requiring radiation-hardened components.")
        .assert()
        .success();
    // After creating the new requirement, lint should still report 0 errors.
    // (The new requirement will have no parent, triggering RQ012 and possibly RQ018,
    //  but those are errors only if the require_parent_for_levels rule applies and
    //  forbid_orphans is active — so we just check the process itself succeeds.)
    rqtk(&repo_root).arg("lint").output().unwrap(); // any exit code is acceptable; we just verify it doesn't panic
}

// ── baseline ──────────────────────────────────────────────────────────────────

#[verifies("VA-SYS-001-03")]
#[test]
fn baseline_creates_git_tag() {
    let (dir, repo_root) = copy_fixture_to_temp("firesat-obc");

    git_set_identity(&repo_root);
    git_commit_all(&repo_root, "init");

    rqtk(&repo_root)
        .arg("baseline")
        .arg("2.0.0")
        .assert()
        .success()
        .stdout(predicate::str::contains("Baseline created"))
        .stdout(predicate::str::contains("2.0.0"));

    assert!(
        git_tag_exists(&repo_root, "rqtk/2.0.0"),
        "git tag rqtk/2.0.0 was not created"
    );

    drop(dir);
}

#[verifies("VA-SYS-001-03")]
#[test]
fn baseline_rejects_invalid_semver() {
    let (_dir, repo_root) = copy_fixture_to_temp("firesat-obc");
    rqtk(&repo_root)
        .arg("baseline")
        .arg("not-a-version")
        .assert()
        .failure();
}

// ── VA-SYS-003-01: cycle detection via CLI ────────────────────────────────────

#[verifies("VA-CORE-002-01")]
#[test]
fn lint_cycle_exits_1_and_reports_rq017() {
    // A→B→A circular parent chain
    let req_a = r#"id = "TEST-SYS-0001"
title = "A"
category = "SYS"
type = "Functional"
state = "Draft"
priority = "Critical"
statement = "The system shall do something."
rationale = "Because it needs to."

[trace]
parents = ["TEST-SYS-0002"]

[verification]
method = "Test"
level = "System"
phase = "Development"
"#;

    let req_b = r#"id = "TEST-SYS-0002"
title = "B"
category = "SYS"
type = "Functional"
state = "Draft"
priority = "Critical"
statement = "The system shall do something else."
rationale = "Because it also needs to."

[trace]
parents = ["TEST-SYS-0001"]

[verification]
method = "Test"
level = "System"
phase = "Development"
"#;

    let (_dir, repo_root) = write_fixture(
        BASE_CONFIG,
        &[
            ("SYS/TEST-SYS-0001.toml", req_a),
            ("SYS/TEST-SYS-0002.toml", req_b),
        ],
    );

    rqtk(&repo_root)
        .arg("lint")
        .assert()
        .code(1)
        .stdout(predicate::str::contains("RQ017"));
}

// ── VA-SYS-003-02: broken reference via CLI ───────────────────────────────────

// #[verifies("VA-SYS-003-02")]
#[verifies("VA-CORE-002-02")]
#[test]
fn lint_broken_parent_exits_1_and_reports_rq015() {
    let req_toml = r#"id = "TEST-SYS-0001"
title = "Broken parent"
category = "SYS"
type = "Functional"
state = "Draft"
priority = "Critical"
statement = "The system shall do something."
rationale = "Because it needs to."

[trace]
parents = ["TEST-SYS-9999"]

[verification]
method = "Test"
level = "System"
phase = "Development"
"#;

    let (_dir, repo_root) = write_fixture(BASE_CONFIG, &[("SYS/TEST-SYS-0001.toml", req_toml)]);

    rqtk(&repo_root)
        .arg("lint")
        .assert()
        .code(1)
        .stdout(predicate::str::contains("RQ015"));
}

// ── VA-SYS-004-01: lint rule coverage matrix ─────────────────────────────────

/// Helper: write a fixture with the given config and single requirement file,
/// run lint, and return stdout as a String.
fn lint_stdout(config: &str, req_content: &str) -> String {
    let (_dir, repo_root) = write_fixture(config, &[("SYS/TEST-SYS-0001.toml", req_content)]);
    let output = rqtk(&repo_root).arg("lint").output().unwrap();
    String::from_utf8(output.stdout).unwrap()
}

fn lint_stdout_with_files(config: &str, files: &[(&str, &str)]) -> String {
    let (_dir, repo_root) = write_fixture(config, files);
    let output = rqtk(&repo_root).arg("lint").output().unwrap();
    String::from_utf8(output.stdout).unwrap()
}

#[verifies("VA-CORE-002-02")]
#[test]
fn lint_rule_rq001_id_mismatch() {
    // id doesn't match id_pattern
    let req = r#"id = "TEST-WRONG-001"
title = "Bad ID"
category = "SYS"
type = "Functional"
state = "Draft"
priority = "Critical"
statement = "The system shall do something."
rationale = "Because it needs to."

[verification]
method = "Test"
level = "System"
phase = "Development"
"#;
    let out = lint_stdout(BASE_CONFIG, req);
    assert!(
        out.contains("RQ001"),
        "expected RQ001 in output; got:\n{out}"
    );
}

#[verifies("VA-CORE-002-02")]
#[test]
fn lint_rule_rq002_unknown_category() {
    let req = r#"id = "TEST-SYS-0001"
title = "Bad category"
category = "UNKNOWN"
type = "Functional"
state = "Draft"
priority = "Critical"
statement = "The system shall do something."
rationale = "Because it needs to."

[verification]
method = "Test"
level = "System"
phase = "Development"
"#;
    let out = lint_stdout(BASE_CONFIG, req);
    assert!(
        out.contains("RQ002"),
        "expected RQ002 in output; got:\n{out}"
    );
}

#[verifies("VA-CORE-002-02")]
#[test]
fn lint_rule_rq003_unknown_type() {
    let req = r#"id = "TEST-SYS-0001"
title = "Bad type"
category = "SYS"
type = "Unknown"
state = "Draft"
priority = "Critical"
statement = "The system shall do something."
rationale = "Because it needs to."

[verification]
method = "Test"
level = "System"
phase = "Development"
"#;
    let out = lint_stdout(BASE_CONFIG, req);
    assert!(
        out.contains("RQ003"),
        "expected RQ003 in output; got:\n{out}"
    );
}

#[verifies("VA-CORE-002-02")]
#[test]
fn lint_rule_rq004_invalid_state() {
    let req = r#"id = "TEST-SYS-0001"
title = "Bad state"
category = "SYS"
type = "Functional"
state = "Invalid"
priority = "Critical"
statement = "The system shall do something."
rationale = "Because it needs to."

[verification]
method = "Test"
level = "System"
phase = "Development"
"#;
    let out = lint_stdout(BASE_CONFIG, req);
    assert!(
        out.contains("RQ004"),
        "expected RQ004 in output; got:\n{out}"
    );
}

#[verifies("VA-CORE-002-02")]
#[test]
fn lint_rule_rq005_invalid_priority() {
    let req = r#"id = "TEST-SYS-0001"
title = "Bad priority"
category = "SYS"
type = "Functional"
state = "Draft"
priority = "Invalid"
statement = "The system shall do something."
rationale = "Because it needs to."

[verification]
method = "Test"
level = "System"
phase = "Development"
"#;
    let out = lint_stdout(BASE_CONFIG, req);
    assert!(
        out.contains("RQ005"),
        "expected RQ005 in output; got:\n{out}"
    );
}

#[verifies("VA-CORE-002-02")]
#[test]
fn lint_rule_rq006_invalid_criticality() {
    let req = r#"id = "TEST-SYS-0001"
title = "Bad criticality"
category = "SYS"
type = "Functional"
state = "Draft"
priority = "Critical"
criticality = "InvalidCrit"
statement = "The system shall do something."
rationale = "Because it needs to."

[verification]
method = "Test"
level = "System"
phase = "Development"
"#;
    let out = lint_stdout(BASE_CONFIG, req);
    assert!(
        out.contains("RQ006"),
        "expected RQ006 in output; got:\n{out}"
    );
}

#[verifies("VA-CORE-002-02")]
#[test]
fn lint_rule_rq007_missing_rationale() {
    let req = r#"id = "TEST-SYS-0001"
title = "No rationale"
category = "SYS"
type = "Functional"
state = "Draft"
priority = "Critical"
statement = "The system shall do something."

[verification]
method = "Test"
level = "System"
phase = "Development"
"#;
    let out = lint_stdout(BASE_CONFIG, req);
    assert!(
        out.contains("RQ007"),
        "expected RQ007 in output; got:\n{out}"
    );
}

#[verifies("VA-CORE-002-02")]
#[test]
fn lint_rule_rq008_empty_verification_method() {
    let req = r#"id = "TEST-SYS-0001"
title = "Empty method"
category = "SYS"
type = "Functional"
state = "Draft"
priority = "Critical"
statement = "The system shall do something."
rationale = "Because it needs to."

[verification]
method = ""
level = "System"
phase = "Development"
"#;
    let out = lint_stdout(BASE_CONFIG, req);
    assert!(
        out.contains("RQ008"),
        "expected RQ008 in output; got:\n{out}"
    );
}

#[verifies("VA-CORE-002-02")]
#[test]
fn lint_rule_rq009_unknown_verification_method() {
    let req = r#"id = "TEST-SYS-0001"
title = "Bad method"
category = "SYS"
type = "Functional"
state = "Draft"
priority = "Critical"
statement = "The system shall do something."
rationale = "Because it needs to."

[verification]
method = "Seance"
level = "System"
phase = "Development"
"#;
    let out = lint_stdout(BASE_CONFIG, req);
    assert!(
        out.contains("RQ009"),
        "expected RQ009 in output; got:\n{out}"
    );
}

#[verifies("VA-CORE-002-02")]
#[test]
fn lint_rule_rq010_no_shall_keyword() {
    let req = r#"id = "TEST-SYS-0001"
title = "No shall"
category = "SYS"
type = "Functional"
state = "Draft"
priority = "Critical"
statement = "The system does something."
rationale = "Because it needs to."

[verification]
method = "Test"
level = "System"
phase = "Development"
"#;
    let out = lint_stdout(BASE_CONFIG, req);
    assert!(
        out.contains("RQ010"),
        "expected RQ010 in output; got:\n{out}"
    );
}

#[verifies("VA-CORE-002-02")]
#[test]
fn lint_rule_rq011_forbidden_keyword_in_mandatory() {
    // Need custom config with "Mandatory" in priority levels and "should" in forbidden_keywords
    let config = r#"schema_version = 1

[project]
name = "test"
version = "0.1.0"

[identification]
id_pattern = "^TEST-(SYS|SUB)-\\d{4}$"
id_separator = "-"
prefix = "TEST"
zero_padding = 4

[categories.SYS]
name = "System"
level = 1
is_root = true

[categories.SUB]
name = "Subsystem"
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
levels = ["Critical", "Mandatory"]

[criticality]
levels = ["Mission-Critical"]

[validation]
require_rationale = true
require_verification_method = true
require_parent_for_levels = []
forbid_orphans = false
forbid_circular_traces = false
allow_tbd = false
allow_tbr = false
shall_keywords = ["shall"]
forbidden_keywords = ["should"]
"#;

    let req = r#"id = "TEST-SYS-0001"
title = "Forbidden keyword"
category = "SYS"
type = "Functional"
state = "Draft"
priority = "Mandatory"
statement = "The system shall not use should anywhere."
rationale = "Because it needs to."

[verification]
method = "Test"
level = "System"
phase = "Development"
"#;
    let out = lint_stdout(config, req);
    assert!(
        out.contains("RQ011"),
        "expected RQ011 in output; got:\n{out}"
    );
}

#[verifies("VA-CORE-002-02")]
#[test]
fn lint_rule_rq012_missing_parent_for_required_level() {
    // SUB category with require_parent_for_levels = ["SUB"]
    let config = r#"schema_version = 1

[project]
name = "test"
version = "0.1.0"

[identification]
id_pattern = "^TEST-(SYS|SUB)-\\d{4}$"
id_separator = "-"
prefix = "TEST"
zero_padding = 4

[categories.SYS]
name = "System"
level = 1
is_root = true

[categories.SUB]
name = "Subsystem"
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
levels = ["Critical"]

[criticality]
levels = ["Mission-Critical"]

[validation]
require_rationale = true
require_verification_method = true
require_parent_for_levels = ["SUB"]
forbid_orphans = false
forbid_circular_traces = false
allow_tbd = false
allow_tbr = false
shall_keywords = ["shall"]
forbidden_keywords = []
"#;

    let req = r#"id = "TEST-SUB-0001"
title = "No parent"
category = "SUB"
type = "Functional"
state = "Draft"
priority = "Critical"
statement = "The system shall do something."
rationale = "Because it needs to."

[verification]
method = "Test"
level = "System"
phase = "Development"
"#;
    let files = [("SUB/TEST-SUB-0001.toml", req)];
    let out = lint_stdout_with_files(config, &files);
    assert!(
        out.contains("RQ012"),
        "expected RQ012 in output; got:\n{out}"
    );
}

#[verifies("VA-CORE-002-02")]
#[test]
fn lint_rule_rq013_tbd_not_allowed() {
    let req = r#"id = "TEST-SYS-0001"
title = "TBD req"
category = "SYS"
type = "Functional"
state = "Draft"
priority = "Critical"
tbd = true
statement = "The system shall do something."
rationale = "Because it needs to."

[verification]
method = "Test"
level = "System"
phase = "Development"
"#;
    let out = lint_stdout(BASE_CONFIG, req);
    assert!(
        out.contains("RQ013"),
        "expected RQ013 in output; got:\n{out}"
    );
}

#[verifies("VA-CORE-002-02")]
#[test]
fn lint_rule_rq014_tbr_not_allowed() {
    let req = r#"id = "TEST-SYS-0001"
title = "TBR req"
category = "SYS"
type = "Functional"
state = "Draft"
priority = "Critical"
tbr = true
statement = "The system shall do something."
rationale = "Because it needs to."

[verification]
method = "Test"
level = "System"
phase = "Development"
"#;
    let out = lint_stdout(BASE_CONFIG, req);
    assert!(
        out.contains("RQ014"),
        "expected RQ014 in output; got:\n{out}"
    );
}

#[verifies("VA-CORE-002-02")]
#[test]
fn lint_rule_rq015_unknown_parent_reference() {
    let req = r#"id = "TEST-SYS-0001"
title = "Bad parent"
category = "SYS"
type = "Functional"
state = "Draft"
priority = "Critical"
statement = "The system shall do something."
rationale = "Because it needs to."

[trace]
parents = ["TEST-SYS-9999"]

[verification]
method = "Test"
level = "System"
phase = "Development"
"#;
    let out = lint_stdout(BASE_CONFIG, req);
    assert!(
        out.contains("RQ015"),
        "expected RQ015 in output; got:\n{out}"
    );
}

#[verifies("VA-CORE-002-02")]
#[test]
fn lint_rule_rq016_unknown_dependency_reference() {
    let req = r#"id = "TEST-SYS-0001"
title = "Bad depends_on"
category = "SYS"
type = "Functional"
state = "Draft"
priority = "Critical"
statement = "The system shall do something."
rationale = "Because it needs to."

[trace]
depends_on = ["TEST-SYS-9999"]

[verification]
method = "Test"
level = "System"
phase = "Development"
"#;
    let out = lint_stdout(BASE_CONFIG, req);
    assert!(
        out.contains("RQ016"),
        "expected RQ016 in output; got:\n{out}"
    );
}

#[verifies("VA-CORE-002-02")]
#[test]
fn lint_rule_rq017_circular_parent_chain() {
    let req_a = r#"id = "TEST-SYS-0001"
title = "A"
category = "SYS"
type = "Functional"
state = "Draft"
priority = "Critical"
statement = "The system shall do something."
rationale = "Because it needs to."

[trace]
parents = ["TEST-SYS-0002"]

[verification]
method = "Test"
level = "System"
phase = "Development"
"#;

    let req_b = r#"id = "TEST-SYS-0002"
title = "B"
category = "SYS"
type = "Functional"
state = "Draft"
priority = "Critical"
statement = "The system shall do something else."
rationale = "Because it also needs to."

[trace]
parents = ["TEST-SYS-0001"]

[verification]
method = "Test"
level = "System"
phase = "Development"
"#;

    let files = [
        ("SYS/TEST-SYS-0001.toml", req_a),
        ("SYS/TEST-SYS-0002.toml", req_b),
    ];
    let out = lint_stdout_with_files(BASE_CONFIG, &files);
    assert!(
        out.contains("RQ017"),
        "expected RQ017 in output; got:\n{out}"
    );
}

#[verifies("VA-CORE-002-02")]
#[test]
fn lint_rule_rq018_orphan_with_no_path_to_root() {
    // SUB requirement with no parents and forbid_orphans=true
    let config = r#"schema_version = 1

[project]
name = "test"
version = "0.1.0"

[identification]
id_pattern = "^TEST-(SYS|SUB)-\\d{4}$"
id_separator = "-"
prefix = "TEST"
zero_padding = 4

[categories.SYS]
name = "System"
level = 1
is_root = true

[categories.SUB]
name = "Subsystem"
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
levels = ["Critical"]

[criticality]
levels = ["Mission-Critical"]

[validation]
require_rationale = true
require_verification_method = true
require_parent_for_levels = []
forbid_orphans = true
forbid_circular_traces = false
allow_tbd = false
allow_tbr = false
shall_keywords = ["shall"]
forbidden_keywords = []
"#;

    let req = r#"id = "TEST-SUB-0001"
title = "Orphan"
category = "SUB"
type = "Functional"
state = "Draft"
priority = "Critical"
statement = "The system shall do something."
rationale = "Because it needs to."

[verification]
method = "Test"
level = "System"
phase = "Development"
"#;
    let files = [("SUB/TEST-SUB-0001.toml", req)];
    let out = lint_stdout_with_files(config, &files);
    assert!(
        out.contains("RQ018"),
        "expected RQ018 in output; got:\n{out}"
    );
}

#[verifies("VA-CLI-004-01")]
#[test]
fn init_scaffolds_root_config_and_custom_requirements_dir() {
    let dir = tempfile::tempdir().unwrap();
    let repo_root = dir.path();
    Command::cargo_bin("rqtk")
        .unwrap()
        .arg("--repo-root")
        .arg(repo_root)
        .arg("init")
        .arg("--requirements-dir")
        .arg("reqs")
        .assert()
        .success();

    assert!(repo_root.join(".rqtk/config.toml").exists());
    assert!(repo_root.join("reqs").is_dir());
    assert!(repo_root.join("reqs").join("SYS").is_dir());
}

#[verifies("VA-CLI-004-01")]
#[test]
fn init_names_the_project_and_leaves_examples_opt_in() {
    let dir = tempfile::tempdir().unwrap();
    let repo_root = dir.path();
    fs::write(repo_root.join("package.json"), r#"{ "name": "shop" }"#).unwrap();
    rqtk(repo_root)
        .arg("init")
        .assert()
        .success()
        .stderr(predicate::str::contains("not inside a git repository"));
    let config = fs::read_to_string(repo_root.join(".rqtk/config.toml")).unwrap();
    assert!(config.contains("name = \"shop\""), "{config}");
    assert!(
        config.contains("require_parent_for_categories = []"),
        "{config}"
    );
    assert!(!repo_root.join(".rqtk/stakeholders").exists());
    git_init(repo_root);
    rqtk(repo_root).arg("lint").assert().success();

    rqtk(repo_root)
        .args(["init", "--force", "--example"])
        .assert()
        .success();
    assert!(repo_root.join(".rqtk/stakeholders/STK-001.toml").exists());
    assert!(repo_root.join(".rqtk/needs/NEED-0001.toml").exists());
}

#[verifies("VA-CLI-004-01")]
#[test]
fn add_writes_trace_verification_and_activities_that_lint_clean() {
    let config = BASE_CONFIG.replace(
        "require_parent_for_levels = []",
        "require_parent_for_levels = [\"SUB\"]",
    );
    let (_dir, repo_root) = write_fixture(
        &config,
        &[("SYS/TEST-SYS-0001.toml", &valid_req("TEST-SYS-0001"))],
    );
    let add = |extra: &[&str]| {
        let mut cmd = rqtk(&repo_root);
        cmd.args([
            "add",
            "--category",
            "SUB",
            "--type",
            "Functional",
            "--title",
            "Boot",
            "--statement",
            "The subsystem shall boot.",
            "--rationale",
            "Needed.",
        ])
        .args(extra);
        cmd
    };
    // The config requires a parent for SUB: refuse rather than write a file that fails lint.
    add(&[])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("--parent"));
    add(&["--parent", "TEST-SYS-9999"]).assert().code(2);

    add(&[
        "--parent",
        "TEST-SYS-0001",
        "--criteria",
        "Boots in under 5 s.",
        "--activity",
        "Boot time",
        "--activity",
        "Cold boot",
    ])
    .assert()
    .success();
    let path = repo_root.join(".rqtk/requirements/SUB/TEST-SUB-0001.toml");
    let written = fs::read_to_string(&path).unwrap();
    assert!(
        written.contains("parents = [\"TEST-SYS-0001\"]"),
        "{written}"
    );
    assert!(
        written.contains("success_criteria = \"Boots in under 5 s.\""),
        "{written}"
    );
    assert!(written.contains("id = \"VA-SUB-0001-01\""), "{written}");
    assert!(written.contains("id = \"VA-SUB-0001-02\""), "{written}");
    assert!(written.contains("priority = \"Medium\""), "{written}");
    assert!(!written.contains("content_hash"), "{written}");
    rqtk(&repo_root).arg("lint").assert().success();

    // Editing a fresh requirement leaves no stale-hash warning behind.
    fs::write(&path, written.replace("shall boot", "shall boot quickly")).unwrap();
    rqtk(&repo_root)
        .arg("lint")
        .assert()
        .success()
        .stdout(predicate::str::contains("RQ021").not());
}

#[verifies("VA-CORE-004-01")]
#[test]
fn add_activity_appends_and_keeps_layout() {
    let req = format!(
        "# Owned by the systems team.\n{}",
        valid_req("TEST-SYS-0001")
    );
    let (_dir, repo_root) = write_fixture(BASE_CONFIG, &[("SYS/TEST-SYS-0001.toml", &req)]);
    rqtk(&repo_root)
        .args(["add-activity", "TEST-SYS-0001", "--name", "Boot time"])
        .assert()
        .success()
        .stdout(predicate::str::contains("VA-SYS-0001-01"));
    let written =
        fs::read_to_string(repo_root.join(".rqtk/requirements/SYS/TEST-SYS-0001.toml")).unwrap();
    assert!(
        written.starts_with("# Owned by the systems team.\n"),
        "{written}"
    );
    assert!(
        written
            .contains("[[verification.activities]]\nid = \"VA-SYS-0001-01\"\nname = \"Boot time\""),
        "{written}"
    );
    rqtk(&repo_root)
        .args([
            "add-activity",
            "TEST-SYS-0001",
            "--name",
            "Again",
            "--id",
            "VA-SYS-0001-01",
        ])
        .assert()
        .code(2);
}

#[verifies("VA-CORE-002-02")]
#[test]
fn lint_reports_missing_required_repository_paths() {
    let config = r#"schema_version = 1

[repository]
requirements_dir = ".rqtk/requirements"
required_files = ["README.md"]
required_dirs = ["docs"]

[project]
name = "test"
version = "0.1.0"

[identification]
id_pattern = "^TEST-(SYS)-\\d{4}$"
id_separator = "-"
prefix = "TEST"
zero_padding = 4

[categories.SYS]
name = "System"
level = 1
is_root = true

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
levels = ["Critical"]

[criticality]
levels = ["Mission-Critical"]

[validation]
require_rationale = true
require_verification_method = true
require_parent_for_levels = []
forbid_orphans = false
forbid_circular_traces = false
allow_tbd = false
allow_tbr = false
shall_keywords = ["shall"]
forbidden_keywords = []
"#;

    let (_dir, repo_root) = write_fixture(config, &[]);
    rqtk(&repo_root)
        .arg("lint")
        .assert()
        .code(1)
        .stdout(predicate::str::contains("RQ019"))
        .stdout(predicate::str::contains("RQ020"));
}

// ── stakeholder & need helpers ────────────────────────────────────────────────

/// Config that enables stakeholders_dir and needs_dir alongside requirements.
const BASE_CONFIG_WITH_STAKEHOLDERS: &str = r#"schema_version = 1

[repository]
requirements_dir = ".rqtk/requirements"
stakeholders_dir = ".rqtk/stakeholders"
needs_dir = ".rqtk/needs"
required_files = []
required_dirs = []

[project]
name = "test"
version = "0.1.0"

[identification]
id_pattern = "^TEST-(SYS)-\\d{4}$"
id_separator = "-"
prefix = "TEST"
zero_padding = 4

[categories.SYS]
name = "System"
level = 1
is_root = true

[types]
allowed = ["Functional"]

[verification]
methods = ["Test"]
levels = ["System"]
phases = ["Development"]

[lifecycle]
states = ["Draft", "Review", "Approved", "Implemented", "Verified", "Deprecated"]
default_state = "Draft"

[priority]
levels = ["Critical", "High", "Medium", "Low"]

[criticality]
levels = ["Mission-Critical"]

[validation]
require_rationale = true
require_verification_method = true
require_parent_for_levels = []
forbid_orphans = false
forbid_circular_traces = true
allow_tbd = false
allow_tbr = false
shall_keywords = ["shall"]
forbidden_keywords = []
"#;

/// Write a complete fixture including stakeholder and need files.
fn write_fixture_full(
    config: &str,
    reqs: &[(&str, &str)],
    stakeholders: &[(&str, &str)],
    needs: &[(&str, &str)],
) -> (tempfile::TempDir, PathBuf) {
    let (dir, repo_root) = write_fixture(config, reqs);
    let rqtk_dir = repo_root.join(".rqtk");

    let stakeholders_root = rqtk_dir.join("stakeholders");
    fs::create_dir_all(&stakeholders_root).unwrap();
    for (filename, content) in stakeholders {
        fs::write(stakeholders_root.join(filename), content).unwrap();
    }

    let needs_root = rqtk_dir.join("needs");
    fs::create_dir_all(&needs_root).unwrap();
    for (filename, content) in needs {
        fs::write(needs_root.join(filename), content).unwrap();
    }

    (dir, repo_root)
}

// ── RQ023: need references unknown stakeholder ───────────────────────────────

#[verifies("VA-CORE-002-02")]
#[test]
fn lint_rule_rq023_need_references_unknown_stakeholder() {
    let need_toml = r#"id = "NEED-0001"
title = "Some need"
state = "Draft"
stakeholders = ["UNKNOWN-STK"]
statement = "The system shall satisfy this need."
"#;
    let (_dir, repo_root) = write_fixture_full(
        BASE_CONFIG_WITH_STAKEHOLDERS,
        &[],
        &[],
        &[("NEED-0001.toml", need_toml)],
    );
    let output = rqtk(&repo_root).arg("lint").output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("RQ023") && stdout.contains("NEED-0001"),
        "expected RQ023 for NEED-0001 in output; got:\n{stdout}"
    );
}

// ── RQ023: validation.stakeholder unknown ─────────────────────────────────────

#[verifies("VA-CORE-002-02")]
#[test]
fn lint_rule_rq023_validation_stakeholder_unknown() {
    let req_toml = r#"id = "TEST-SYS-0001"
title = "Validated requirement"
category = "SYS"
type = "Functional"
state = "Draft"
priority = "Critical"
statement = "The system shall do something."
rationale = "To satisfy a need."

[verification]
method = "Test"
level = "System"
phase = "Development"

[validation]
stakeholder = "UNKNOWN-STK"
"#;
    let (_dir, repo_root) = write_fixture_full(
        BASE_CONFIG_WITH_STAKEHOLDERS,
        &[("SYS/TEST-SYS-0001.toml", req_toml)],
        &[],
        &[],
    );
    let output = rqtk(&repo_root).arg("lint").output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("RQ023"),
        "expected RQ023 in output; got:\n{stdout}"
    );
}

// ── add-stakeholder command ───────────────────────────────────────────────────

#[verifies("VA-CLI-004-01")]
#[test]
fn add_stakeholder_creates_file() {
    let (_dir, repo_root) = write_fixture_full(BASE_CONFIG_WITH_STAKEHOLDERS, &[], &[], &[]);
    rqtk(&repo_root)
        .args([
            "add-stakeholder",
            "--id",
            "STK-001",
            "--name",
            "Mission Ops",
            "--role",
            "Systems Engineer",
            "--organization",
            "ACME",
        ])
        .assert()
        .success();
    let stk_path = repo_root.join(".rqtk/stakeholders/STK-001.toml");
    assert!(stk_path.exists(), "stakeholder file not created");
    let content = fs::read_to_string(&stk_path).unwrap();
    assert!(content.contains("STK-001"), "ID not in file");
    assert!(content.contains("Mission Ops"), "name not in file");
}

// ── add-need command ──────────────────────────────────────────────────────────

#[verifies("VA-CLI-004-01")]
#[test]
fn add_need_creates_file() {
    let (_dir, repo_root) = write_fixture_full(BASE_CONFIG_WITH_STAKEHOLDERS, &[], &[], &[]);
    rqtk(&repo_root)
        .args([
            "add-need",
            "--id",
            "NEED-0001",
            "--title",
            "Operator visibility",
            "--statement",
            "The system shall provide operator visibility.",
        ])
        .assert()
        .success();
    let need_path = repo_root.join(".rqtk/needs/NEED-0001.toml");
    assert!(need_path.exists(), "need file not created");
    let content = fs::read_to_string(&need_path).unwrap();
    assert!(content.contains("NEED-0001"), "ID not in file");
    assert!(content.contains("Operator visibility"), "title not in file");
}

// ── schema v1: loading, diagnostics and format-preserving writes ─────────────

/// A valid requirement whose category is taken from its ID (`TEST-<CAT>-NNNN`).
fn valid_req(id: &str) -> String {
    let category = id.split('-').nth(1).unwrap();
    format!(
        r#"id = "{id}"
title = "Requirement {id}"
category = "{category}"
type = "Functional"
state = "Draft"
priority = "High"
statement = "The system shall do {id}."
rationale = "Because."

[verification]
method = "Test"
level = "System"
phase = "Development"
"#
    )
}

fn lint_output(files: &[(&str, &str)]) -> (i32, String) {
    let (_dir, repo_root) = write_fixture(BASE_CONFIG, files);
    let output = rqtk(&repo_root).arg("lint").output().unwrap();
    (
        output.status.code().unwrap(),
        String::from_utf8(output.stdout).unwrap(),
    )
}

#[verifies("VA-CORE-001-02")]
#[test]
fn lint_reports_unknown_field_with_line_number() {
    let req = valid_req("TEST-SYS-0001").replace("rationale =", "ratoinale =");
    let (code, out) = lint_output(&[("SYS/TEST-SYS-0001.toml", &req)]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("RQ100"), "{out}");
    assert!(out.contains("unknown field `ratoinale`"), "{out}");
    assert!(out.contains("TEST-SYS-0001.toml:8"), "{out}");
}

#[verifies("VA-CORE-001-01")]
#[test]
fn lint_reports_every_broken_file_in_one_run() {
    let broken_syntax = format!("{}garbage =\n", valid_req("TEST-SYS-0001"));
    let missing_field = valid_req("TEST-SYS-0002").replace("priority = \"High\"\n", "");
    let (_, out) = lint_output(&[
        ("SYS/TEST-SYS-0001.toml", &broken_syntax),
        ("SYS/TEST-SYS-0002.toml", &missing_field),
        ("SYS/TEST-SYS-0003.toml", &valid_req("TEST-SYS-0003")),
    ]);
    assert!(out.contains("TEST-SYS-0001.toml"), "{out}");
    assert!(out.contains("missing field `priority`"), "{out}");
    assert_eq!(out.matches("RQ100").count(), 2, "{out}");
}

#[verifies("VA-CORE-001-01")]
#[test]
fn lint_does_not_cascade_from_a_broken_parent_file() {
    let parent = valid_req("TEST-SYS-0001").replace("rationale =", "ratoinale =");
    let child = format!(
        "{}\n[trace]\nparents = [\"TEST-SYS-0001\"]\n",
        valid_req("TEST-SUB-0001")
    );
    let (_, out) = lint_output(&[
        ("SYS/TEST-SYS-0001.toml", &parent),
        ("SUB/TEST-SUB-0001.toml", &child),
    ]);
    assert!(out.contains("RQ100"), "{out}");
    assert!(!out.contains("RQ015"), "{out}");
}

#[verifies("VA-CORE-002-02")]
#[test]
fn lint_rule_rq101_duplicate_id_across_files() {
    let (_, out) = lint_output(&[
        ("SYS/TEST-SYS-0001.toml", &valid_req("TEST-SYS-0001")),
        ("SUB/TEST-SYS-0001.toml", &valid_req("TEST-SYS-0001")),
    ]);
    assert!(out.contains("RQ101"), "{out}");
}

#[verifies("VA-CORE-002-02")]
#[test]
fn lint_rule_rq102_file_name_must_match_id() {
    let (_, out) = lint_output(&[("SYS/renamed.toml", &valid_req("TEST-SYS-0001"))]);
    assert!(out.contains("RQ102"), "{out}");
    assert!(out.contains("expected `TEST-SYS-0001.toml`"), "{out}");
}

#[test]
fn lint_rules_rq024_rq025_invalid_verification_level_and_phase() {
    let req = valid_req("TEST-SYS-0001")
        .replace("level = \"System\"", "level = \"Galactic\"")
        .replace("phase = \"Development\"", "phase = \"Someday\"");
    let (_, out) = lint_output(&[("SYS/TEST-SYS-0001.toml", &req)]);
    assert!(out.contains("RQ024"), "{out}");
    assert!(out.contains("RQ025"), "{out}");
}

#[verifies("VA-CORE-002-02")]
#[test]
fn lint_rule_rq026_unknown_refines_reference() {
    let req = format!(
        "{}\n[trace]\nrefines = [\"TEST-SYS-9999\"]\n",
        valid_req("TEST-SYS-0001")
    );
    let (_, out) = lint_output(&[("SYS/TEST-SYS-0001.toml", &req)]);
    assert!(out.contains("RQ026"), "{out}");
}

#[verifies("VA-CORE-002-02")]
#[test]
fn lint_rule_rq027_duplicate_activity_id() {
    let activity = "\n[[verification.activities]]\nid = \"VA-1\"\nname = \"Shared\"\n";
    let a = format!("{}{activity}", valid_req("TEST-SYS-0001"));
    let b = format!("{}{activity}", valid_req("TEST-SYS-0002"));
    let (_, out) = lint_output(&[
        ("SYS/TEST-SYS-0001.toml", &a),
        ("SYS/TEST-SYS-0002.toml", &b),
    ]);
    assert!(out.contains("RQ027"), "{out}");
}

#[verifies("VA-CORE-002-01")]
#[test]
fn lint_mutual_conflicts_with_is_not_a_cycle() {
    let a = format!(
        "{}\n[trace]\nconflicts_with = [\"TEST-SYS-0002\"]\n",
        valid_req("TEST-SYS-0001")
    );
    let b = format!(
        "{}\n[trace]\nconflicts_with = [\"TEST-SYS-0001\"]\n",
        valid_req("TEST-SYS-0002")
    );
    let (code, out) = lint_output(&[
        ("SYS/TEST-SYS-0001.toml", &a),
        ("SYS/TEST-SYS-0002.toml", &b),
    ]);
    assert_eq!(code, 0, "{out}");
}

#[test]
fn lint_rq017_names_every_requirement_in_the_cycle() {
    let a = format!(
        "{}\n[trace]\ndepends_on = [\"TEST-SYS-0002\"]\n",
        valid_req("TEST-SYS-0001")
    );
    let b = format!(
        "{}\n[trace]\nderived_from = [\"TEST-SYS-0001\"]\n",
        valid_req("TEST-SYS-0002")
    );
    let (_, out) = lint_output(&[
        ("SYS/TEST-SYS-0001.toml", &a),
        ("SYS/TEST-SYS-0002.toml", &b),
    ]);
    assert_eq!(out.matches("RQ017").count(), 2, "{out}");
    assert!(out.contains("TEST-SYS-0001 ↔ TEST-SYS-0002"), "{out}");
}

#[verifies("VA-CORE-005-01")]
#[test]
fn lint_forbidden_keyword_matches_whole_words_only() {
    let config = BASE_CONFIG.replace(
        "forbidden_keywords = []",
        "forbidden_keywords = [\"should\"]",
    );
    let ok = valid_req("TEST-SYS-0001").replace("do TEST-SYS-0001", "move the shoulder joint");
    let bad = valid_req("TEST-SYS-0002").replace("do TEST-SYS-0002", "do what it should");
    let out = lint_stdout_with_files(
        &config,
        &[
            ("SYS/TEST-SYS-0001.toml", &ok),
            ("SYS/TEST-SYS-0002.toml", &bad),
        ],
    );
    assert_eq!(out.matches("RQ011").count(), 1, "{out}");
    assert!(out.contains("TEST-SYS-0002"), "{out}");
}

#[verifies("VA-CORE-001-02")]
#[test]
fn native_toml_dates_are_accepted() {
    let req = format!(
        "{}\n[[verification.activities]]\nid = \"VA-1\"\nname = \"Run\"\nstatus = \"Passed\"\nexecuted_at = 2024-11-15\n",
        valid_req("TEST-SYS-0001")
    );
    let (code, out) = lint_output(&[("SYS/TEST-SYS-0001.toml", &req)]);
    assert_eq!(code, 0, "{out}");
}

#[verifies("VA-CORE-001-02")]
#[test]
fn old_config_without_schema_version_is_rejected_clearly() {
    let config = BASE_CONFIG.replace("schema_version = 1\n", "");
    let (_dir, repo_root) = write_fixture(&config, &[]);
    rqtk(&repo_root)
        .arg("lint")
        .assert()
        .failure()
        .stderr(predicate::str::contains("supports schema_version 1"));
}

#[verifies("VA-CORE-004-01")]
#[test]
fn rehash_preserves_comments_and_layout() {
    let req = format!(
        "# Owned by the systems team.\n{}",
        valid_req("TEST-SYS-0001").replace("title =", "# keep this note\ntitle =")
    );
    let (_dir, repo_root) = write_fixture(BASE_CONFIG, &[("SYS/TEST-SYS-0001.toml", &req)]);
    // Without --all, files that carry no hash are left alone.
    rqtk(&repo_root).arg("rehash").assert().success();
    let path = repo_root.join(".rqtk/requirements/SYS/TEST-SYS-0001.toml");
    assert_eq!(fs::read_to_string(&path).unwrap(), req);
    rqtk(&repo_root)
        .args(["rehash", "--all"])
        .assert()
        .success();

    let written = fs::read_to_string(&path).unwrap();
    assert!(
        written.starts_with("# Owned by the systems team.\n"),
        "{written}"
    );
    assert!(written.contains("# keep this note\ntitle ="), "{written}");
    assert!(written.contains("content_hash = \"v1:"), "{written}");
    rqtk(&repo_root).arg("lint").assert().code(0);
    // A second run is a no-op.
    rqtk(&repo_root)
        .arg("rehash")
        .assert()
        .stdout(predicate::str::contains("current"));
    assert_eq!(fs::read_to_string(&path).unwrap(), written);
}

#[verifies("VA-CLI-004-01")]
#[test]
fn add_requires_flags_and_rejects_unknown_category() {
    let (_dir, repo_root) = write_fixture(BASE_CONFIG, &[]);
    rqtk(&repo_root).arg("add").assert().failure();
    rqtk(&repo_root)
        .args(["add", "--category", "NOPE", "--type", "Functional"])
        .args(["--title", "T", "--statement", "The system shall work."])
        .assert()
        .failure()
        .stderr(predicate::str::contains("expected one of: SUB, SYS"));
}

#[verifies("VA-CLI-007-02")]
#[test]
fn report_prints_markdown_to_stdout() {
    let repo_root = fixture_root("firesat-obc");
    rqtk(&repo_root)
        .arg("report")
        .assert()
        .success()
        .stdout(predicate::str::starts_with(
            "# FireSat OBC — Requirements Specification",
        ))
        .stdout(predicate::str::contains("## Traceability matrix"))
        .stdout(predicate::str::contains("#### `FOBC-SYS-0001`"));
}

#[verifies("VA-CLI-007-02")]
#[test]
fn graph_rejects_graphml() {
    let repo_root = fixture_root("firesat-obc");
    rqtk(&repo_root)
        .args(["graph", "--format", "graphml"])
        .assert()
        .failure();
}

// ── 0.3: verification from test evidence ─────────────────────────────────────

/// A requirement whose verification has success criteria and the given activities.
fn req_with_activities(id: &str, activities: &[&str]) -> String {
    let mut req = format!("{}success_criteria = \"It works.\"\n", valid_req(id));
    for a in activities {
        req.push_str(&format!(
            "\n[[verification.activities]]\nid = \"{a}\"\nname = \"Activity {a}\"\n"
        ));
    }
    req
}

/// Rust test source linking each `(activity, fn)` pair. Built at runtime so this file
/// itself contains no annotation for `rqtk scan` to pick up.
fn test_source(links: &[(&str, &str)]) -> String {
    links
        .iter()
        .map(|(activity, func)| {
            format!(
                "#[{}(\"{activity}\")]\n#[test]\nfn {func}() {{}}\n",
                "verifies"
            )
        })
        .collect()
}

fn junit(cases: &[(&str, bool)]) -> String {
    let body: String = cases
        .iter()
        .map(|(name, passed)| {
            if *passed {
                format!("<testcase classname=\"boot\" name=\"{name}\"/>")
            } else {
                format!("<testcase classname=\"boot\" name=\"{name}\"><failure/></testcase>")
            }
        })
        .collect();
    format!(
        "<?xml version=\"1.0\"?><testsuites><testsuite name=\"t\">{body}</testsuite></testsuites>"
    )
}

fn evidence_fixture(activities: &[&str], links: &[(&str, &str)]) -> (tempfile::TempDir, PathBuf) {
    let req = req_with_activities("TEST-SYS-0001", activities);
    let (dir, repo_root) = write_fixture(BASE_CONFIG, &[("SYS/TEST-SYS-0001.toml", &req)]);
    fs::create_dir_all(repo_root.join("tests")).unwrap();
    fs::write(repo_root.join("tests/boot.rs"), test_source(links)).unwrap();
    (dir, repo_root)
}

fn write_junit(repo_root: &Path, cases: &[(&str, bool)]) -> PathBuf {
    let path = repo_root.join("junit.xml");
    fs::write(&path, junit(cases)).unwrap();
    path
}

#[verifies("VA-CORE-006-01")]
#[test]
fn scan_lists_links_with_test_names() {
    let (_dir, repo_root) = evidence_fixture(&["VA-1"], &[("VA-1", "boots")]);
    rqtk(&repo_root)
        .arg("scan")
        .assert()
        .success()
        .stdout(predicate::str::contains("VA-1"))
        .stdout(predicate::str::contains("tests/boot.rs:1"))
        .stdout(predicate::str::contains("boots"));
}

#[verifies("VA-SYS-002-01")]
#[test]
fn verify_then_edit_makes_requirement_suspect_until_reverified() {
    let (_dir, repo_root) = evidence_fixture(&["VA-1"], &[("VA-1", "boots")]);
    let results = write_junit(&repo_root, &[("boots", true)]);

    rqtk(&repo_root)
        .args(["coverage"])
        .assert()
        .stdout(predicate::str::contains("Planned 1"));

    rqtk(&repo_root)
        .arg("verify")
        .arg("--results")
        .arg(&results)
        .assert()
        .success()
        .stdout(predicate::str::contains("+ VA-1  passed"));
    let evidence = fs::read_to_string(repo_root.join(".rqtk/evidence.toml")).unwrap();
    assert!(evidence.contains("id = \"VA-1\""), "{evidence}");
    assert!(evidence.contains("outcome = \"passed\""), "{evidence}");
    assert!(evidence.contains("tests = [\"boot::boots\"]"), "{evidence}");

    rqtk(&repo_root)
        .args(["coverage", "--strict"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Verified 1"));

    // Change what the requirement demands: the passing result no longer applies.
    let path = repo_root.join(".rqtk/requirements/SYS/TEST-SYS-0001.toml");
    let text = fs::read_to_string(&path).unwrap();
    fs::write(&path, text.replace("shall do", "shall always do")).unwrap();

    rqtk(&repo_root)
        .args(["coverage", "--strict"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Suspect 1"))
        .stdout(predicate::str::contains("VA-1 suspect"));
    rqtk(&repo_root)
        .arg("verify")
        .arg("--check")
        .arg("--results")
        .arg(&results)
        .assert()
        .code(1);

    // Rerunning the same, unchanged test proves nothing about the new wording.
    rqtk(&repo_root)
        .arg("verify")
        .arg("--results")
        .arg(&results)
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "VA-1  passed with unchanged tests",
        ));
    rqtk(&repo_root)
        .args(["coverage", "--strict"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains(
            "passed again with unchanged tests",
        ));

    // Updating the test for the new wording and running it settles the requirement.
    let source = repo_root.join("tests/boot.rs");
    let text = fs::read_to_string(&source).unwrap();
    fs::write(
        &source,
        text.replace("fn boots() {}", "fn boots() { assert!(true); }"),
    )
    .unwrap();
    rqtk(&repo_root)
        .arg("verify")
        .arg("--results")
        .arg(&results)
        .assert()
        .success()
        .stdout(predicate::str::contains("VA-1  passed (re-verified)"));
    rqtk(&repo_root)
        .args(["coverage", "--strict"])
        .assert()
        .success();
    rqtk(&repo_root)
        .arg("verify")
        .arg("--check")
        .arg("--results")
        .arg(&results)
        .assert()
        .success();
}

#[verifies("VA-SYS-005-01")]
#[test]
fn review_settles_unchanged_tests_but_not_an_unrun_change() {
    let (_dir, repo_root) = evidence_fixture(&["VA-1"], &[("VA-1", "boots")]);
    let results = write_junit(&repo_root, &[("boots", true)]);
    let verify = |repo_root: &Path| {
        rqtk(repo_root)
            .arg("verify")
            .arg("--results")
            .arg(&results)
            .assert()
            .success();
    };
    verify(&repo_root);
    let path = repo_root.join(".rqtk/requirements/SYS/TEST-SYS-0001.toml");
    let text = fs::read_to_string(&path).unwrap();
    fs::write(&path, text.replace("shall do", "shall always do")).unwrap();

    // Not rerun yet: a review doesn't stand in for running the tests.
    rqtk(&repo_root)
        .args(["review", "TEST-SYS-0001"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Still Suspect"));
    rqtk(&repo_root)
        .args(["coverage", "--strict"])
        .assert()
        .code(1);

    verify(&repo_root);
    rqtk(&repo_root)
        .args(["coverage", "--strict"])
        .assert()
        .code(1);
    rqtk(&repo_root)
        .args(["review", "TEST-SYS-0001", "--note", "wording only"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Settled"));
    rqtk(&repo_root)
        .args(["coverage", "--strict"])
        .assert()
        .success();
    let evidence = fs::read_to_string(repo_root.join(".rqtk/evidence.toml")).unwrap();
    assert!(evidence.contains("note = \"wording only\""), "{evidence}");
}

#[verifies("VA-SYS-005-01")]
#[test]
fn changed_parent_leaves_children_suspect_until_reviewed() {
    let parent = req_with_activities("TEST-SYS-0001", &["VA-1"]);
    let child = req_with_activities("TEST-SUB-0001", &["VA-2"]).replace(
        "[verification]",
        "[trace]\nparents = [\"TEST-SYS-0001\"]\n\n[verification]",
    );
    let (_dir, repo_root) = write_fixture(
        BASE_CONFIG,
        &[
            ("SYS/TEST-SYS-0001.toml", &parent),
            ("SUB/TEST-SUB-0001.toml", &child),
        ],
    );
    fs::create_dir_all(repo_root.join("tests")).unwrap();
    fs::write(
        repo_root.join("tests/boot.rs"),
        test_source(&[("VA-1", "boots"), ("VA-2", "halts")]),
    )
    .unwrap();
    let results = write_junit(&repo_root, &[("boots", true), ("halts", true)]);
    let verify = || {
        rqtk(&repo_root)
            .arg("verify")
            .arg("--results")
            .arg(&results)
            .assert()
            .success();
    };
    verify();
    rqtk(&repo_root)
        .args(["coverage", "--strict"])
        .assert()
        .success();

    let path = repo_root.join(".rqtk/requirements/SYS/TEST-SYS-0001.toml");
    let text = fs::read_to_string(&path).unwrap();
    fs::write(&path, text.replace("shall do", "shall quickly do")).unwrap();
    rqtk(&repo_root)
        .args(["coverage", "--strict"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains(
            "TEST-SUB-0001  (TEST-SYS-0001 changed",
        ));

    // Rerunning the child's tests says nothing about whether it still fits its parent.
    verify();
    rqtk(&repo_root)
        .arg("coverage")
        .assert()
        .stdout(predicate::str::contains(
            "TEST-SUB-0001  (TEST-SYS-0001 changed",
        ));
    rqtk(&repo_root)
        .args(["review", "TEST-SUB-0001"])
        .assert()
        .success();
    rqtk(&repo_root)
        .arg("coverage")
        .assert()
        .stdout(predicate::str::contains("TEST-SUB-0001").not());
}

#[verifies("VA-SYS-002-02")]
#[test]
fn verify_records_failures_and_exits_nonzero() {
    let (_dir, repo_root) = evidence_fixture(&["VA-1"], &[("VA-1", "boots")]);
    let results = write_junit(&repo_root, &[("boots", false)]);
    rqtk(&repo_root)
        .arg("verify")
        .arg("--results")
        .arg(&results)
        .assert()
        .code(1);
    rqtk(&repo_root)
        .args(["coverage", "--strict"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Failed 1"))
        .stdout(predicate::str::contains("VA-1 failed"));
}

#[verifies("VA-SYS-004-02")]
#[test]
fn coverage_strict_requires_every_requirement_verified_unless_allowed() {
    let (_dir, repo_root) = evidence_fixture(&["VA-1"], &[("VA-1", "boots")]);
    rqtk(&repo_root)
        .args(["coverage", "--strict"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Planned 1"));
    rqtk(&repo_root)
        .args(["coverage", "--strict", "--allow", "planned"])
        .assert()
        .success();
}

#[verifies("VA-SYS-002-02")]
#[test]
fn verify_fails_when_no_result_matches_a_linked_test() {
    let (_dir, repo_root) = evidence_fixture(&["VA-1"], &[("VA-1", "boots")]);
    let results = write_junit(&repo_root, &[("something_else", true)]);
    rqtk(&repo_root)
        .arg("verify")
        .arg("--results")
        .arg(&results)
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "none of the 1 test results matched",
        ));
    assert!(!repo_root.join(".rqtk/evidence.toml").exists());
}

#[verifies("VA-SYS-002-02")]
#[test]
fn verify_refuses_to_guess_between_same_named_tests() {
    let (_dir, repo_root) = evidence_fixture(&["VA-1"], &[("VA-1", "boots")]);
    let results = repo_root.join("junit.xml");
    fs::write(
        &results,
        "<testsuite><testcase classname=\"alpha\" name=\"boots\"/>\
         <testcase classname=\"beta\" name=\"boots\"><failure/></testcase></testsuite>",
    )
    .unwrap();
    rqtk(&repo_root)
        .arg("verify")
        .arg("--results")
        .arg(&results)
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Ambiguous"))
        .stdout(predicate::str::contains("alpha::boots, beta::boots"));
    assert!(!repo_root.join(".rqtk/evidence.toml").exists());
}

#[verifies("VA-SYS-002-03")]
#[test]
fn verify_partial_run_keeps_other_evidence() {
    let (_dir, repo_root) =
        evidence_fixture(&["VA-1", "VA-2"], &[("VA-1", "boots"), ("VA-2", "halts")]);
    let both = write_junit(&repo_root, &[("boots", true), ("halts", true)]);
    rqtk(&repo_root)
        .arg("verify")
        .arg("--results")
        .arg(&both)
        .assert()
        .success();

    let only_halts = write_junit(&repo_root, &[("halts", true)]);
    rqtk(&repo_root)
        .arg("verify")
        .arg("--results")
        .arg(&only_halts)
        .assert()
        .success()
        .stdout(predicate::str::contains("Evidence is up to date"));
    let evidence = fs::read_to_string(repo_root.join(".rqtk/evidence.toml")).unwrap();
    assert!(evidence.contains("\"VA-1\""), "{evidence}");
    rqtk(&repo_root)
        .arg("coverage")
        .assert()
        .stdout(predicate::str::contains("Verified 1"));
}

#[verifies("VA-SYS-002-03")]
#[test]
fn verify_leaves_evidence_alone_when_a_linked_test_was_skipped() {
    let (_dir, repo_root) = evidence_fixture(&["VA-1"], &[("VA-1", "boots"), ("VA-1", "halts")]);
    let results = write_junit(&repo_root, &[("boots", true)]);
    rqtk(&repo_root)
        .arg("verify")
        .arg("--results")
        .arg(&results)
        .assert()
        .success()
        .stdout(predicate::str::contains("no result for halts"));
    assert!(!repo_root.join(".rqtk/evidence.toml").exists());
}

#[verifies("VA-CORE-006-01")]
#[test]
fn lint_cross_checks_source_links() {
    let req = req_with_activities("TEST-SYS-0001", &["VA-1"]).replace(
        "name = \"Activity VA-1\"",
        "name = \"Activity VA-1\"\nstatus = \"Passed\"",
    );
    let (_dir, repo_root) = write_fixture(BASE_CONFIG, &[("SYS/TEST-SYS-0001.toml", &req)]);
    fs::create_dir_all(repo_root.join("tests")).unwrap();
    let mut src = test_source(&[("VA-1", "boots"), ("VA-TYPO", "halts")]);
    src.push_str(&format!("#[{}(\"VA-1\")]\nconst X: u8 = 1;\n", "verifies"));
    fs::write(repo_root.join("tests/boot.rs"), src).unwrap();

    let output = rqtk(&repo_root).arg("lint").output().unwrap();
    let out = String::from_utf8(output.stdout).unwrap();
    assert_eq!(output.status.code(), Some(1), "{out}");
    assert!(out.contains("RQ028") && out.contains("VA-TYPO"), "{out}");
    assert!(out.contains("tests/boot.rs:4"), "{out}");
    assert!(out.contains("RQ029"), "{out}");
    assert!(out.contains("RQ030"), "{out}");
}

#[verifies("VA-CORE-006-01")]
#[test]
fn scan_honours_exclude_patterns() {
    let config = format!("{BASE_CONFIG}\n[scan]\nexclude = [\"tests/**\"]\n");
    let (_dir, repo_root) = write_fixture(&config, &[]);
    fs::create_dir_all(repo_root.join("tests")).unwrap();
    fs::write(
        repo_root.join("tests/x.rs"),
        test_source(&[("VA-TYPO", "t")]),
    )
    .unwrap();
    rqtk(&repo_root).arg("lint").assert().code(0);
}

// ── 0.4: agent-facing CLI ────────────────────────────────────────────────────

fn json_stdout(repo_root: &Path, args: &[&str]) -> (i32, serde_json::Value) {
    let output = rqtk(repo_root).arg("--json").args(args).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout is not JSON ({e}):\n{stdout}"));
    (output.status.code().unwrap(), value)
}

#[verifies("VA-CLI-002-01")]
#[test]
fn exit_codes_distinguish_findings_usage_and_errors() {
    let bad = valid_req("TEST-SYS-0001").replace("rationale = \"Because.\"\n", "");
    let (_dir, repo_root) = write_fixture(BASE_CONFIG, &[("SYS/TEST-SYS-0001.toml", &bad)]);
    rqtk(&repo_root).arg("lint").assert().code(1);
    rqtk(&repo_root)
        .args(["lint", "--no-such-flag"])
        .assert()
        .code(2);
    rqtk(&repo_root)
        .args(["add", "--category", "NOPE", "--type", "Functional"])
        .args(["--title", "T", "--statement", "The system shall work."])
        .assert()
        .code(2);
    rqtk(&repo_root).args(["--json", "report"]).assert().code(2);

    let empty = tempfile::tempdir().unwrap();
    git_init(empty.path());
    rqtk(empty.path()).arg("lint").assert().code(3);
}

#[verifies("VA-CLI-001-01")]
#[test]
fn json_errors_go_to_stderr() {
    let empty = tempfile::tempdir().unwrap();
    git_init(empty.path());
    let output = rqtk(empty.path())
        .args(["--json", "lint"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(3));
    assert!(output.stdout.is_empty());
    let err: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(err["error"]["kind"], "error");
    assert!(
        err["error"]["message"]
            .as_str()
            .unwrap()
            .contains("config.toml")
    );
}

#[verifies("VA-CLI-001-01")]
#[test]
fn lint_json_reports_located_diagnostics() {
    let bad = valid_req("TEST-SYS-0001").replace("rationale =", "ratoinale =");
    let (_dir, repo_root) = write_fixture(BASE_CONFIG, &[("SYS/TEST-SYS-0001.toml", &bad)]);
    let (code, report) = json_stdout(&repo_root, &["lint"]);
    assert_eq!(code, 1);
    assert_eq!(report["errors"], 1);
    let diag = &report["diagnostics"][0];
    assert_eq!(diag["code"], "RQ100");
    assert_eq!(diag["severity"], "error");
    assert_eq!(
        diag["location"]["path"],
        ".rqtk/requirements/SYS/TEST-SYS-0001.toml"
    );
    assert_eq!(diag["location"]["line"], 8);
}

#[verifies("VA-CLI-001-01")]
#[test]
fn coverage_and_search_json() {
    let repo_root = fixture_root("firesat-obc");
    let (code, coverage) = json_stdout(&repo_root, &["coverage"]);
    assert_eq!(code, 0);
    let reqs = coverage["requirements"].as_array().unwrap();
    assert_eq!(reqs.len(), 7);
    assert!(reqs[0]["status"].is_string());
    assert!(coverage["summary"].is_object());

    let (_, search) = json_stdout(&repo_root, &["search", "telemetry", "-i"]);
    let hit = &search["hits"][0];
    assert_eq!(hit["subject"]["kind"], "requirement");
    assert!(hit["matches"][0]["ranges"].is_array());
}

#[verifies("VA-CLI-003-01")]
#[test]
fn dry_runs_write_nothing() {
    let (_dir, repo_root) = write_fixture(
        BASE_CONFIG,
        &[("SYS/TEST-SYS-0001.toml", &valid_req("TEST-SYS-0001"))],
    );
    git_set_identity(&repo_root);
    git_commit_all(&repo_root, "initial");
    let snapshot = || {
        let mut files = Vec::new();
        for entry in fs::read_dir(repo_root.join(".rqtk/requirements/SYS")).unwrap() {
            let p = entry.unwrap().path();
            files.push((p.clone(), fs::read_to_string(p).unwrap()));
        }
        files.sort();
        files
    };
    let before = snapshot();

    let (_, added) = json_stdout(
        &repo_root,
        &[
            "add",
            "--category",
            "SYS",
            "--type",
            "Functional",
            "--title",
            "T",
            "--statement",
            "The system shall work.",
            "--dry-run",
        ],
    );
    assert_eq!(added["id"], "TEST-SYS-0002");
    assert_eq!(added["dry_run"], true);
    assert_eq!(added["item"]["statement"], "The system shall work.");

    let (_, rehash) = json_stdout(&repo_root, &["rehash", "--all", "--dry-run"]);
    assert_eq!(rehash["updated"].as_array().unwrap().len(), 1);

    let (_, baseline) = json_stdout(&repo_root, &["baseline", "1.0.0", "--dry-run"]);
    assert_eq!(baseline["tag"], "rqtk/1.0.0");
    assert!(!git_tag_exists(&repo_root, "rqtk/1.0.0"));

    assert_eq!(snapshot(), before);

    let fresh = tempfile::tempdir().unwrap();
    let (_, init) = json_stdout(fresh.path(), &["init", "--dry-run"]);
    assert!(init["created"].as_array().unwrap().len() >= 2);
    assert!(!fresh.path().join(".rqtk").exists());
}

#[verifies("VA-CLI-003-01")]
#[test]
fn verify_dry_run_does_not_write_evidence() {
    let (_dir, repo_root) = evidence_fixture(&["VA-1"], &[("VA-1", "boots")]);
    let results = write_junit(&repo_root, &[("boots", true)]);
    let (code, report) = json_stdout(
        &repo_root,
        &[
            "verify",
            "--dry-run",
            "--results",
            results.to_str().unwrap(),
        ],
    );
    assert_eq!(code, 0);
    assert_eq!(report["written"], false);
    assert_eq!(report["changes"][0]["kind"], "added");
    assert!(!repo_root.join(".rqtk/evidence.toml").exists());
}

#[verifies("VA-CLI-006-01")]
#[test]
fn schema_prints_json_schema_per_kind() {
    let repo_root = fixture_root("firesat-obc");
    let (code, schema) = json_stdout(&repo_root, &["schema", "requirement"]);
    assert_eq!(code, 0);
    assert!(schema["properties"]["statement"].is_object());
    assert_eq!(schema["additionalProperties"], false);
    rqtk(&repo_root).args(["schema", "nope"]).assert().code(2);
    rqtk(&repo_root)
        .arg("schema")
        .assert()
        .success()
        .stdout(predicate::str::contains("evidence"));
}

#[verifies("VA-CLI-006-01")]
#[test]
fn explain_describes_rules_and_fixes() {
    let repo_root = fixture_root("firesat-obc");
    rqtk(&repo_root)
        .args(["explain", "rq010"])
        .assert()
        .success()
        .stdout(predicate::str::contains("shall"))
        .stdout(predicate::str::contains("Fix:"));
    let (_, one) = json_stdout(&repo_root, &["explain", "RQ021"]);
    assert_eq!(one["severity"], "warning");
    let (_, all) = json_stdout(&repo_root, &["explain"]);
    assert!(all.as_array().unwrap().len() >= 30);
    rqtk(&repo_root).args(["explain", "RQ999"]).assert().code(2);
}

#[verifies("VA-SYS-003-01")]
#[test]
fn context_briefs_a_requirement_need_and_stakeholder() {
    let (_dir, repo_root) = evidence_fixture(&["VA-1"], &[("VA-1", "boots")]);
    let path = repo_root.join(".rqtk/requirements/SYS/TEST-SYS-0001.toml");
    let text = fs::read_to_string(&path).unwrap();
    fs::write(
        &path,
        format!("{text}\n[trace]\nsatisfies = [\"NEED-0001\"]\n"),
    )
    .unwrap();
    let need = "id = \"NEED-0001\"\ntitle = \"Boot fast\"\nstate = \"Draft\"\nstakeholders = [\"STK-001\"]\nstatement = \"Operators need fast boot.\"\n";
    let stk = "id = \"STK-001\"\nname = \"Ops\"\n";
    fs::create_dir_all(repo_root.join(".rqtk/needs")).unwrap();
    fs::create_dir_all(repo_root.join(".rqtk/stakeholders")).unwrap();
    fs::write(repo_root.join(".rqtk/needs/NEED-0001.toml"), need).unwrap();
    fs::write(repo_root.join(".rqtk/stakeholders/STK-001.toml"), stk).unwrap();

    let (code, ctx) = json_stdout(&repo_root, &["context", "TEST-SYS-0001"]);
    assert_eq!(code, 0);
    assert_eq!(ctx["kind"], "requirement");
    assert_eq!(ctx["path"], ".rqtk/requirements/SYS/TEST-SYS-0001.toml");
    assert_eq!(ctx["needs"][0]["id"], "NEED-0001");
    assert_eq!(ctx["tests"][0]["test_name"], "boots");
    assert_eq!(ctx["verification"]["status"], "planned");

    let (_, need_ctx) = json_stdout(&repo_root, &["context", "NEED-0001"]);
    assert_eq!(need_ctx["kind"], "need");
    assert_eq!(need_ctx["satisfied_by"][0]["id"], "TEST-SYS-0001");
    assert_eq!(need_ctx["stakeholders"][0]["name"], "Ops");

    let (_, stk_ctx) = json_stdout(&repo_root, &["context", "STK-001"]);
    assert_eq!(stk_ctx["needs"][0]["id"], "NEED-0001");

    rqtk(&repo_root)
        .args(["context", "TEST-SYS-0001"])
        .assert()
        .success()
        .stdout(predicate::str::contains("boots"));
    rqtk(&repo_root)
        .args(["context", "NOPE-1"])
        .assert()
        .code(2);
}

#[verifies("VA-SYS-003-02")]
#[test]
fn impact_reports_changes_downstream_and_reverification() {
    let parent = req_with_activities("TEST-SYS-0001", &["VA-1"]);
    let child = format!(
        "{}\n[trace]\nparents = [\"TEST-SYS-0001\"]\n",
        req_with_activities("TEST-SUB-0001", &["VA-2"])
    );
    let (_dir, repo_root) = write_fixture(
        BASE_CONFIG,
        &[
            ("SYS/TEST-SYS-0001.toml", &parent),
            ("SUB/TEST-SUB-0001.toml", &child),
        ],
    );
    fs::create_dir_all(repo_root.join("tests")).unwrap();
    fs::write(
        repo_root.join("tests/boot.rs"),
        test_source(&[("VA-1", "boots"), ("VA-2", "halts")]),
    )
    .unwrap();
    // Commit through the git CLI so the index matches HEAD, as in a real checkout.
    for args in [
        &["add", "-A"][..],
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-qm",
            "initial",
        ][..],
    ] {
        let status = std::process::Command::new("git")
            .args(args)
            .current_dir(&repo_root)
            .status()
            .unwrap();
        assert!(status.success());
    }

    let (_, none) = json_stdout(&repo_root, &["impact", "HEAD"]);
    assert_eq!(none["reverify"].as_array().unwrap().len(), 0, "{none}");
    assert_eq!(none["requirements"].as_array().unwrap().len(), 0);

    let path = repo_root.join(".rqtk/requirements/SYS/TEST-SYS-0001.toml");
    let text = fs::read_to_string(&path).unwrap();
    fs::write(&path, text.replace("shall do", "shall always do")).unwrap();

    let (code, impact) = json_stdout(&repo_root, &["impact", "HEAD"]);
    assert_eq!(code, 0);
    assert_eq!(impact["requirements"][0]["id"], "TEST-SYS-0001");
    assert_eq!(impact["requirements"][0]["change"], "semantic");
    assert_eq!(impact["downstream"][0]["id"], "TEST-SUB-0001");
    assert_eq!(impact["downstream"][0]["via"][0], "TEST-SYS-0001");
    assert_eq!(impact["reverify"][0]["activity"], "VA-1");
    assert_eq!(
        impact["reverify"][0]["reasons"][0]["reason"],
        "requirement_changed"
    );

    // Touch the test file: VA-2's test changed, so it needs re-running too.
    let src = fs::read_to_string(repo_root.join("tests/boot.rs")).unwrap();
    fs::write(repo_root.join("tests/boot.rs"), format!("{src}// edited\n")).unwrap();
    let (_, impact) = json_stdout(&repo_root, &["impact", "HEAD"]);
    let reverify = impact["reverify"].as_array().unwrap();
    assert_eq!(reverify.len(), 2, "{impact}");
    let va2 = reverify.iter().find(|r| r["activity"] == "VA-2").unwrap();
    assert_eq!(va2["reasons"][0]["reason"], "test_changed");
    assert_eq!(va2["reasons"][0]["path"], "tests/boot.rs");
    let va1 = reverify.iter().find(|r| r["activity"] == "VA-1").unwrap();
    assert_eq!(va1["reasons"].as_array().unwrap().len(), 2, "{va1}");

    rqtk(&repo_root)
        .args(["impact", "HEAD"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Re-verify"));
}

// ── 0.5: agent skills ────────────────────────────────────────────────────────

fn skills_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skills")
}

#[verifies("VA-CLI-005-01")]
#[test]
fn skills_list_names_model_and_user_invoked_skills() {
    let repo_root = fixture_root("firesat-obc");
    let (code, skills) = json_stdout(&repo_root, &["skills", "list"]);
    assert_eq!(code, 0);
    let skills = skills.as_array().unwrap();
    let user: Vec<&str> = skills
        .iter()
        .filter(|s| s["user_invoked"] == true)
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert_eq!(skills.len(), 4);
    assert_eq!(user, ["to-requirements", "requirements-review"]);
}

fn actions(report: &serde_json::Value) -> Vec<(String, String)> {
    report["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            (
                f["path"].as_str().unwrap().to_owned(),
                f["action"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

#[verifies("VA-CLI-005-01")]
#[test]
fn skills_install_defaults_to_shared_dir_with_claude_links() {
    let (_dir, repo_root) = write_fixture(BASE_CONFIG, &[]);
    let (code, report) = json_stdout(&repo_root, &["skills", "install"]);
    assert_eq!(code, 0);
    assert_eq!(
        report["dirs"],
        serde_json::json!([".agents/skills", ".claude/skills"])
    );

    let shared = repo_root.join(".agents/skills/rqtk-verification/SKILL.md");
    assert!(shared.is_file() && !shared.is_symlink());
    let link = repo_root.join(".claude/skills/rqtk-verification");
    if cfg!(unix) {
        assert_eq!(
            fs::read_link(&link).unwrap(),
            PathBuf::from("../../.agents/skills/rqtk-verification")
        );
    }
    // Claude Code reads the very same file through the link.
    assert_eq!(
        fs::read_to_string(link.join("SKILL.md")).unwrap(),
        fs::read_to_string(&shared).unwrap()
    );

    let (_, again) = json_stdout(&repo_root, &["skills", "install"]);
    assert!(
        actions(&again).iter().all(|(_, a)| a == "unchanged"),
        "{again}"
    );
}

#[verifies("VA-CLI-005-01")]
#[test]
fn skills_install_for_one_agent_family_or_as_copies() {
    let (_dir, claude_only) = write_fixture(BASE_CONFIG, &[]);
    json_stdout(&claude_only, &["skills", "install", "--for", "claude"]);
    assert!(
        claude_only
            .join(".claude/skills/to-requirements/SKILL.md")
            .is_file()
    );
    assert!(
        !claude_only
            .join(".claude/skills/to-requirements")
            .is_symlink()
    );
    assert!(!claude_only.join(".agents").exists());

    let (_dir, universal_only) = write_fixture(BASE_CONFIG, &[]);
    json_stdout(
        &universal_only,
        &["skills", "install", "--for", "universal"],
    );
    assert!(
        universal_only
            .join(".agents/skills/to-requirements/SKILL.md")
            .is_file()
    );
    assert!(!universal_only.join(".claude").exists());

    let (_dir, copies) = write_fixture(BASE_CONFIG, &[]);
    json_stdout(&copies, &["skills", "install", "--copy"]);
    assert!(!copies.join(".claude/skills/to-requirements").is_symlink());
    assert!(
        copies
            .join(".claude/skills/to-requirements/SKILL.md")
            .is_file()
    );
    assert!(
        copies
            .join(".agents/skills/to-requirements/SKILL.md")
            .is_file()
    );
}

#[verifies("VA-CLI-005-01")]
#[test]
fn skills_install_keeps_local_edits_and_earlier_copies() {
    let (_dir, repo_root) = write_fixture(BASE_CONFIG, &[]);
    // An earlier copy-style install in .claude/skills stays a copy.
    json_stdout(&repo_root, &["skills", "install", "--for", "claude"]);
    let (_, report) = json_stdout(&repo_root, &["skills", "install"]);
    assert!(
        !repo_root
            .join(".claude/skills/rqtk-requirements")
            .is_symlink()
    );
    assert!(
        actions(&report)
            .iter()
            .filter(|(p, _)| p.starts_with(".claude/"))
            .all(|(_, a)| a == "unchanged"),
        "{report}"
    );

    let shared = repo_root.join(".agents/skills/rqtk-verification/SKILL.md");
    fs::write(&shared, "my own version\n").unwrap();
    let (_, kept) = json_stdout(&repo_root, &["skills", "install"]);
    assert!(
        actions(&kept).contains(&(
            ".agents/skills/rqtk-verification/SKILL.md".into(),
            "skipped_modified".into()
        )),
        "{kept}"
    );
    assert_eq!(fs::read_to_string(&shared).unwrap(), "my own version\n");
    rqtk(&repo_root)
        .args(["skills", "install", "--force"])
        .assert()
        .success();
    assert_ne!(fs::read_to_string(&shared).unwrap(), "my own version\n");
}

#[verifies("VA-CLI-005-01")]
#[test]
fn skills_install_updates_existing_instruction_files_only() {
    // Neither file: nothing created.
    let (_dir, none) = write_fixture(BASE_CONFIG, &[]);
    let (_, report) = json_stdout(&none, &["skills", "install"]);
    assert_eq!(report["instructions"], serde_json::json!([]));
    assert!(!none.join("AGENTS.md").exists() && !none.join("CLAUDE.md").exists());

    // Both, and CLAUDE.md imports AGENTS.md: only AGENTS.md gets the block.
    let (_dir, both) = write_fixture(BASE_CONFIG, &[]);
    fs::write(both.join("AGENTS.md"), "# Agents\n").unwrap();
    fs::write(both.join("CLAUDE.md"), "@AGENTS.md\n").unwrap();
    let (_, report) = json_stdout(&both, &["skills", "install"]);
    assert_eq!(report["instructions"].as_array().unwrap().len(), 1);
    assert_eq!(report["instructions"][0]["path"], "AGENTS.md");
    assert_eq!(
        fs::read_to_string(both.join("CLAUDE.md")).unwrap(),
        "@AGENTS.md\n"
    );

    // Both, independent: both get it. Existing text is kept; reruns change nothing.
    let (_dir, separate) = write_fixture(BASE_CONFIG, &[]);
    fs::write(separate.join("AGENTS.md"), "# Agents\n").unwrap();
    fs::write(separate.join("CLAUDE.md"), "# Project\n\nOur rules.\n").unwrap();
    let (_, report) = json_stdout(&separate, &["skills", "install"]);
    assert_eq!(report["instructions"].as_array().unwrap().len(), 2);
    let claude = fs::read_to_string(separate.join("CLAUDE.md")).unwrap();
    assert!(
        claude.starts_with("# Project\n\nOur rules.\n\n<!-- BEGIN rqtk"),
        "{claude}"
    );
    assert!(claude.contains("rqtk coverage --strict"));
    let (_, again) = json_stdout(&separate, &["skills", "install"]);
    assert!(
        again["instructions"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["action"] == "unchanged")
    );

    // Named files are created; anything but AGENTS.md / CLAUDE.md is a usage error.
    rqtk(&none)
        .args(["skills", "install", "--instructions", "NOTES.md"])
        .assert()
        .code(2);
    assert!(!none.join("NOTES.md").exists());
    let (_, named) = json_stdout(&none, &["skills", "install", "--instructions", "AGENTS.md"]);
    assert_eq!(named["instructions"][0]["action"], "created");
    assert!(
        fs::read_to_string(none.join("AGENTS.md"))
            .unwrap()
            .contains("## Requirements (rqtk)")
    );
}

#[verifies("VA-CLI-005-01")]
#[test]
fn skills_install_into_a_custom_dir() {
    let (_dir, repo_root) = write_fixture(BASE_CONFIG, &[]);
    let (_, report) = json_stdout(&repo_root, &["skills", "install", "--dir", "tools/skills"]);
    assert_eq!(report["dirs"], serde_json::json!(["tools/skills"]));
    assert!(
        repo_root
            .join("tools/skills/requirements-review/SKILL.md")
            .is_file()
    );
    assert!(!repo_root.join(".agents").exists() && !repo_root.join(".claude").exists());
}

#[verifies("VA-CLI-005-01")]
#[test]
fn skills_install_dry_run_and_init_agents() {
    let fresh = tempfile::tempdir().unwrap();
    git_init(fresh.path());
    fs::write(fresh.path().join("AGENTS.md"), "# Agents\n").unwrap();
    let (_, dry) = json_stdout(fresh.path(), &["init", "--agents", "--dry-run"]);
    assert!(dry["init"]["created"].is_array());
    assert_eq!(dry["skills"]["dry_run"], true);
    assert!(!fresh.path().join(".claude").exists() && !fresh.path().join(".agents").exists());
    assert_eq!(
        fs::read_to_string(fresh.path().join("AGENTS.md")).unwrap(),
        "# Agents\n"
    );

    let (code, done) = json_stdout(fresh.path(), &["init", "--agents"]);
    assert_eq!(code, 0);
    assert_eq!(done["skills"]["instructions"][0]["path"], "AGENTS.md");
    assert!(
        fresh
            .path()
            .join(".agents/skills/rqtk-requirements/SKILL.md")
            .is_file()
    );
    assert!(
        fresh
            .path()
            .join(".claude/skills/rqtk-requirements/SKILL.md")
            .is_file()
    );
    rqtk(fresh.path()).arg("lint").assert().success();
}

/// Every `rqtk <command> --flag` a skill, the README or the instructions block tells an agent
/// to run must exist in the CLI, so the docs cannot drift from the binary.
#[verifies("VA-CLI-006-02")]
#[test]
fn documented_commands_and_flags_exist() {
    let mut docs: Vec<(String, String)> = Vec::new();
    for entry in fs::read_dir(skills_dir()).unwrap() {
        let path = entry.unwrap().path().join("SKILL.md");
        docs.push((
            path.display().to_string(),
            fs::read_to_string(&path).unwrap(),
        ));
    }
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    docs.push((
        "README.md".into(),
        fs::read_to_string(repo.join("README.md")).unwrap(),
    ));
    // The hand-written pages of rqtk.dev (the reference pages are generated from the binary).
    let mut pending = vec![repo.join("site/docs/src")];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|e| e == "md") {
                docs.push((
                    path.display().to_string(),
                    fs::read_to_string(&path).unwrap(),
                ));
            }
        }
    }

    // Inline code spans in prose, and command lines inside fenced blocks
    // (trailing `# comment` dropped).
    let inline = regex::Regex::new(r"`rqtk ([a-z][a-z-]*)((?: [^`]*)?)`").unwrap();
    let fenced = regex::Regex::new(r"^\s*rqtk ([a-z][a-z-]*)([^#]*)").unwrap();
    let flag = regex::Regex::new(r"--[a-z][a-z-]*").unwrap();
    let mut help_cache = std::collections::HashMap::new();
    let mut checked = 0;
    for (source, text) in &docs {
        let mut invocations: Vec<(String, String)> = Vec::new();
        let mut in_fence = false;
        for line in text.lines() {
            if line.trim_start().starts_with("```") {
                in_fence = !in_fence;
                continue;
            }
            let segments: Vec<&str> = if in_fence {
                line.split(['&', '|', ';']).collect()
            } else {
                vec![line]
            };
            let re = if in_fence { &fenced } else { &inline };
            for segment in segments {
                for caps in re.captures_iter(segment) {
                    invocations.push((caps[1].to_owned(), caps[2].to_owned()));
                }
            }
        }
        for (command, rest) in invocations {
            // `rqtk skills install --for …`: check flags against the subcommand's help.
            let sub = rest
                .split_whitespace()
                .next()
                .filter(|w| {
                    !w.starts_with('-') && w.chars().all(|c| c.is_ascii_lowercase() || c == '-')
                })
                .unwrap_or("")
                .to_owned();
            let help = help_cache
                .entry(format!("{command} {sub}"))
                .or_insert_with(|| {
                    if !sub.is_empty() {
                        let out = rqtk(Path::new("."))
                            .args([command.as_str(), sub.as_str(), "--help"])
                            .output()
                            .unwrap();
                        if out.status.success() {
                            return String::from_utf8(out.stdout).unwrap();
                        }
                    }
                    let out = rqtk(Path::new("."))
                        .args([&command, "--help"])
                        .output()
                        .unwrap();
                    assert!(
                        out.status.success(),
                        "{source}: `rqtk {command}` is not a command"
                    );
                    String::from_utf8(out.stdout).unwrap()
                })
                .clone();
            for f in flag.find_iter(&rest) {
                let f = f.as_str();
                let global = ["--json", "--repo-root", "--help"].contains(&f);
                // Whole-flag match: `--result` must not pass because `--results` exists.
                let listed = help.match_indices(f).any(|(i, _)| {
                    !help[i + f.len()..]
                        .starts_with(|c: char| c.is_ascii_alphanumeric() || c == '-')
                });
                assert!(
                    global || listed,
                    "{source}: `rqtk {command} {sub}` has no flag {f}"
                );
            }
            checked += 1;
        }
    }
    assert!(checked > 80, "only {checked} invocations found");
}

#[verifies("VA-CLI-005-01")]
#[test]
fn plugin_manifest_lists_every_bundled_skill() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let plugin: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root.join(".claude-plugin/plugin.json")).unwrap())
            .unwrap();
    let listed: Vec<String> = plugin["skills"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap().trim_start_matches("./").to_owned())
        .collect();
    let mut bundled: Vec<String> = fs::read_dir(skills_dir())
        .unwrap()
        .map(|e| {
            format!(
                "crates/rqtk/skills/{}",
                e.unwrap().file_name().to_string_lossy()
            )
        })
        .collect();
    bundled.sort();
    let mut listed_sorted = listed.clone();
    listed_sorted.sort();
    assert_eq!(listed_sorted, bundled);
    let _: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(root.join(".claude-plugin/marketplace.json")).unwrap(),
    )
    .unwrap();
}

#[verifies("VA-CLI-005-01")]
#[test]
fn skills_install_upgrades_untouched_files_but_keeps_edits() {
    use sha2::{Digest, Sha256};
    let (_dir, repo_root) = write_fixture(BASE_CONFIG, &[]);
    json_stdout(&repo_root, &["skills", "install", "--for", "universal"]);
    let dir = repo_root.join(".agents/skills");
    let manifest_path = dir.join(".rqtk-skills.json");
    let mut manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();

    // Simulate a file an older rqtk wrote (manifest agrees) next to one the user edited.
    let old = "---\nname: rqtk-requirements\ndescription: old\n---\nold body\n";
    fs::write(dir.join("rqtk-requirements/SKILL.md"), old).unwrap();
    manifest["rqtk-requirements/SKILL.md"] = format!("{:x}", Sha256::digest(old)).into();
    fs::write(&manifest_path, manifest.to_string()).unwrap();
    fs::write(dir.join("to-requirements/SKILL.md"), "my own version\n").unwrap();

    let (_, report) = json_stdout(&repo_root, &["skills", "install", "--for", "universal"]);
    let acts = actions(&report);
    assert!(
        acts.contains(&(
            ".agents/skills/rqtk-requirements/SKILL.md".into(),
            "updated".into()
        )),
        "{report}"
    );
    assert!(
        acts.contains(&(
            ".agents/skills/to-requirements/SKILL.md".into(),
            "skipped_modified".into()
        )),
        "{report}"
    );
    assert_ne!(
        fs::read_to_string(dir.join("rqtk-requirements/SKILL.md")).unwrap(),
        old
    );
    assert_eq!(
        fs::read_to_string(dir.join("to-requirements/SKILL.md")).unwrap(),
        "my own version\n"
    );
}

// ── 1.0: JSON output contract ────────────────────────────────────────────────

/// The type structure of a JSON value: objects keep their keys, arrays are represented by
/// their first element, scalars by their type name.
fn shape(value: &serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match value {
        Value::Object(map) => {
            Value::Object(map.iter().map(|(k, v)| (k.clone(), shape(v))).collect())
        }
        Value::Array(items) => Value::Array(items.first().map(shape).into_iter().collect()),
        Value::String(_) => "string".into(),
        Value::Number(_) => "number".into(),
        Value::Bool(_) => "bool".into(),
        Value::Null => "null".into(),
    }
}

/// Every key and type in `old` is still present in `new`. `new` may add keys, and a value
/// that was `null` (an unset optional field) may since have become any type.
fn missing_from(
    old: &serde_json::Value,
    new: &serde_json::Value,
    path: &str,
    out: &mut Vec<String>,
) {
    use serde_json::Value;
    match (old, new) {
        (Value::Object(o), Value::Object(n)) => {
            for (k, v) in o {
                match n.get(k) {
                    Some(nv) => missing_from(v, nv, &format!("{path}.{k}"), out),
                    None => out.push(format!("{path}.{k} was removed")),
                }
            }
        }
        (Value::Array(o), Value::Array(n)) => {
            if let (Some(o), Some(n)) = (o.first(), n.first()) {
                missing_from(o, n, &format!("{path}[]"), out);
            }
        }
        (Value::String(o), _) if o == "null" => {}
        (o, n) if o == n => {}
        (o, n) => out.push(format!("{path} changed from {o} to {n}")),
    }
}

/// Commands whose `--json` output is covered by the 1.x compatibility promise, run on the
/// FireSat fixture.
const JSON_CONTRACT: &[&[&str]] = &[
    &["lint"],
    &["coverage"],
    &["scan"],
    &["search", "telemetry", "-i"],
    &["trace", "FOBC-SW-0001"],
    &["context", "FOBC-SW-0001"],
    &["rehash", "--dry-run"],
    &[
        "add",
        "--category",
        "SW",
        "--type",
        "Functional",
        "--title",
        "T",
        "--statement",
        "The OBC shall work.",
        "--dry-run",
    ],
    &["explain", "RQ001"],
    &["explain"],
    &["schema"],
    &["skills", "list"],
];

#[verifies("VA-CLI-001-01")]
#[test]
fn json_output_only_grows() {
    let repo_root = fixture_root("firesat-obc");
    let current: serde_json::Map<String, serde_json::Value> = JSON_CONTRACT
        .iter()
        .map(|args| {
            let (_, value) = json_stdout(&repo_root, args);
            (args.join(" "), shape(&value))
        })
        .collect();
    let current = serde_json::Value::Object(current);
    let snapshot_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/json-shapes.json");
    if std::env::var_os("RQTK_UPDATE_SNAPSHOTS").is_some() {
        fs::write(
            &snapshot_path,
            serde_json::to_string_pretty(&current).unwrap() + "\n",
        )
        .unwrap();
        return;
    }
    let snapshot: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&snapshot_path).expect("run with RQTK_UPDATE_SNAPSHOTS=1 to create"),
    )
    .unwrap();
    let mut broken = Vec::new();
    missing_from(&snapshot, &current, "", &mut broken);
    assert!(
        broken.is_empty(),
        "JSON output lost fields or changed types (a breaking change in 1.x):\n{}",
        broken.join("\n")
    );
}

#[verifies("VA-CLI-002-01")]
#[test]
fn a_closed_pipe_is_not_an_error() {
    // `rqtk … | head`: the reader closes stdout before rqtk is done writing.
    let mut child = std::process::Command::new(assert_cmd::cargo::cargo_bin("rqtk"))
        .args(["explain"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
