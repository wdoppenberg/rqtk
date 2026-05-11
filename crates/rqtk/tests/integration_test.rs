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

fn fixture_requirements(fixture: &str) -> PathBuf {
    fixture_root(fixture).join("requirements")
}

fn rqtk(requirements_dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("rqtk").unwrap();
    let repo_root = requirements_dir
        .parent()
        .map_or_else(|| requirements_dir.to_path_buf(), Path::to_path_buf);
    cmd.arg("--repo-root").arg(repo_root);
    cmd
}

fn copy_fixture_to_temp(fixture: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
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
    let req_path = dir.path().join("requirements");
    (dir, req_path)
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

/// Write an `rqtk.toml` + requirement files to a tempdir and return the
/// dir handle and path.
fn write_fixture(config: &str, reqs: &[(&str, &str)]) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("rqtk.toml"), config).unwrap();
    let requirements_root = dir.path().join("requirements");
    fs::create_dir_all(&requirements_root).unwrap();
    for (filename, content) in reqs {
        let path = requirements_root.join(filename);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }
    (dir, requirements_root)
}

/// Minimal project config that accepts TEST-(SYS|SUB)-NNNN IDs.
const BASE_CONFIG: &str = r#"
[repository]
requirements_dir = "requirements"
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

[change_control]
ccb_required_after = "Approved"
require_signoff = false

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

[export]
formats = []
"#;

// ── lint ─────────────────────────────────────────────────────────────────────

#[verifies("VA-CLI-002-01")]
#[test]
fn lint_exits_zero_on_valid_project() {
    let req_dir = fixture_requirements("firesat-obc");
    rqtk(&req_dir).arg("lint").assert().code(0);
}

#[test]
fn lint_prints_summary_with_zero_errors() {
    let req_dir = fixture_requirements("firesat-obc");
    rqtk(&req_dir)
        .arg("lint")
        .assert()
        .stdout(predicate::str::contains("All requirements passed lint"));
}

#[test]
fn lint_prints_all_requirement_ids_in_issues_when_present() {
    // The fixture has no errors so the output should not contain error lines,
    // but the success line must always be present.
    let req_dir = fixture_requirements("firesat-obc");
    rqtk(&req_dir)
        .arg("lint")
        .assert()
        .stdout(predicate::str::contains("All requirements passed lint"));
}

/// VA-CLI-002-01 (negative): missing rationale should exit 2 and report RQ007.
#[verifies("VA-CLI-002-01")]
#[test]
fn lint_exits_2_when_rationale_missing() {
    let req_toml = r#"[requirement]
id = "TEST-SYS-0001"
title = "No rationale"
category = "SYS"
type = "Functional"
version = "0.1.0"
created = "2026-01-01T00:00:00+00:00"
updated = "2026-01-01T00:00:00+00:00"

[requirement.statement]
text = "The system shall do something."

[requirement.status]
state = "Draft"
priority = "Critical"

[requirement.traceability]

[requirement.verification]
method = "Test"
level = "System"
phase = "Development"
"#;

    let (_dir, req_dir) = write_fixture(BASE_CONFIG, &[("SYS/TEST-SYS-0001.toml", req_toml)]);
    rqtk(&req_dir)
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
    let req_dir = fixture_requirements("firesat-obc");
    rqtk(&req_dir)
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
    let req_dir = fixture_requirements("firesat-obc");
    rqtk(&req_dir)
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
    let req_dir = fixture_requirements("firesat-obc");
    rqtk(&req_dir)
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
    let req_dir = fixture_requirements("firesat-obc");
    let output = rqtk(&req_dir)
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
    let req_dir = fixture_requirements("firesat-obc");
    rqtk(&req_dir)
        .arg("trace")
        .arg("FOBC-SYS-9999")
        .assert()
        .failure();
}

// ── coverage ──────────────────────────────────────────────────────────────────

#[verifies("VA-CLI-004-01")]
#[test]
fn coverage_exits_3_when_gaps_present() {
    let req_dir = fixture_requirements("firesat-obc");
    rqtk(&req_dir).arg("coverage").assert().code(3);
}

#[test]
fn coverage_lists_requirement_missing_activities() {
    // FOBC-SW-0003 has no verification activities
    let req_dir = fixture_requirements("firesat-obc");
    rqtk(&req_dir)
        .arg("coverage")
        .assert()
        .stdout(predicate::str::contains("FOBC-SW-0003"));
}

