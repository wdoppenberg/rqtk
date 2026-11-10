//! The Rust client against small repositories built in temporary directories.

use rqtk_client::{
    ClosureStatus, Direction, ErrorCode, Filter, LinkKind, Origin, RemoteOptions, Repository,
};
use rqtk_macros::verifies;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

/// Tests that set `RQTK_CACHE_DIR` must not run at the same time.
static CACHE_ENV: Mutex<()> = Mutex::new(());

const CONFIG: &str = r#"schema_version = 1

[project]
name = "demo"
version = "0.1.0"

[identification]
prefix = "DEMO"
zero_padding = 4

[categories.SYS]
name = "System"
level = 1
is_root = true

[categories.SW]
name = "Software"
level = 2

[types]
allowed = ["Functional", "Interface"]

[verification]
methods = ["Test", "Inspection"]
levels = ["System"]
phases = ["Development"]

[lifecycle]
states = ["Draft", "Approved"]
default_state = "Draft"

[priority]
levels = ["High", "Medium"]

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

fn requirement(id: &str, category: &str, statement: &str, extra: &str) -> String {
    let number = id.rsplit('-').next().unwrap();
    format!(
        r#"id = "{id}"
title = "Title of {id}"
category = "{category}"
type = "Functional"
state = "Approved"
priority = "High"
statement = "{statement}"
{extra}
[verification]
method = "Test"
level = "System"
phase = "Development"
success_criteria = "It works."

[[verification.activities]]
id = "VA-{category}-{number}-01"
name = "It works"
"#
    )
}

struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

impl Fixture {
    /// A repository with a stakeholder, a need, a system requirement and a software child.
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("demo");
        let f = Self { _dir: dir, root };
        f.write(".rqtk/config.toml", CONFIG)
            .write(
                ".rqtk/stakeholders/STK-001.toml",
                "id = \"STK-001\"\nname = \"Operator\"\n",
            )
            .write(
                ".rqtk/needs/NEED-0001.toml",
                "id = \"NEED-0001\"\ntitle = \"Work\"\nstate = \"Approved\"\nstatement = \"Operators need it to work.\"\nstakeholders = [\"STK-001\"]\n",
            )
            .write(
                ".rqtk/requirements/SYS/DEMO-SYS-0001.toml",
                &requirement(
                    "DEMO-SYS-0001",
                    "SYS",
                    "The system shall work.",
                    "rationale = \"Telemetry matters.\"\n[trace]\nsatisfies = [\"NEED-0001\"]\n",
                ),
            )
            .write(
                ".rqtk/requirements/SW/DEMO-SW-0002.toml",
                &requirement(
                    "DEMO-SW-0002",
                    "SW",
                    "The software shall send telemetry.",
                    "[trace]\nparents = [\"DEMO-SYS-0001\"]\n",
                ),
            )
            .write(
                "tests/app.rs",
                "#[verifies(\"VA-SW-0002-01\")]\n#[test]\nfn sends_telemetry() {}\n",
            );
        git(&f.root, &["init", "-q"]);
        f.commit("v1");
        git(&f.root, &["tag", "v1"]);
        f
    }

    fn write(&self, rel: &str, text: &str) -> &Self {
        let path = self.root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
        self
    }

    fn commit(&self, message: &str) -> &Self {
        git(&self.root, &["add", "-A"]);
        git(
            &self.root,
            &["commit", "-q", "--allow-empty", "-m", message],
        );
        self
    }

    fn url(&self) -> String {
        format!("file://{}", self.root.display())
    }
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
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
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

fn ids<T>(items: &[T], id: impl Fn(&T) -> &str) -> Vec<String> {
    items.iter().map(|i| id(i).to_owned()).collect()
}

