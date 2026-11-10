//! The published JSON Schemas: every `--json` document validates against its schema, and the
//! files under `schema/` are what `rqtk schema` prints.

use assert_cmd::Command;
use rqtk_macros::verifies;
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn rqtk(root: &Path) -> Command {
    let mut cmd = Command::cargo_bin("rqtk").unwrap();
    cmd.arg("--repo-root").arg(root);
    cmd
}

fn git(dir: &Path, args: &[&str]) {
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

/// The FireSat fixture as its own git repository, committed and tagged `v1`.
fn firesat(dir: &Path, name: &str) -> PathBuf {
    let root = dir.join(name);
    copy_dir(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/firesat-obc"),
        &root,
    );
    git(&root, &["init", "-q"]);
    git(&root, &["config", "user.name", "Test"]);
    git(&root, &["config", "user.email", "test@example.com"]);
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "v1"]);
    git(&root, &["tag", "v1"]);
    root
}

fn schema(kind: &str) -> Value {
    let out = Command::cargo_bin("rqtk")
        .unwrap()
        .args(["schema", kind])
        .output()
        .unwrap();
    assert!(out.status.success(), "rqtk schema {kind}");
    serde_json::from_slice(&out.stdout).unwrap()
}

/// Every kind `rqtk schema` lists, as `(kind, category)`.
fn kinds() -> Vec<(String, String)> {
    let out = Command::cargo_bin("rqtk")
        .unwrap()
        .args(["--json", "schema"])
        .output()
        .unwrap();
    let listing: Value = serde_json::from_slice(&out.stdout).unwrap();
    listing["kinds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| {
            (
                k["kind"].as_str().unwrap().to_owned(),
                k["category"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

#[verifies("VA-CLI-0028-01")]
#[test]
fn every_json_document_validates_against_its_schema() {
    let dir = tempfile::tempdir().unwrap();
    let root = firesat(dir.path(), "obc");
    let provider = firesat(dir.path(), "provider");
    let empty = dir.path().join("empty");
    fs::create_dir_all(&empty).unwrap();
    git(&empty, &["init", "-q"]);
    let junit = dir.path().join("junit.xml");
    fs::write(
        &junit,
        r#"<?xml version="1.0"?><testsuites><testsuite name="t"><testcase classname="obc" name="frame_is_sent"/></testsuite></testsuites>"#,
    )
    .unwrap();
    let export = dir.path().join("export.json");
    let provider_url = format!("file://{}", provider.display());

    let s = |v: &str| v.to_owned();
    let runs: Vec<(&str, PathBuf, Vec<String>)> = vec![
        ("lint", root.clone(), vec![s("lint")]),
        ("coverage", root.clone(), vec![s("coverage")]),
        (
            "context",
            root.clone(),
            vec![s("context"), s("FOBC-SW-0001")],
        ),
        ("trace", root.clone(), vec![s("trace"), s("FOBC-SW-0001")]),
        ("impact", root.clone(), vec![s("impact"), s("v1")]),
        ("diff", root.clone(), vec![s("diff"), s("v1"), s("HEAD")]),
        ("baselines", root.clone(), vec![s("baseline"), s("--list")]),
        ("history", root.clone(), vec![s("log"), s("FOBC-SW-0001")]),
        (
            "search",
            root.clone(),
            vec![s("search"), s("telemetry"), s("-i")],
        ),
        ("scan", root.clone(), vec![s("scan")]),
        (
            "verify",
            root.clone(),
            vec![
                s("verify"),
                s("--results"),
                junit.display().to_string(),
                s("--dry-run"),
            ],
        ),
        (
            "review",
            root.clone(),
            vec![s("review"), s("FOBC-SW-0001"), s("--dry-run")],
        ),
        (
            "rehash",
            root.clone(),
            vec![s("rehash"), s("--all"), s("--dry-run")],
        ),
        (
            "baseline",
            root.clone(),
            vec![s("baseline"), s("1.0.0"), s("--dry-run")],
        ),
        (
            "export",
            root.clone(),
            vec![
                s("export"),
                s("--format"),
                s("json"),
                s("--output"),
                export.display().to_string(),
            ],
        ),
        (
            "add-requirement",
            root.clone(),
            [
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
            ]
            .map(s)
            .to_vec(),
        ),
        (
            "add-need",
            root.clone(),
            [
                "add-need",
                "--title",
                "T",
                "--statement",
                "Operators need it.",
                "--dry-run",
            ]
            .map(s)
            .to_vec(),
        ),
        (
            "add-stakeholder",
            root.clone(),
            ["add-stakeholder", "--name", "Operator", "--dry-run"]
                .map(s)
                .to_vec(),
        ),
        (
            "add-activity",
            root.clone(),
            [
                "add-activity",
                "FOBC-SW-0001",
                "--name",
                "More",
                "--dry-run",
            ]
            .map(s)
            .to_vec(),
        ),
        ("init", empty.clone(), vec![s("init"), s("--dry-run")]),
        (
            "init-agents",
            empty.clone(),
            vec![s("init"), s("--agents"), s("--dry-run")],
        ),
        ("install-hook", root.clone(), vec![s("install-hook")]),
        ("skills", root.clone(), vec![s("skills"), s("list")]),
        (
            "skills-install",
            root.clone(),
            vec![s("skills"), s("install"), s("--dry-run")],
        ),
        (
            "interfaces-add",
            root.clone(),
            vec![
                s("interfaces"),
                s("add"),
                s("obc"),
                provider_url.clone(),
                s("--ref"),
                s("v1"),
                s("--allow-file-url"),
            ],
        ),
        (
            "interfaces-sync",
            root.clone(),
            vec![s("interfaces"), s("sync"), s("--allow-file-url")],
        ),
        (
            "interfaces-list",
            root.clone(),
            vec![s("interfaces"), s("list")],
        ),
        (
            "interfaces-remove",
            root.clone(),
            vec![s("interfaces"), s("remove"), s("obc")],
        ),
        ("explain", root.clone(), vec![s("explain")]),
        ("schema-kinds", root.clone(), vec![s("schema")]),
    ];

    let mut validated = BTreeSet::new();
    for (kind, dir, args) in &runs {
        let out = rqtk(dir).arg("--json").args(args).output().unwrap();
        let document: Value = serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
            panic!(
                "`rqtk {}` did not print JSON ({e}):\n{}\n{}",
                args.join(" "),
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            )
        });
        validate(kind, &document, args);
        validated.insert(kind.to_string());
    }
    // An error, on stderr.
    let out = rqtk(&root)
        .args(["--json", "context", "NOPE"])
        .output()
        .unwrap();
    validate(
        "error",
        &serde_json::from_slice(&out.stderr).unwrap(),
        &["context".into(), "NOPE".into()],
    );
    validated.insert("error".to_owned());

    let documents: BTreeSet<String> = kinds()
        .into_iter()
        .filter(|(_, category)| category == "document")
        .map(|(kind, _)| kind)
        .collect();
    let missing: Vec<&String> = documents.difference(&validated).collect();
    assert!(missing.is_empty(), "documents never validated: {missing:?}");
}

fn validate(kind: &str, document: &Value, args: &[String]) {
    let schema = schema(kind);
    let validator = jsonschema::validator_for(&schema)
        .unwrap_or_else(|e| panic!("schema `{kind}` is not a valid JSON Schema: {e}"));
    let errors: Vec<String> = validator
        .iter_errors(document)
        .map(|e| format!("{} at {}", e, e.instance_path()))
        .collect();
    assert!(
        errors.is_empty(),
        "`rqtk {}` does not match schema `{kind}`:\n{}",
        args.join(" "),
        errors.join("\n")
    );
}

/// Where kind `kind` of `category` is published.
fn published_path(kind: &str, category: &str) -> PathBuf {
    let schema_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schema");
    match category {
        "file" => schema_dir.join(format!("{kind}.schema.json")),
        "document" => schema_dir
            .join("output")
            .join(format!("{kind}.schema.json")),
        _ => schema_dir
            .join("client")
            .join(format!("{kind}.schema.json")),
    }
}

#[verifies("VA-CLI-0028-01")]
#[test]
fn published_schemas_are_current() {
    let update = std::env::var_os("RQTK_UPDATE_SNAPSHOTS").is_some();
    let mut stale = Vec::new();
    for (kind, category) in kinds() {
        let current = schema(&kind);
        let path = published_path(&kind, &category);
        // File schemas are written by `cargo run -p rqtk-core --bin generate_schemas`.
        if update && category != "file" {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(
                &path,
                serde_json::to_string_pretty(&current).unwrap() + "\n",
            )
            .unwrap();
            continue;
        }
        let published: Option<Value> = fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok());
        if published.as_ref() != Some(&current) {
            stale.push(path.display().to_string());
        }
    }
    assert!(
        stale.is_empty(),
        "published schemas differ from `rqtk schema`; run with RQTK_UPDATE_SNAPSHOTS=1:\n{}",
        stale.join("\n")
    );
}