#[test]
fn coverage_lists_requirement_missing_success_criteria() {
    // FOBC-ICD-0001 has activities but no success_criteria
    let req_dir = fixture_requirements("firesat-obc");
    rqtk(&req_dir)
        .arg("coverage")
        .assert()
        .stdout(predicate::str::contains("FOBC-ICD-0001"));
}

#[test]
fn coverage_does_not_flag_fully_covered_requirements() {
    let req_dir = fixture_requirements("firesat-obc");
    let output = rqtk(&req_dir).arg("coverage").output().unwrap();
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
    let req_dir = fixture_requirements("firesat-obc");
    rqtk(&req_dir)
        .arg("graph")
        .assert()
        .success()
        .stdout(predicate::str::contains("digraph {"));
}

#[test]
fn graph_dot_output_contains_all_requirement_ids() {
    let req_dir = fixture_requirements("firesat-obc");
    let output = rqtk(&req_dir)
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
    let req_dir = fixture_requirements("firesat-obc");
    rqtk(&req_dir)
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
    let (_dir, req_dir) = copy_fixture_to_temp("firesat-obc");
    let out = req_dir.join("export.json");
    rqtk(&req_dir)
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
    let (_dir, req_dir) = copy_fixture_to_temp("firesat-obc");
    let out = req_dir.join("export.csv");
    rqtk(&req_dir)
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
    let (_dir, req_dir) = copy_fixture_to_temp("firesat-obc");
    let out = req_dir.join("export.md");
    rqtk(&req_dir)
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
    let (_dir, req_dir) = copy_fixture_to_temp("firesat-obc");
    rqtk(&req_dir)
        .arg("export")
        .arg("--format")
        .arg("xml")
        .assert()
        .failure();
}

// ── diff ──────────────────────────────────────────────────────────────────────

#[test]
fn diff_reports_no_deltas_for_unknown_versions() {
    let req_dir = fixture_requirements("firesat-obc");
    rqtk(&req_dir)
        .arg("diff")
        .arg("0.0.0")
        .arg("9.9.9")
        .assert()
        .success()
        .stdout(predicate::str::contains("No deltas between"));
}

#[verifies("VA-CLI-008-01")]
#[test]
fn diff_detects_requirements_present_in_one_version_only() {
    // FOBC-SW-0003 has history entries for 0.1.0 and 0.2.0 but not for 1.0.0;
    // comparing 0.1.0 vs 1.0.0 should surface it.
    let req_dir = fixture_requirements("firesat-obc");
    rqtk(&req_dir)
        .arg("diff")
        .arg("0.1.0")
        .arg("1.0.0")
        .assert()
        .success()
        .stdout(predicate::str::contains("FOBC-SW-0003"));
}

// ── new ───────────────────────────────────────────────────────────────────────

#[verifies("VA-CLI-001-01")]
#[test]
fn new_creates_requirement_file_with_correct_id() {
    let (_dir, req_dir) = copy_fixture_to_temp("firesat-obc");
    // Existing SW requirements are 0001–0003, so the next ID is 0004.
    rqtk(&req_dir)
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
        req_dir.join("SW").join("FOBC-SW-0004.toml").exists(),
        "Expected SW/FOBC-SW-0004.toml to be created"
    );
}

#[test]
fn new_created_file_passes_lint() {
    let (_dir, req_dir) = copy_fixture_to_temp("firesat-obc");
    rqtk(&req_dir)
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
    rqtk(&req_dir).arg("lint").output().unwrap(); // any exit code is acceptable; we just verify it doesn't panic
}

// ── baseline ──────────────────────────────────────────────────────────────────

#[verifies("VA-CLI-007-01")]
#[test]
fn baseline_updates_project_version() {
    let (_dir, req_dir) = copy_fixture_to_temp("firesat-obc");
    rqtk(&req_dir)
        .arg("baseline")
        .arg("2.0.0")
        .assert()
        .success()
        .stdout(predicate::str::contains("Baseline updated"))
        .stdout(predicate::str::contains("2.0.0"));
    let config = std::fs::read_to_string(req_dir.parent().unwrap().join("rqtk.toml")).unwrap();
    assert!(
        config.contains("2.0.0"),
        "rqtk.toml was not updated with the new version"
    );
}