#[verifies("VA-CLIENT-0002-01")]
#[test]
fn opens_a_working_tree() {
    let f = Fixture::new();
    let repo = Repository::open(&f.root).unwrap();
    assert_eq!(
        ids(&repo.requirements(), |r| &r.id),
        ["DEMO-SW-0002", "DEMO-SYS-0001"]
    );
    assert_eq!(ids(&repo.needs(), |n| &n.id), ["NEED-0001"]);
    assert_eq!(ids(&repo.stakeholders(), |s| &s.id), ["STK-001"]);
    let info = repo.info();
    assert_eq!(info.project, "demo");
    assert_eq!(
        info.origin,
        Origin::Checkout {
            path: f.root.clone()
        }
    );
    let sw = repo.requirement("DEMO-SW-0002").unwrap();
    assert_eq!(
        sw.path,
        Path::new(".rqtk/requirements/SW/DEMO-SW-0002.toml")
    );
    assert_eq!(sw.status, ClosureStatus::Planned);
}

#[verifies("VA-CLIENT-0003-01")]
#[test]
fn opens_a_tag_a_baseline_and_a_commit_without_touching_the_working_tree() {
    let f = Fixture::new();
    let v1 = git(&f.root, &["rev-parse", "HEAD"]);
    git(&f.root, &["tag", "rqtk/1.0.0"]);
    f.write(
        ".rqtk/requirements/SYS/DEMO-SYS-0003.toml",
        &requirement("DEMO-SYS-0003", "SYS", "The system shall log.", ""),
    )
    .commit("v2");
    let before = fs::read_dir(f.root.join(".rqtk/requirements/SYS"))
        .unwrap()
        .count();

    for reference in ["v1", "1.0.0", v1.as_str()] {
        let repo = Repository::open_ref(&f.root, reference).unwrap();
        assert_eq!(
            ids(&repo.requirements(), |r| &r.id),
            ["DEMO-SW-0002", "DEMO-SYS-0001"],
            "{reference}"
        );
        assert_eq!(repo.info().commit.as_deref(), Some(v1.as_str()));
        assert_eq!(
            repo.requirement("DEMO-SW-0002").unwrap().path,
            Path::new(".rqtk/requirements/SW/DEMO-SW-0002.toml"),
            "paths are relative, not into the extracted copy"
        );
    }
    let head = Repository::open_ref(&f.root, "HEAD").unwrap();
    assert_eq!(head.requirements().len(), 3);
    assert_eq!(
        fs::read_dir(f.root.join(".rqtk/requirements/SYS"))
            .unwrap()
            .count(),
        before
    );
    assert_eq!(git(&f.root, &["status", "--porcelain"]), "");

    let err = Repository::open_ref(&f.root, "no-such-ref").unwrap_err();
    assert_eq!(err.code, ErrorCode::RefNotFound);
}

#[verifies("VA-CLIENT-0004-01")]
#[test]
fn opens_a_remote_url_at_a_ref() {
    let _env = CACHE_ENV.lock().unwrap_or_else(|e| e.into_inner());
    let cache = tempfile::tempdir().unwrap();
    // SAFETY: tests touching RQTK_CACHE_DIR hold CACHE_ENV.
    unsafe { std::env::set_var("RQTK_CACHE_DIR", cache.path()) };
    let f = Fixture::new();
    let options = RemoteOptions {
        allow_file_url: true,
    };
    let remote = Repository::open_remote_with(&f.url(), "v1", &options).unwrap();
    let local = Repository::open_ref(&f.root, "v1").unwrap();
    assert_eq!(
        serde_json::to_value(remote.coverage()).unwrap(),
        serde_json::to_value(local.coverage()).unwrap()
    );
    assert_eq!(
        remote.info().origin,
        Origin::Remote {
            url: f.url(),
            reference: "v1".into()
        }
    );
    // A new commit upstream reaches a reload.
    f.write(
        ".rqtk/requirements/SYS/DEMO-SYS-0003.toml",
        &requirement("DEMO-SYS-0003", "SYS", "The system shall log.", ""),
    )
    .commit("v2");
    let main = Repository::open_remote_with(&f.url(), "main", &options).unwrap();
    assert_eq!(main.requirements().len(), 3);
    unsafe { std::env::remove_var("RQTK_CACHE_DIR") };
}

