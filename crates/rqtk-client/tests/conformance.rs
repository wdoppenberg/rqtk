//! The conformance suite in Rust: each case in `conformance/cases.json`, compared with its
//! `conformance/expected/<name>.json`. Python and TypeScript run the same cases against the
//! same files; see `conformance/README.md`.
//!
//! `RQTK_BLESS=1 cargo test -p rqtk-client --test conformance` rewrites the expected files.

use rqtk_client::{Filter, RemoteOptions, Repository};
use rqtk_macros::verifies;
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

const REPO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

struct Suite {
    _dir: tempfile::TempDir,
    root: PathBuf,
    cases: Vec<Value>,
    fixtures: Value,
}

/// The suite, with its fixtures built once in a temporary directory.
fn suite() -> &'static Suite {
    static SUITE: OnceLock<Suite> = OnceLock::new();
    SUITE.get_or_init(|| {
        let doc: Value = serde_json::from_str(
            &fs::read_to_string(Path::new(REPO).join("conformance/cases.json")).unwrap(),
        )
        .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        // SAFETY: set once, while every other test in this binary waits on `SUITE`.
        unsafe { std::env::set_var("RQTK_CACHE_DIR", root.join("cache")) };
        for (name, spec) in doc["fixtures"].as_object().unwrap() {
            build(&root.join(name), spec);
        }
        Suite {
            _dir: dir,
            root,
            cases: doc["cases"].as_array().unwrap().clone(),
            fixtures: doc["fixtures"].clone(),
        }
    })
}

fn build(dir: &Path, spec: &Value) {
    if let Some(bundle) = spec["bundle"].as_str() {
        let out = Command::new("git")
            .arg("clone")
            .arg("-q")
            .arg(Path::new(REPO).join(bundle))
            .arg(dir)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    } else if let Some(source) = spec["copy"].as_str() {
        copy(&Path::new(REPO).join(source), dir);
    } else {
        fs::create_dir_all(dir).unwrap();
        for (rel, text) in spec["files"].as_object().into_iter().flatten() {
            let path = dir.join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text.as_str().unwrap()).unwrap();
        }
        for rel in spec["directories"].as_array().into_iter().flatten() {
            fs::create_dir_all(dir.join(rel.as_str().unwrap())).unwrap();
        }
    }
}

fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            copy(&entry.path(), &to.join(entry.file_name()));
        } else {
            fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
        }
    }
}

fn file_url(path: &Path) -> String {
    let path = path.to_string_lossy().replace('\\', "/");
    if path.starts_with('/') {
        format!("file://{path}")
    } else {
        format!("file:///{path}")
    }
}

/// What a case's call returned, or the code of the error it failed with.
fn run(suite: &Suite, case: &Value) -> Value {
    let open = &case["open"];
    let reference = open["ref"].as_str();
    let opened = if let Some(name) = open["fixture"].as_str() {
        let path = suite.root.join(name);
        match reference {
            Some(r) => Repository::open_ref(&path, r),
            None => Repository::open(&path),
        }
    } else {
        let (url, allow) = match open["remote"].as_str() {
            Some(name) => (
                file_url(&suite.root.join(name)),
                open["allow_file_url"].as_bool().unwrap_or(true),
            ),
            None => (open["url"].as_str().unwrap().to_string(), false),
        };
        let options = RemoteOptions {
            allow_file_url: allow,
        };
        Repository::open_remote_with(&url, reference.unwrap(), &options)
    };
    let repo = match opened {
        Ok(repo) => repo,
        Err(e) => return json!({ "error": e.code }),
    };
    let args = case["args"].as_array().unwrap();
    let arg = |i: usize| args[i].as_str().unwrap();
    fn ok<T: Serialize>(value: T) -> Value {
        json!({ "result": value })
    }
    fn query<T: Serialize>(result: rqtk_client::Result<T>) -> Value {
        match result {
            Ok(value) => ok(value),
            Err(e) => json!({ "error": e.code }),
        }
    }
    match case["call"].as_str().unwrap() {
        "info" => ok(repo.info()),
        "requirements" => ok(repo.requirements()),
        "requirement" => ok(repo.requirement(arg(0))),
        "find_requirements" => {
            let filter: Filter = serde_json::from_value(args[0].clone()).unwrap();
            ok(repo.find_requirements(&filter))
        }
        "needs" => ok(repo.needs()),
        "need" => ok(repo.need(arg(0))),
        "stakeholders" => ok(repo.stakeholders()),
        "stakeholder" => ok(repo.stakeholder(arg(0))),
        "activities" => ok(repo.activities()),
        "context" => query(repo.context(arg(0))),
        "trace" => query(repo.trace(arg(0))),
        "coverage" => ok(repo.coverage()),
        "lint" => ok(repo.lint()),
        "impact" => query(repo.impact(arg(0))),
        "diff" => query(repo.diff(arg(0), arg(1))),
        "baselines" => query(repo.baselines()),
        "history" => query(repo.history(arg(0))),
        other => panic!("unknown call {other}"),
    }
}