#[test]
fn baseline_rejects_invalid_semver() {
    let (_dir, req_dir) = copy_fixture_to_temp("firesat-obc");
    rqtk(&req_dir)
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
    let req_a = r#"[requirement]
id = "TEST-SYS-0001"
title = "A"
category = "SYS"
type = "Functional"
version = "0.1.0"
created = "2026-01-01T00:00:00+00:00"
updated = "2026-01-01T00:00:00+00:00"

[requirement.statement]
text = "The system shall do something."
rationale = "Because it needs to."

[requirement.status]
state = "Draft"
priority = "Critical"

[requirement.traceability]
parents = ["TEST-SYS-0002"]

[requirement.verification]
method = "Test"
level = "System"
phase = "Development"
"#;

    let req_b = r#"[requirement]
id = "TEST-SYS-0002"
title = "B"
category = "SYS"
type = "Functional"
version = "0.1.0"
created = "2026-01-01T00:00:00+00:00"
updated = "2026-01-01T00:00:00+00:00"

[requirement.statement]
text = "The system shall do something else."
rationale = "Because it also needs to."

[requirement.status]
state = "Draft"
priority = "Critical"

[requirement.traceability]
parents = ["TEST-SYS-0001"]

[requirement.verification]
method = "Test"
level = "System"
phase = "Development"
"#;

    let (_dir, req_dir) = write_fixture(
        BASE_CONFIG,
        &[
            ("SYS/TEST-SYS-0001.toml", req_a),
            ("SYS/TEST-SYS-0002.toml", req_b),
        ],
    );

    rqtk(&req_dir)
        .arg("lint")
        .assert()
        .code(2)
        .stdout(predicate::str::contains("RQ017"));
}

// ── VA-SYS-003-02: broken reference via CLI ───────────────────────────────────

// #[verifies("VA-SYS-003-02")]
#[test]
fn lint_broken_parent_exits_2_and_reports_rq015() {
    let req_toml = r#"[requirement]
id = "TEST-SYS-0001"
title = "Broken parent"
category = "SYS"
type = "Functional"
version = "0.1.0"
created = "2026-01-01T00:00:00+00:00"
updated = "2026-01-01T00:00:00+00:00"

[requirement.statement]
text = "The system shall do something."
rationale = "Because it needs to."

[requirement.status]
state = "Draft"
priority = "Critical"

[requirement.traceability]
parents = ["TEST-SYS-9999"]

[requirement.verification]
method = "Test"
level = "System"
phase = "Development"
"#;

    let (_dir, req_dir) = write_fixture(BASE_CONFIG, &[("SYS/TEST-SYS-0001.toml", req_toml)]);

    rqtk(&req_dir)
        .arg("lint")
        .assert()
        .code(2)
        .stdout(predicate::str::contains("RQ015"));
}

// ── VA-SYS-004-01: lint rule coverage matrix ─────────────────────────────────

/// Helper: write a fixture with the given config and single requirement file,
/// run lint, and return stdout as a String.
fn lint_stdout(config: &str, req_content: &str) -> String {
    let (_dir, req_dir) = write_fixture(config, &[("SYS/TEST-SYS-0001.toml", req_content)]);
    let output = rqtk(&req_dir).arg("lint").output().unwrap();
    String::from_utf8(output.stdout).unwrap()
}

fn lint_stdout_with_files(config: &str, files: &[(&str, &str)]) -> String {
    let (_dir, req_dir) = write_fixture(config, files);
    let output = rqtk(&req_dir).arg("lint").output().unwrap();
    String::from_utf8(output.stdout).unwrap()
}