#[verifies("VA-CLIENT-0004-02")]
#[test]
fn disallowed_transports_are_refused() {
    let f = Fixture::new();
    for url in [
        f.url(),
        f.root.display().to_string(),
        "http://example.invalid/repo".into(),
        "git://example.invalid/repo".into(),
        "ext::sh -c touch% /tmp/pwned".into(),
    ] {
        let err = Repository::open_remote(&url, "v1").unwrap_err();
        assert_eq!(err.code, ErrorCode::RemoteRefused, "{url}");
    }
}

#[verifies("VA-CLIENT-0005-01")]
#[verifies("VA-CLIENT-0005-02")]
#[test]
fn the_cache_lives_where_configured_and_only_clear_cache_removes_it() {
    let _env = CACHE_ENV.lock().unwrap_or_else(|e| e.into_inner());
    let cache = tempfile::tempdir().unwrap();
    let dir = cache.path().join("repos");
    unsafe { std::env::set_var("RQTK_CACHE_DIR", &dir) };
    assert_eq!(rqtk_client::cache_dir(), dir);
    let f = Fixture::new();
    let options = RemoteOptions {
        allow_file_url: true,
    };
    let repo = Repository::open_remote_with(&f.url(), "v1", &options).unwrap();
    let cached: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert_eq!(cached.len(), 1, "one bare clone per URL: {cached:?}");
    assert!(cached[0].join("HEAD").is_file());
    // Opening again, reloading and dropping keep the clone.
    let again = Repository::open_remote_with(&f.url(), "v1", &options).unwrap();
    let reloaded = again.reload().unwrap();
    drop((repo, again, reloaded));
    assert!(cached[0].join("HEAD").is_file());
    rqtk_client::clear_cache().unwrap();
    assert!(!dir.exists());
    unsafe { std::env::remove_var("RQTK_CACHE_DIR") };

    // Without the override, the platform's cache directory.
    let default = rqtk_client::cache_dir();
    let expected = if cfg!(windows) {
        Path::new("rqtk").join("cache").join("repos")
    } else if cfg!(target_os = "macos") {
        PathBuf::from("Library/Caches/rqtk/repos")
    } else {
        PathBuf::from("rqtk/repos")
    };
    assert!(default.ends_with(&expected), "{}", default.display());
}

#[verifies("VA-CLIENT-0007-01")]
#[test]
fn queries_ignore_later_edits_until_reload() {
    let f = Fixture::new();
    let repo = Repository::open(&f.root).unwrap();
    let before = serde_json::to_value(repo.coverage()).unwrap();
    let lint_before = serde_json::to_value(repo.lint()).unwrap();
    f.write(
        ".rqtk/requirements/SYS/DEMO-SYS-0003.toml",
        &requirement("DEMO-SYS-0003", "SYS", "The system shall log.", ""),
    )
    .write(
        ".rqtk/requirements/SYS/DEMO-SYS-0001.toml",
        "this is not toml",
    )
    .write("tests/app.rs", "// the test is gone\n");
    assert_eq!(repo.requirements().len(), 2);
    assert_eq!(serde_json::to_value(repo.coverage()).unwrap(), before);
    assert_eq!(serde_json::to_value(repo.lint()).unwrap(), lint_before);
    assert!(repo.context("DEMO-SYS-0003").is_err());
    let impact = serde_json::to_value(repo.impact("HEAD").unwrap()).unwrap();
    assert_eq!(impact["requirements"], serde_json::json!([]), "{impact}");
    assert_eq!(impact["reverify"], serde_json::json!([]), "{impact}");

    let fresh = repo.reload().unwrap();
    assert!(fresh.requirement("DEMO-SYS-0003").is_some());
    assert_ne!(serde_json::to_value(fresh.lint()).unwrap(), lint_before);
}

