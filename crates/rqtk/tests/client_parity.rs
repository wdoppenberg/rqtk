//! The Rust client and the CLI report the same thing: each client view equals the JSON the
//! matching command prints, and status at a ref equals coverage in a checkout of that ref.

use assert_cmd::Command;
use rqtk_client::Repository;
use rqtk_macros::verifies;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

fn git(dir: &Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "tag.gpgsign=false",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn cli(root: &Path, args: &[&str]) -> Value {
    let out = Command::cargo_bin("rqtk")
        .unwrap()
        .arg("--repo-root")
        .arg(root)
        .arg("--json")
        .args(args)
        .output()
        .unwrap();
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "`rqtk {}` ({e}): {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr)
        )
    })
}

fn json<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap()
}

/// The FireSat fixture with history: at `v1` (also baseline `rqtk/1.0.0`) a test verifies
/// FOBC-VA-SW-001-01 and evidence is recorded for it; at HEAD the test and its evidence are gone
/// and a requirement was reworded.
fn repository(dir: &Path) -> PathBuf {
    let root = dir.join("obc");
    copy_dir(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/firesat-obc"),
        &root,
    );
    fs::create_dir_all(root.join("tests")).unwrap();
    fs::write(
        root.join("tests/frames.rs"),
        "#[verifies(\"FOBC-VA-SW-001-01\")]\n#[test]\nfn frame_is_sent() {}\n",
    )
    .unwrap();
    git(&root, &["init", "-q"]);
    git(&root, &["config", "user.name", "Test"]);
    git(&root, &["config", "user.email", "test@example.com"]);
    let junit = dir.join("junit.xml");
    fs::write(
        &junit,
        r#"<?xml version="1.0"?><testsuites><testsuite name="t"><testcase classname="frames" name="frame_is_sent"/></testsuite></testsuites>"#,
    )
    .unwrap();
    Command::cargo_bin("rqtk")
        .unwrap()
        .arg("--repo-root")
        .arg(&root)
        .args(["verify", "--results"])
        .arg(&junit)
        .assert()
        .success();
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "v1"]);
    git(&root, &["tag", "v1"]);
    git(
        &root,
        &[
            "tag",
            "-a",
            "rqtk/1.0.0",
            "-m",
            "Requirements baseline 1.0.0",
        ],
    );

    fs::remove_file(root.join("tests/frames.rs")).unwrap();
    fs::remove_file(root.join(".rqtk/evidence.toml")).unwrap();
    let sw2 = root.join(".rqtk/requirements/SW/FOBC-SW-0002.toml");
    let text = fs::read_to_string(&sw2).unwrap();
    let reworded = text.replacen("shall", "shall always", 1);
    assert_ne!(text, reworded);
    fs::write(&sw2, reworded).unwrap();
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "v2"]);
    root
}

#[verifies("VA-CLIENT-0012-01")]
#[test]
fn client_views_equal_the_cli_json() {
    let dir = tempfile::tempdir().unwrap();
    let root = repository(dir.path());
    let repo = Repository::open(&root).unwrap();
    let id = "FOBC-SW-0001";
    let pairs: Vec<(Vec<&str>, Value)> = vec![
        (vec!["context", id], json(&repo.context(id).unwrap())),
        (
            vec!["context", "FOBC-SW-0002"],
            json(&repo.context("FOBC-SW-0002").unwrap()),
        ),
        (vec!["trace", id], json(&repo.trace(id).unwrap())),
        (vec!["coverage"], json(&repo.coverage())),
        (vec!["lint"], json(&repo.lint())),
        (vec!["impact", "v1"], json(&repo.impact("v1").unwrap())),
        (
            vec!["diff", "v1", "HEAD"],
            json(&repo.diff("v1", "HEAD").unwrap()),
        ),
        (vec!["baseline", "--list"], json(&repo.baselines().unwrap())),
        (
            vec!["log", "FOBC-SW-0002"],
            json(&repo.history("FOBC-SW-0002").unwrap()),
        ),
    ];
    for (args, client) in pairs {
        assert_eq!(cli(&root, &args), client, "rqtk {}", args.join(" "));
    }
    // The fixture exercises what the views report, not empty documents.
    let impact = json(&repo.impact("v1").unwrap());
    assert!(
        !impact["requirements"].as_array().unwrap().is_empty(),
        "{impact}"
    );
    assert_eq!(
        json(&repo.diff("v1", "HEAD").unwrap())["modified"][0]["id"],
        "FOBC-SW-0002"
    );
    assert_eq!(
        json(&repo.baselines().unwrap())["baselines"][0]["name"],
        "1.0.0"
    );
    assert_eq!(
        json(&repo.history("FOBC-SW-0002").unwrap())["commits"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[verifies("VA-CLIENT-0006-01")]
#[test]
fn status_at_a_tag_equals_coverage_in_its_checkout() {
    let dir = tempfile::tempdir().unwrap();
    let root = repository(dir.path());
    let checkout = dir.path().join("checkout");
    git(
        dir.path(),
        &[
            "clone",
            "-q",
            "--branch",
            "v1",
            &root.display().to_string(),
            &checkout.display().to_string(),
        ],
    );
    let at_tag = Repository::open_ref(&root, "v1").unwrap();
    assert_eq!(json(&at_tag.coverage()), cli(&checkout, &["coverage"]));
    let status = |coverage: &Value| {
        coverage["requirements"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == "FOBC-SW-0001")
            .unwrap()["activities"][0]["state"]
            .clone()
    };
    assert_eq!(
        status(&json(&at_tag.coverage())),
        "passed",
        "the tag's test and evidence"
    );
    let head = Repository::open(&root).unwrap();
    assert_ne!(
        status(&json(&head.coverage())),
        "passed",
        "HEAD has neither, so the ref really was used"
    );
}