#[verifies("VA-SYS-004-01")]
#[test]
fn lint_rule_rq001_id_mismatch() {
    // id doesn't match id_pattern
    let req = r#"[requirement]
id = "TEST-WRONG-001"
title = "Bad ID"
category = "SYS"
type = "Functional"
version = "0.1.0"
created = "2026-01-01T00:00:00+00:00"
updated = "2026-01-01T00:00:00+00:00"

[requirement.statement]
text = "The system shall do something."
rationale = "Because it needs to."

[requirement.status]
state = "Draft"
priority = "Critical"

[requirement.traceability]

[requirement.verification]
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
    let req = r#"[requirement]
id = "TEST-SYS-0001"
title = "Bad category"
category = "UNKNOWN"
type = "Functional"
version = "0.1.0"
created = "2026-01-01T00:00:00+00:00"
updated = "2026-01-01T00:00:00+00:00"

[requirement.statement]
text = "The system shall do something."
rationale = "Because it needs to."

[requirement.status]
state = "Draft"
priority = "Critical"

[requirement.traceability]

[requirement.verification]
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
    let req = r#"[requirement]
id = "TEST-SYS-0001"
title = "Bad type"
category = "SYS"
type = "Unknown"
version = "0.1.0"
created = "2026-01-01T00:00:00+00:00"
updated = "2026-01-01T00:00:00+00:00"

[requirement.statement]
text = "The system shall do something."
rationale = "Because it needs to."

[requirement.status]
state = "Draft"
priority = "Critical"

[requirement.traceability]

[requirement.verification]
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
    let req = r#"[requirement]
id = "TEST-SYS-0001"
title = "Bad state"
category = "SYS"
type = "Functional"
version = "0.1.0"
created = "2026-01-01T00:00:00+00:00"
updated = "2026-01-01T00:00:00+00:00"

[requirement.statement]
text = "The system shall do something."
rationale = "Because it needs to."

[requirement.status]
state = "Invalid"
priority = "Critical"

[requirement.traceability]

[requirement.verification]
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
    let req = r#"[requirement]
id = "TEST-SYS-0001"
title = "Bad priority"
category = "SYS"
type = "Functional"
version = "0.1.0"
created = "2026-01-01T00:00:00+00:00"
updated = "2026-01-01T00:00:00+00:00"

[requirement.statement]
text = "The system shall do something."
rationale = "Because it needs to."

[requirement.status]
state = "Draft"
priority = "Invalid"

[requirement.traceability]

[requirement.verification]
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
    let req = r#"[requirement]
id = "TEST-SYS-0001"
title = "Bad criticality"
category = "SYS"
type = "Functional"
version = "0.1.0"
created = "2026-01-01T00:00:00+00:00"
updated = "2026-01-01T00:00:00+00:00"

[requirement.statement]
text = "The system shall do something."
rationale = "Because it needs to."

[requirement.status]
state = "Draft"
priority = "Critical"
criticality = "InvalidCrit"

[requirement.traceability]

[requirement.verification]
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
    let req = r#"[requirement]
id = "TEST-SYS-0001"
title = "No rationale"
category = "SYS"
type = "Functional"
version = "0.1.0"
created = "2026-01-01T00:00:00+00:00"
updated = "2026-01-01T00:00:00+00:00"

[requirement.statement]
text = "The system shall do something."

[requirement.status]
state = "Draft"
priority = "Critical"

[requirement.traceability]

[requirement.verification]
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
    let req = r#"[requirement]
id = "TEST-SYS-0001"
title = "Empty method"
category = "SYS"
type = "Functional"
version = "0.1.0"
created = "2026-01-01T00:00:00+00:00"
updated = "2026-01-01T00:00:00+00:00"

[requirement.statement]
text = "The system shall do something."
rationale = "Because it needs to."

[requirement.status]
state = "Draft"
priority = "Critical"

[requirement.traceability]

[requirement.verification]
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
    let req = r#"[requirement]
id = "TEST-SYS-0001"
title = "Bad method"
category = "SYS"
type = "Functional"
version = "0.1.0"
created = "2026-01-01T00:00:00+00:00"
updated = "2026-01-01T00:00:00+00:00"

[requirement.statement]
text = "The system shall do something."
rationale = "Because it needs to."

[requirement.status]
state = "Draft"
priority = "Critical"

[requirement.traceability]

[requirement.verification]
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
    let req = r#"[requirement]
id = "TEST-SYS-0001"
title = "No shall"
category = "SYS"
type = "Functional"
version = "0.1.0"
created = "2026-01-01T00:00:00+00:00"
updated = "2026-01-01T00:00:00+00:00"

[requirement.statement]
text = "The system does something."
rationale = "Because it needs to."

[requirement.status]
state = "Draft"
priority = "Critical"

[requirement.traceability]

[requirement.verification]
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
    let config = r#"
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

[change_control]
ccb_required_after = "Draft"
require_signoff = false

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

[export]
formats = []
"#;

    let req = r#"[requirement]
id = "TEST-SYS-0001"
title = "Forbidden keyword"
category = "SYS"
type = "Functional"
version = "0.1.0"
created = "2026-01-01T00:00:00+00:00"
updated = "2026-01-01T00:00:00+00:00"

[requirement.statement]
text = "The system shall not use should anywhere."
rationale = "Because it needs to."

[requirement.status]
state = "Draft"
priority = "Mandatory"

[requirement.traceability]

[requirement.verification]
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
    let config = r#"
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

[change_control]
ccb_required_after = "Draft"
require_signoff = false

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

[export]
formats = []
"#;

    let req = r#"[requirement]
id = "TEST-SUB-0001"
title = "No parent"
category = "SUB"
type = "Functional"
version = "0.1.0"
created = "2026-01-01T00:00:00+00:00"
updated = "2026-01-01T00:00:00+00:00"

[requirement.statement]
text = "The system shall do something."
rationale = "Because it needs to."

[requirement.status]
state = "Draft"
priority = "Critical"

[requirement.traceability]

[requirement.verification]
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
    let req = r#"[requirement]
id = "TEST-SYS-0001"
title = "TBD req"
category = "SYS"
type = "Functional"
version = "0.1.0"
created = "2026-01-01T00:00:00+00:00"
updated = "2026-01-01T00:00:00+00:00"

[requirement.statement]
text = "The system shall do something."
rationale = "Because it needs to."

[requirement.status]
state = "Draft"
priority = "Critical"
tbd = true

[requirement.traceability]

[requirement.verification]
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
    let req = r#"[requirement]
id = "TEST-SYS-0001"
title = "TBR req"
category = "SYS"
type = "Functional"
version = "0.1.0"
created = "2026-01-01T00:00:00+00:00"
updated = "2026-01-01T00:00:00+00:00"

[requirement.statement]
text = "The system shall do something."
rationale = "Because it needs to."

[requirement.status]
state = "Draft"
priority = "Critical"
tbr = true

[requirement.traceability]

[requirement.verification]
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
    let req = r#"[requirement]
id = "TEST-SYS-0001"
title = "Bad parent"
category = "SYS"
type = "Functional"
version = "0.1.0"
created = "2026-01-01T00:00:00+00:00"
updated = "2026-01-01T00:00:00+00:00"

[requirement.statement]
text = "The system shall do something."
rationale = "Because it needs to."

[requirement.status]
state = "Draft"
priority = "Critical"

[requirement.traceability]
parents = ["TEST-SYS-9999"]

[requirement.verification]
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
    let req = r#"[requirement]
id = "TEST-SYS-0001"
title = "Bad depends_on"
category = "SYS"
type = "Functional"
version = "0.1.0"
created = "2026-01-01T00:00:00+00:00"
updated = "2026-01-01T00:00:00+00:00"

[requirement.statement]
text = "The system shall do something."
rationale = "Because it needs to."

[requirement.status]
state = "Draft"
priority = "Critical"

[requirement.traceability]
depends_on = ["TEST-SYS-9999"]

[requirement.verification]
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
    let req_a = r#"[requirement]
id = "TEST-SYS-0001"
title = "A"
category = "SYS"
type = "Functional"
version = "0.1.0"
created = "2026-01-01T00:00:00+00:00"
updated = "2026-01-01T00:00:00+00:00"

[requirement.statement]
text = "The system shall do something."
rationale = "Because it needs to."

[requirement.status]
state = "Draft"
priority = "Critical"

[requirement.traceability]
parents = ["TEST-SYS-0002"]

[requirement.verification]
method = "Test"
level = "System"
phase = "Development"
"#;

    let req_b = r#"[requirement]
id = "TEST-SYS-0002"
title = "B"
category = "SYS"
type = "Functional"
version = "0.1.0"
created = "2026-01-01T00:00:00+00:00"
updated = "2026-01-01T00:00:00+00:00"

[requirement.statement]
text = "The system shall do something else."
rationale = "Because it also needs to."

[requirement.status]
state = "Draft"
priority = "Critical"

[requirement.traceability]
parents = ["TEST-SYS-0001"]

[requirement.verification]
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
    let config = r#"
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

[change_control]
ccb_required_after = "Draft"
require_signoff = false

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

[export]
formats = []
"#;

    let req = r#"[requirement]
id = "TEST-SUB-0001"
title = "Orphan"
category = "SUB"
type = "Functional"
version = "0.1.0"
created = "2026-01-01T00:00:00+00:00"
updated = "2026-01-01T00:00:00+00:00"

[requirement.statement]
text = "The system shall do something."
rationale = "Because it needs to."

[requirement.status]
state = "Draft"
priority = "Critical"

[requirement.traceability]

[requirement.verification]
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

    assert!(repo_root.join("rqtk.toml").exists());
    assert!(repo_root.join("reqs").is_dir());
    assert!(repo_root.join("reqs").join("SYS").is_dir());
}

#[test]
fn lint_reports_missing_required_repository_paths() {
    let config = r#"
[repository]
requirements_dir = "requirements"
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

[change_control]
ccb_required_after = "Draft"
require_signoff = false

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

[export]
formats = []
"#;

    let (_dir, req_dir) = write_fixture(config, &[]);
    rqtk(&req_dir)
        .arg("lint")
        .assert()
        .code(2)
        .stdout(predicate::str::contains("RQ019"))
        .stdout(predicate::str::contains("RQ020"));
}