#[verifies("VA-CLIENT-0008-01")]
#[test]
fn only_an_unreadable_repository_fails_to_open() {
    let f = Fixture::new();
    let empty = tempfile::tempdir().unwrap();
    assert_eq!(
        Repository::open(empty.path()).unwrap_err().code,
        ErrorCode::NotARepository
    );
    f.write(".rqtk/config.toml", "schema_version = [");
    assert_eq!(
        Repository::open(&f.root).unwrap_err().code,
        ErrorCode::InvalidConfig
    );
    f.write(
        ".rqtk/config.toml",
        &CONFIG.replace("schema_version = 1", "schema_version = 99"),
    );
    assert_eq!(
        Repository::open(&f.root).unwrap_err().code,
        ErrorCode::UnsupportedSchema
    );
    f.write(".rqtk/config.toml", CONFIG);
    assert_eq!(
        Repository::open_ref(&f.root, "v99").unwrap_err().code,
        ErrorCode::RefNotFound
    );
}

#[verifies("VA-CLIENT-0008-02")]
#[test]
fn other_problems_become_lint_findings() {
    let f = Fixture::new();
    f.write(".rqtk/requirements/SYS/DEMO-SYS-0009.toml", "id = [broken");
    let repo = Repository::open(&f.root).unwrap();
    let lint = repo.lint();
    assert_eq!(lint.errors, 1, "{:?}", lint.diagnostics);
    assert_eq!(lint.diagnostics[0].code, "RQ100");
    assert_eq!(
        lint.diagnostics[0].location.as_ref().unwrap().path,
        Path::new(".rqtk/requirements/SYS/DEMO-SYS-0009.toml")
    );
    assert_eq!(repo.requirements().len(), 2, "the rest still loads");
}

#[verifies("VA-CLIENT-0010-01")]
#[test]
fn links_appear_from_both_ends() {
    let f = Fixture::new();
    let repo = Repository::open(&f.root).unwrap();
    let links = |id: &str| {
        let mut out: Vec<(LinkKind, Direction, String)> = repo
            .requirement(id)
            .map(|r| r.links)
            .or_else(|| repo.need(id).map(|n| n.links))
            .or_else(|| repo.stakeholder(id).map(|s| s.links))
            .unwrap()
            .into_iter()
            .map(|l| (l.kind, l.direction, l.id))
            .collect();
        out.sort();
        out
    };
    use Direction::{Incoming, Outgoing};
    assert_eq!(
        links("DEMO-SYS-0001"),
        [
            (LinkKind::Parents, Incoming, "DEMO-SW-0002".into()),
            (LinkKind::Satisfies, Outgoing, "NEED-0001".into()),
            (LinkKind::Activities, Outgoing, "VA-SYS-0001-01".into()),
        ]
    );
    assert_eq!(
        links("DEMO-SW-0002"),
        [
            (LinkKind::Parents, Outgoing, "DEMO-SYS-0001".into()),
            (LinkKind::Activities, Outgoing, "VA-SW-0002-01".into()),
        ]
    );
    assert_eq!(
        links("NEED-0001"),
        [
            (LinkKind::Satisfies, Incoming, "DEMO-SYS-0001".into()),
            (LinkKind::Stakeholders, Outgoing, "STK-001".into()),
        ]
    );
    assert_eq!(
        links("STK-001"),
        [(LinkKind::Stakeholders, Incoming, "NEED-0001".into())]
    );
    let activities = repo.activities();
    assert_eq!(
        ids(&activities, |a| &a.id),
        ["VA-SW-0002-01", "VA-SYS-0001-01"]
    );
    assert_eq!(
        activities[0].tests[0].test_name.as_deref(),
        Some("sends_telemetry")
    );
    assert!(repo.need("NEED-0001").unwrap().satisfied);
}