/// The placeholders an expected file uses for where this run put things, as JSON string
/// contents: the open fixture's path and URL.
fn placeholders(suite: &Suite, case: &Value) -> Vec<(&'static str, String)> {
    let escaped = |s: &str| {
        let quoted = serde_json::to_string(s).unwrap();
        quoted[1..quoted.len() - 1].to_string()
    };
    let open = &case["open"];
    let name = open["fixture"].as_str().or(open["remote"].as_str());
    let mut pairs = Vec::new();
    if let Some(name) = name {
        let path = suite.root.join(name);
        pairs.push(("$URL", escaped(&file_url(&path))));
        pairs.push(("$ROOT", escaped(&path.to_string_lossy())));
    }
    pairs
}

fn expected_path(case: &Value) -> PathBuf {
    Path::new(REPO).join(format!(
        "conformance/expected/{}.json",
        case["name"].as_str().unwrap()
    ))
}

/// Runs the cases `select` picks; with `RQTK_BLESS` set, writes their results instead.
fn check(select: impl Fn(&Value) -> bool) {
    let suite = suite();
    let bless = std::env::var_os("RQTK_BLESS").is_some();
    let mut failures = Vec::new();
    let mut ran = 0;
    for case in suite.cases.iter().filter(|c| select(c)) {
        ran += 1;
        let actual = run(suite, case);
        let path = expected_path(case);
        let pairs = placeholders(suite, case);
        if bless {
            let mut text = serde_json::to_string_pretty(&actual).unwrap();
            for (name, value) in &pairs {
                text = text.replace(value.as_str(), name);
            }
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, text + "\n").unwrap();
            continue;
        }
        let Ok(mut text) = fs::read_to_string(&path) else {
            failures.push(format!("{}: no expected file; bless it", case["name"]));
            continue;
        };
        for (name, value) in &pairs {
            text = text.replace(name, value);
        }
        let expected: Value = serde_json::from_str(&text).unwrap();
        if actual != expected {
            failures.push(format!(
                "{}: expected {expected:#}\n  got {actual:#}",
                case["name"]
            ));
        }
    }
    assert!(ran > 0, "no cases selected");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn is_error(case: &Value) -> bool {
    case["name"].as_str().unwrap().starts_with("error-")
}

fn is_filter(case: &Value) -> bool {
    case["call"] == "find_requirements"
}

#[verifies("VA-CLIENT-0017-01")]
#[verifies("VA-SYS-0007-01")]
#[test]
fn every_view_matches_the_suite() {
    check(|c| !is_error(c) && !is_filter(c));
}

#[verifies("VA-CLIENT-0011-02")]
#[test]
fn every_filter_matches_the_suite() {
    check(is_filter);
}

#[verifies("VA-CLIENT-0009-01")]
#[test]
fn every_error_code_matches_the_suite() {
    let suite = suite();
    check(is_error);
    // Each code in the fixed list is triggered by some case.
    let triggered: BTreeSet<String> = suite
        .cases
        .iter()
        .filter(|c| is_error(c))
        .map(|c| run(suite, c)["error"].as_str().unwrap().to_string())
        .collect();
    let schema: Value = serde_json::from_str(
        &fs::read_to_string(Path::new(REPO).join("schema/client/client-error.schema.json"))
            .unwrap(),
    )
    .unwrap();
    let codes: BTreeSet<String> = schema["$defs"]["ErrorCode"]["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["const"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(triggered, codes);
}

/// Every expected file belongs to a case, and every fixture is used. A removed case's file
/// has to be deleted by hand.
#[test]
fn the_suite_has_no_stale_files() {
    let suite = suite();
    let names: BTreeSet<String> = suite
        .cases
        .iter()
        .map(|c| format!("{}.json", c["name"].as_str().unwrap()))
        .collect();
    assert_eq!(names.len(), suite.cases.len(), "case names repeat");
    if std::env::var_os("RQTK_BLESS").is_some() {
        return; // The blessing tests are still writing; check on the next run.
    }
    let files: BTreeSet<String> = fs::read_dir(Path::new(REPO).join("conformance/expected"))
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(files, names);
    let used: BTreeSet<&str> = suite
        .cases
        .iter()
        .filter_map(|c| {
            c["open"]["fixture"]
                .as_str()
                .or(c["open"]["remote"].as_str())
        })
        .collect();
    let defined: BTreeSet<&str> = suite
        .fixtures
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .chain(["missing"])
        .collect();
    assert_eq!(used, defined);
}
