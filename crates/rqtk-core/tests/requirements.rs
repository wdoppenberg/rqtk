use rqtk_core::{RequirementId, RequirementSet};
use rqtk_macros::verifies;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

// ── helpers ───────────────────────────────────────────────────────────────────

/// The minimal config that works for all "TEST-" tests.
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

/// A single valid requirement as a TOML string. The caller can replace fields as needed.
fn valid_req(id: &str, category: &str) -> String {
    format!(
        r#"id = "{id}"
title = "Test requirement"
category = "{category}"
type = "Functional"
state = "Draft"
priority = "Critical"
statement = "The system shall do something."
rationale = "Because it needs to."

[verification]
method = "Test"
level = "System"
phase = "Development"
"#
    )
}

/// Write `.rqtk/config.toml` + requirement TOML files to a temp dir (with a git repo)
/// and return the temp dir handle and root path.
fn minimal_fixture(config: &str, reqs: &[(&str, &str)]) -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    gix::init(dir.path()).expect("git init failed");
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
    let root = dir.path().to_path_buf();
    (dir, root)
}

// ── VA-SYS-001-01: lifecycle round-trip ──────────────────────────────────────

#[verifies("VA-SYS-001-01")]
#[test]
fn lifecycle_round_trip() {
    let states = [
        "Draft",
        "Review",
        "Approved",
        "Implemented",
        "Verified",
        "Deprecated",
    ];

    for state in &states {
        let req_toml = format!(
            r#"id = "TEST-SYS-0001"
title = "Round-trip req"
category = "SYS"
type = "Functional"
state = "{state}"
priority = "Critical"
statement = "The system shall do something."
rationale = "Because it needs to."

[verification]
method = "Test"
level = "System"
phase = "Development"
"#
        );

        let (_dir, root) = minimal_fixture(BASE_CONFIG, &[("SYS/TEST-SYS-0001.toml", &req_toml)]);
        let set = RequirementSet::load_from_repo_root(&root)
            .unwrap_or_else(|e| panic!("failed to load for state {state}: {e}"));

        let req = set
            .requirements
            .get(&RequirementId("TEST-SYS-0001".to_owned()))
            .unwrap_or_else(|| panic!("requirement not found for state {state}"));

        assert_eq!(
            req.state, *state,
            "state mismatch after round-trip for {state}"
        );
        assert_eq!(
            req.id,
            RequirementId("TEST-SYS-0001".to_owned()),
            "id mismatch after round-trip for {state}"
        );
        assert_eq!(
            req.title, "Round-trip req",
            "title mismatch after round-trip for {state}"
        );
    }
}

// ── VA-SYS-002-01: file format inspection ────────────────────────────────────

#[verifies("VA-SYS-002-01")]
#[test]
fn load_project_requirements_without_errors() {
    // Load the actual workspace requirements directory (the project dogfoods itself).
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.parent().unwrap().parent().unwrap();

    let set = RequirementSet::load_from_repo_root(workspace_root)
        .expect("should load all project requirement files without parse errors");

    assert!(
        !set.requirements.is_empty(),
        "expected at least one requirement to be loaded"
    );

    for id in set.requirements.keys() {
        assert!(!id.0.is_empty(), "requirement ID must not be empty");
    }
}

// ── VA-CORE-002-01: cycle detection unit test ─────────────────────────────────

#[verifies("VA-CORE-002-01")]
#[test]
fn cycle_detection_unit_produces_rq017() {
    // A.parents = [B], B.parents = [A] — circular
    // Both are SYS (root) so no orphan issues. forbid_circular_traces = true.
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

    let (_dir, root) = minimal_fixture(
        BASE_CONFIG,
        &[
            ("SYS/TEST-SYS-0001.toml", req_a),
            ("SYS/TEST-SYS-0002.toml", req_b),
        ],
    );

    let set = RequirementSet::load_from_repo_root(&root).unwrap();
    let (_, issues) = set.validate();

    let has_rq017 = issues.iter().any(|i| i.code == "RQ017");
    assert!(
        has_rq017,
        "expected RQ017 cycle detection issue; got: {:?}",
        issues.iter().map(|i| i.code).collect::<Vec<_>>()
    );
}

// ── VA-CORE-003-01: ID generation unit test ──────────────────────────────────

#[verifies("VA-CORE-003-01")]
#[test]
fn next_requirement_id_increments_correctly() {
    let req1 = valid_req("TEST-SYS-0001", "SYS");
    let req2 = valid_req("TEST-SYS-0002", "SYS");

    let (_dir, root) = minimal_fixture(
        BASE_CONFIG,
        &[
            ("SYS/TEST-SYS-0001.toml", &req1),
            ("SYS/TEST-SYS-0002.toml", &req2),
        ],
    );

    let set = RequirementSet::load_from_repo_root(&root).unwrap();
    let next_id = set.next_requirement_id("SYS");
    assert_eq!(next_id, RequirementId("TEST-SYS-0003".to_owned()));
}

// ── VA-CORE-004-01: config-driven policy ─────────────────────────────────────

#[verifies("VA-CORE-004-01")]
#[test]
fn unknown_category_produces_rq002() {
    let req_toml = r#"id = "TEST-SYS-0001"
title = "Test"
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

    let (_dir, root) = minimal_fixture(BASE_CONFIG, &[("SYS/TEST-SYS-0001.toml", req_toml)]);
    let set = RequirementSet::load_from_repo_root(&root).unwrap();
    let (_, issues) = set.validate();

    let has_rq002 = issues.iter().any(|i| i.code == "RQ002");
    assert!(
        has_rq002,
        "expected RQ002 for unknown category; got: {:?}",
        issues.iter().map(|i| i.code).collect::<Vec<_>>()
    );
}

#[test]
#[verifies("VA-CORE-004-01")]
fn known_category_suppresses_rq002() {
    // Add "EXTRA" category to config; the requirement uses it; RQ002 should not fire.
    let config_with_extra = r#"schema_version = 1

[project]
name = "test"
version = "0.1.0"

[identification]
id_pattern = "^TEST-(SYS|SUB|EXTRA)-\\d{4}$"
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

[categories.EXTRA]
name = "Extra"
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
forbid_orphans = false
forbid_circular_traces = false
allow_tbd = false
allow_tbr = false
shall_keywords = ["shall"]
forbidden_keywords = []
"#;

    let req_toml = r#"id = "TEST-EXTRA-0001"
title = "Test"
category = "EXTRA"
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

    let (_dir, root) = minimal_fixture(
        config_with_extra,
        &[("EXTRA/TEST-EXTRA-0001.toml", req_toml)],
    );
    let set = RequirementSet::load_from_repo_root(&root).unwrap();
    let (_, issues) = set.validate();

    let rq002_issues: Vec<_> = issues.iter().filter(|i| i.code == "RQ002").collect();
    assert!(
        rq002_issues.is_empty(),
        "expected no RQ002 once category is added to config; got: {:?}",
        rq002_issues.iter().map(|i| &i.message).collect::<Vec<_>>()
    );
}