#[verifies("VA-CLIENT-0011-01")]
#[test]
fn each_filter_field_selects_its_requirements() {
    let f = Fixture::new();
    f.write(
        ".rqtk/requirements/SYS/DEMO-SYS-0003.toml",
        &requirement(
            "DEMO-SYS-0003",
            "SYS",
            "The system shall use the user service.",
            "[trace]\nconsumes = [\"users:USR-SYS-0001\"]\n",
        )
        .replace("state = \"Approved\"", "state = \"Draft\"")
        .replace("priority = \"High\"", "priority = \"Medium\""),
    );
    let repo = Repository::open(&f.root).unwrap();
    let find = |filter: Filter| ids(&repo.find_requirements(&filter), |r| &r.id);
    assert_eq!(find(Filter::default()).len(), 3);
    assert_eq!(
        find(Filter {
            category: vec!["SW".into()],
            ..Filter::default()
        }),
        ["DEMO-SW-0002"]
    );
    assert_eq!(
        find(Filter {
            req_type: vec!["Interface".into()],
            ..Filter::default()
        }),
        Vec::<String>::new()
    );
    assert_eq!(
        find(Filter {
            state: vec!["Draft".into()],
            ..Filter::default()
        }),
        ["DEMO-SYS-0003"]
    );
    assert_eq!(
        find(Filter {
            priority: vec!["Medium".into()],
            ..Filter::default()
        }),
        ["DEMO-SYS-0003"]
    );
    assert_eq!(
        find(Filter {
            ancestor: Some("DEMO-SYS-0001".into()),
            ..Filter::default()
        }),
        ["DEMO-SW-0002"]
    );
    for consumes in ["users:USR-SYS-0001", "users"] {
        assert_eq!(
            find(Filter {
                consumes: Some(consumes.into()),
                ..Filter::default()
            }),
            ["DEMO-SYS-0003"],
            "{consumes}"
        );
    }
    assert_eq!(
        find(Filter {
            text: Some("TELEMETRY".into()),
            ..Filter::default()
        }),
        ["DEMO-SW-0002", "DEMO-SYS-0001"],
        "statement or rationale, case-insensitive"
    );
    let statuses: Vec<ClosureStatus> = repo.requirements().iter().map(|r| r.status).collect();
    assert_eq!(
        find(Filter {
            status: vec![statuses[0]],
            req_type: vec!["Functional".into()],
            ..Filter::default()
        })
        .len(),
        statuses.iter().filter(|s| **s == statuses[0]).count()
    );
    // A filter is data: it round-trips through JSON, with `type` as the field name.
    let filter: Filter =
        serde_json::from_str(r#"{"type": ["Functional"], "category": ["SW"]}"#).unwrap();
    assert_eq!(find(filter), ["DEMO-SW-0002"]);
    assert!(serde_json::from_str::<Filter>(r#"{"colour": "red"}"#).is_err());
}

#[verifies("VA-CLIENT-0021-01")]
#[test]
fn one_repository_answers_queries_from_many_threads() {
    let f = Fixture::new();
    f.write(
        ".rqtk/requirements/SW/DEMO-SW-0002.toml",
        &requirement(
            "DEMO-SW-0002",
            "SW",
            "The software shall always send telemetry.",
            "[trace]\nparents = [\"DEMO-SYS-0001\"]\n",
        ),
    )
    .commit("v2");
    let repo = Repository::open(&f.root).unwrap();
    // Every query, including those that read git objects (impact, diff, history).
    let answers = |repo: &Repository| {
        serde_json::json!([
            repo.requirements(),
            repo.activities(),
            repo.context("DEMO-SW-0002").unwrap(),
            repo.coverage(),
            repo.lint(),
            repo.impact("v1").unwrap(),
            repo.diff("v1", "HEAD").unwrap(),
            repo.history("DEMO-SW-0002").unwrap(),
            repo.baselines().unwrap(),
        ])
    };
    let expected = answers(&repo);
    let shared = &repo;
    std::thread::scope(|scope| {
        let threads: Vec<_> = (0..8)
            .map(|_| scope.spawn(move || (0..10).map(|_| answers(shared)).collect::<Vec<_>>()))
            .collect();
        for thread in threads {
            for answer in thread.join().unwrap() {
                assert_eq!(answer, expected);
            }
        }
    });
}
