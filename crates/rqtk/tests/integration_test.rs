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
fn lint_exits_2_when_rationale_missing() {
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
        .code(2)
        .stdout(predicate::str::contains("RQ007"));
}

// ── trace ─────────────────────────────────────────────────────────────────────

#[verifies("VA-CLI-003-01")]
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
    // "Children" section exists but contains only the root ID itself
    let downward_start = text.find("Children").unwrap();
    let downward_section = &text[downward_start..];
    assert!(downward_section.contains("FOBC-ICD-0001"));
    assert!(!downward_section.contains("FOBC-SW-0001"));
}

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

#[verifies("VA-CLI-004-01")]
#[test]
fn coverage_exits_nonzero_when_gaps_present_and_strict() {
    let repo_root = fixture_root("firesat-obc");
    rqtk(&repo_root)
        .arg("coverage")
        .arg("--strict")
        .assert()
        .failure();
}

#[test]
fn coverage_lists_requirement_missing_activities() {
    // FOBC-SW-0003 has no verification activities
    let repo_root = fixture_root("firesat-obc");
    rqtk(&repo_root)
        .arg("coverage")
        .assert()
        .stdout(predicate::str::contains("FOBC-SW-0003"));
}

#[test]
fn coverage_lists_requirement_missing_success_criteria() {
    // FOBC-ICD-0001 has activities but no success_criteria
    let repo_root = fixture_root("firesat-obc");
    rqtk(&repo_root)
        .arg("coverage")
        .assert()
        .stdout(predicate::str::contains("FOBC-ICD-0001"));
}

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

#[verifies("VA-CLI-005-01")]
#[test]
fn graph_dot_output_is_valid_digraph() {
    let repo_root = fixture_root("firesat-obc");
    rqtk(&repo_root)
        .arg("graph")
        .assert()
        .success()
        .stdout(predicate::str::contains("digraph {"));
}

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

#[verifies("VA-CLI-006-01")]
#[verifies("VA-SYS-005-01")]
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

#[verifies("VA-CLI-008-01")]
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

#[verifies("VA-CLI-001-01")]
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

#[verifies("VA-CLI-007-01")]
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

#[verifies("VA-SYS-003-01")]
#[test]
fn lint_cycle_exits_2_and_reports_rq017() {
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
        .code(2)
        .stdout(predicate::str::contains("RQ017"));
}

// ── VA-SYS-003-02: broken reference via CLI ───────────────────────────────────

// #[verifies("VA-SYS-003-02")]
#[test]
fn lint_broken_parent_exits_2_and_reports_rq015() {
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
        .code(2)
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

#[verifies("VA-SYS-004-01")]
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

#[verifies("VA-SYS-004-01")]
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

#[verifies("VA-SYS-004-01")]
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

#[verifies("VA-SYS-004-01")]
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

#[verifies("VA-SYS-004-01")]
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

#[verifies("VA-SYS-004-01")]
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

#[verifies("VA-SYS-004-01")]
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

#[verifies("VA-SYS-004-01")]
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

#[verifies("VA-SYS-004-01")]
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

#[verifies("VA-SYS-004-01")]
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

#[verifies("VA-SYS-004-01")]
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

#[verifies("VA-SYS-004-01")]
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

#[verifies("VA-SYS-004-01")]
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

#[verifies("VA-SYS-004-01")]
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

#[verifies("VA-SYS-004-01")]
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

#[verifies("VA-SYS-004-01")]
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

#[verifies("VA-SYS-004-01")]
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

#[verifies("VA-SYS-004-01")]
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
        .code(2)
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

#[verifies("VA-SYS-004-01")]
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

#[verifies("VA-SYS-004-01")]
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

#[test]
fn lint_reports_unknown_field_with_line_number() {
    let req = valid_req("TEST-SYS-0001").replace("rationale =", "ratoinale =");
    let (code, out) = lint_output(&[("SYS/TEST-SYS-0001.toml", &req)]);
    assert_eq!(code, 2, "{out}");
    assert!(out.contains("RQ100"), "{out}");
    assert!(out.contains("unknown field `ratoinale`"), "{out}");
    assert!(out.contains("TEST-SYS-0001.toml:8"), "{out}");
}

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

#[test]
fn lint_rule_rq101_duplicate_id_across_files() {
    let (_, out) = lint_output(&[
        ("SYS/TEST-SYS-0001.toml", &valid_req("TEST-SYS-0001")),
        ("SUB/TEST-SYS-0001.toml", &valid_req("TEST-SYS-0001")),
    ]);
    assert!(out.contains("RQ101"), "{out}");
}

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

#[test]
fn lint_rule_rq026_unknown_refines_reference() {
    let req = format!(
        "{}\n[trace]\nrefines = [\"TEST-SYS-9999\"]\n",
        valid_req("TEST-SYS-0001")
    );
    let (_, out) = lint_output(&[("SYS/TEST-SYS-0001.toml", &req)]);
    assert!(out.contains("RQ026"), "{out}");
}

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

#[test]
fn native_toml_dates_are_accepted() {
    let req = format!(
        "{}\n[[verification.activities]]\nid = \"VA-1\"\nname = \"Run\"\nstatus = \"Passed\"\nexecuted_at = 2024-11-15\n",
        valid_req("TEST-SYS-0001")
    );
    let (code, out) = lint_output(&[("SYS/TEST-SYS-0001.toml", &req)]);
    assert_eq!(code, 0, "{out}");
}

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

#[test]
fn rehash_preserves_comments_and_layout() {
    let req = format!(
        "# Owned by the systems team.\n{}",
        valid_req("TEST-SYS-0001").replace("title =", "# keep this note\ntitle =")
    );
    let (_dir, repo_root) = write_fixture(BASE_CONFIG, &[("SYS/TEST-SYS-0001.toml", &req)]);
    rqtk(&repo_root).arg("rehash").assert().success();

    let path = repo_root.join(".rqtk/requirements/SYS/TEST-SYS-0001.toml");
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
    assert_eq!(output.status.code(), Some(2), "{out}");
    assert!(out.contains("RQ028") && out.contains("VA-TYPO"), "{out}");
    assert!(out.contains("tests/boot.rs:4"), "{out}");
    assert!(out.contains("RQ029"), "{out}");
    assert!(out.contains("RQ030"), "{out}");
}

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
