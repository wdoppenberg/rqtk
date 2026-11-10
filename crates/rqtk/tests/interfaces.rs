//! `rqtk interfaces`: consuming interface requirements from other repositories, end to end.
//! Provider repositories are real git repositories, synced over `file://` URLs.

use assert_cmd::Command;
use predicates::prelude::*;
use rqtk_macros::verifies;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const WARNING: &str = "Vendored interface content comes from the provider's repository";

fn rqtk(root: &Path) -> Command {
    let mut cmd = Command::cargo_bin("rqtk").unwrap();
    cmd.arg("--repo-root").arg(root);
    cmd
}

fn json(cmd: &mut Command) -> Value {
    let out = cmd.arg("--json").output().unwrap();
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "{e}: {}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
    })
}

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
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

fn config(name: &str, prefix: &str, types: &str) -> String {
    format!(
        r#"schema_version = 1

[project]
name = "{name}"
version = "0.1.0"

[identification]
prefix = "{prefix}"
zero_padding = 4

[categories.SYS]
name = "System"
level = 1
is_root = true

[types]
allowed = [{types}]

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
"#
    )
}

/// A requirement file; `extra` is appended verbatim (e.g. `[interface]` or `[trace]`).
fn requirement(id: &str, req_type: &str, statement: &str, extra: &str) -> String {
    let number = id.rsplit('-').next().unwrap();
    format!(
        r#"id = "{id}"
title = "Title of {id}"
category = "SYS"
type = "{req_type}"
state = "Draft"
priority = "High"
statement = "{statement}"

[verification]
method = "Test"
level = "System"
phase = "Development"
success_criteria = "It works."

[[verification.activities]]
id = "VA-SYS-{number}-01"
name = "It works"
{extra}"#
    )
}

const API: &str = r##"openapi: 3.0.3
info: {title: Users, version: "1"}
paths:
  /users/{id}:
    get:
      responses:
        "200":
          description: A user
          content:
            application/json:
              schema: {$ref: "#/components/schemas/User"}
  /health:
    get:
      responses:
        "200": {description: ok}
components:
  schemas:
    User:
      type: object
      properties:
        id: {type: string}
"##;

const GET_USER: &str = r##"
[interface]
[[interface.specs]]
format = "openapi"
path = "api/openapi.yaml"
element = "#/paths/~1users~1{id}/get"
"##;

struct Repo {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

impl Repo {
    fn new(name: &str, prefix: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join(name);
        fs::create_dir_all(root.join(".rqtk/requirements/SYS")).unwrap();
        fs::write(
            root.join(".rqtk/config.toml"),
            config(name, prefix, r#""Functional", "Interface""#),
        )
        .unwrap();
        git(&root, &["init", "-q"]);
        Self { _dir: dir, root }
    }

    fn write(&self, rel: &str, text: &str) -> &Self {
        let path = self.root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
        self
    }

    fn req(&self, id: &str, req_type: &str, statement: &str, extra: &str) -> &Self {
        self.write(
            &format!(".rqtk/requirements/SYS/{id}.toml"),
            &requirement(id, req_type, statement, extra),
        )
    }

    fn commit(&self, message: &str) -> &Self {
        git(&self.root, &["add", "-A"]);
        git(
            &self.root,
            &["commit", "-q", "--allow-empty", "-m", message],
        );
        self
    }

    fn tag(&self, tag: &str) -> &Self {
        git(&self.root, &["tag", "-f", tag]);
        self
    }

    fn url(&self) -> String {
        format!("file://{}", self.root.display())
    }

    fn read(&self, rel: &str) -> String {
        fs::read_to_string(self.root.join(rel)).unwrap()
    }

    /// Every file below `.rqtk/vendor` and the lockfile, with contents.
    fn vendor_state(&self) -> BTreeMap<String, Vec<u8>> {
        fn walk(dir: &Path, root: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
            let Ok(entries) = fs::read_dir(dir) else {
                return;
            };
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, root, out);
                } else {
                    out.insert(
                        p.strip_prefix(root).unwrap().display().to_string(),
                        fs::read(&p).unwrap(),
                    );
                }
            }
        }
        let mut out = BTreeMap::new();
        walk(&self.root.join(".rqtk/vendor"), &self.root, &mut out);
        if let Ok(lock) = fs::read(self.root.join(".rqtk/interfaces.lock")) {
            out.insert("lock".to_owned(), lock);
        }
        out
    }
}

/// A provider with one OpenAPI interface requirement (`USR-SYS-0001`), tagged `v1`.
fn provider() -> Repo {
    let p = Repo::new("users", "USR");
    p.write("api/openapi.yaml", API)
        .req(
            "USR-SYS-0001",
            "Interface",
            "The user service shall expose GET /users/{id} returning a User.",
            GET_USER,
        )
        .commit("v1")
        .tag("v1");
    p
}

/// A consumer whose `REQ-SYS-0001` consumes `users:USR-SYS-0001`, declared but not synced.
fn consumer(provider: &Repo, reference: &str) -> Repo {
    let c = Repo::new("app", "REQ");
    c.req(
        "REQ-SYS-0001",
        "Functional",
        "The app shall show a user's profile.",
        "\n[trace]\nconsumes = [\"users:USR-SYS-0001\"]\n",
    );
    rqtk(&c.root)
        .args([
            "interfaces",
            "add",
            "users",
            &provider.url(),
            "--ref",
            reference,
        ])
        .arg("--allow-file-url")
        .assert()
        .success();
    c
}

fn sync(c: &Repo) -> assert_cmd::assert::Assert {
    rqtk(&c.root)
        .args(["interfaces", "sync", "--allow-file-url"])
        .assert()
}

fn lint_codes(root: &Path) -> Vec<String> {
    let report = json(rqtk(root).arg("lint"));
    let mut codes: Vec<String> = report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["code"].as_str().unwrap().to_owned())
        .collect();
    codes.sort();
    codes
}

fn status_of(root: &Path, id: &str) -> Value {
    let report = json(rqtk(root).arg("coverage"));
    report["requirements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == id)
        .cloned()
        .unwrap()
}

/// A linked Rust test for `activity` named `name`, and a JUnit file where it passed.
fn passing_test(repo: &Repo, activity: &str, name: &str) -> PathBuf {
    repo.write(
        "tests/app.rs",
        &format!(
            "#[{}(\"{activity}\")]\n#[test]\nfn {name}() {{}}\n",
            "verifies"
        ),
    );
    let junit = repo.root.join("junit.xml");
    fs::write(
        &junit,
        format!(
            "<?xml version=\"1.0\"?><testsuites><testsuite name=\"t\"><testcase classname=\"app\" name=\"{name}\"/></testsuite></testsuites>"
        ),
    )
    .unwrap();
    junit
}

// ── end to end ──────────────────────────────────────────────────────────────

// The end-to-end journey is in `e2e_interfaces.rs`.

#[verifies("VA-CLI-0025-01")]
#[test]
fn syncing_a_repository_into_itself_warns() {
    let p = provider();
    rqtk(&p.root)
        .args([
            "interfaces",
            "add",
            "me",
            &p.url(),
            "--ref",
            "v1",
            "--allow-file-url",
        ])
        .assert()
        .success();
    rqtk(&p.root)
        .args(["interfaces", "sync", "--allow-file-url"])
        .assert()
        .success()
        .stderr(predicate::str::contains("this repository or a fork of it"));
    let report = json(rqtk(&p.root).args(["interfaces", "sync", "--allow-file-url"]));
    let warnings = report["sources"][0]["warnings"].as_array().unwrap();
    assert_eq!(warnings.len(), 1, "{report}");
    assert!(warnings[0].as_str().unwrap().contains("`me`"));
    // A separate provider is not flagged.
    let other = provider();
    let c = consumer(&other, "v1");
    let report = json(rqtk(&c.root).args(["interfaces", "sync", "--allow-file-url"]));
    assert_eq!(report["sources"][0]["warnings"], serde_json::json!([]));
}

// ── provider side ───────────────────────────────────────────────────────────

#[verifies("VA-CORE-0007-01")]
#[test]
fn interfaces_are_marked_by_the_table_not_the_type() {
    let p = Repo::new("users", "USR");
    p.write("api/openapi.yaml", API)
        .req(
            "USR-SYS-0001",
            "Functional",
            "The service shall expose GET /users/{id}.",
            GET_USER,
        )
        .req(
            "USR-SYS-0002",
            "Interface",
            "The service shall answer health checks.",
            "",
        )
        .commit("v1")
        .tag("v1");
    let c = consumer(&p, "v1");
    sync(&c).success();
    let lock = c.read(".rqtk/interfaces.lock");
    assert!(lock.contains("USR-SYS-0001"), "{lock}");
    assert!(!lock.contains("USR-SYS-0002"), "{lock}");
}

#[verifies("VA-CORE-0008-01")]
#[test]
fn editing_a_spec_file_makes_the_content_hash_stale() {
    let p = Repo::new("svc", "SVC");
    p.write("proto/a.proto", "message A {}\n")
        .write("README.md", "hello\n")
        .req(
            "SVC-SYS-0001",
            "Interface",
            "The service shall accept A messages.",
            "\n[interface]\n[[interface.specs]]\nformat = \"protobuf\"\npath = \"proto/a.proto\"\n",
        );
    rqtk(&p.root).args(["rehash", "--all"]).assert().success();
    assert!(lint_codes(&p.root).is_empty());
    p.write("README.md", "changed\n");
    assert!(
        lint_codes(&p.root).is_empty(),
        "an unreferenced file is not the contract"
    );
    p.write("proto/a.proto", "message A { string id = 1; }\n");
    assert_eq!(lint_codes(&p.root), ["RQ021"]);
}

#[verifies("VA-CORE-0008-03")]
#[test]
fn impact_and_diff_see_a_change_to_spec_files_alone() {
    let p = Repo::new("svc", "SVC");
    p.write("proto/a.proto", "message A {}\n")
        .req(
            "SVC-SYS-0001",
            "Interface",
            "The service shall accept A messages.",
            "\n[interface]\n[[interface.specs]]\nformat = \"protobuf\"\npath = \"proto/*.proto\"\n",
        )
        .commit("v1");
    p.write("proto/a.proto", "message A { string id = 1; }\n");

    let impact = json(rqtk(&p.root).args(["impact", "HEAD"]));
    let changed = impact["requirements"].as_array().unwrap();
    assert_eq!(changed.len(), 1, "{impact}");
    assert_eq!(changed[0]["id"], "SVC-SYS-0001");
    assert_eq!(changed[0]["change"], "semantic");

    p.commit("v2");
    let diff = json(rqtk(&p.root).args(["diff", "HEAD~1", "HEAD"]));
    let modified = diff["modified"].as_array().unwrap();
    assert_eq!(modified.len(), 1, "{diff}");
    assert_eq!(modified[0]["id"], "SVC-SYS-0001");
}

#[verifies("VA-CORE-0019-01")]
#[verifies("VA-CORE-0009-01")]
#[verifies("VA-CORE-0026-01")]
#[test]
fn interface_spec_problems_are_lint_errors() {
    let p = Repo::new("svc", "SVC");
    p.write("api/openapi.yaml", API)
        .req(
            "SVC-SYS-0001",
            "Interface",
            "The service shall be empty.",
            "\n[interface]\n",
        )
        .req(
            "SVC-SYS-0002",
            "Interface",
            "The service shall exist.",
            "\n[interface]\n[[interface.specs]]\nformat = \"protobuf\"\npath = \"nope.proto\"\n",
        )
        .req(
            "SVC-SYS-0003",
            "Interface",
            "The service shall expose POST /users.",
            &GET_USER.replace("~1users~1{id}/get", "~1users/post"),
        );
    let report = json(rqtk(&p.root).arg("lint"));
    let found: Vec<(String, String)> = report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            (
                d["code"].as_str().unwrap().to_owned(),
                d["subject"]["id"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    for expected in [
        ("RQ034", "SVC-SYS-0001"),
        ("RQ032", "SVC-SYS-0002"),
        ("RQ037", "SVC-SYS-0003"),
    ] {
        assert!(
            found.contains(&(expected.0.to_owned(), expected.1.to_owned())),
            "{expected:?} not in {found:?}"
        );
    }
    let located = report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .all(|d| d["location"]["line"].is_number());
    assert!(located, "{report}");
}

// ── sync ────────────────────────────────────────────────────────────────────

#[verifies("VA-CLI-0011-01")]
#[verifies("VA-CLI-0011-02")]
#[verifies("VA-CORE-0022-01")]
#[test]
fn sync_vendors_requirements_specs_and_status() {
    let p = provider();
    p.write("proto/v1/user.proto", "message User {}\n")
        .write("proto/v1/notes.md", "not a spec\n")
        .write("unrelated.txt", "not vendored\n")
        .req(
            "USR-SYS-0002",
            "Interface",
            "The service shall publish User messages.",
            "\n[interface]\n[[interface.specs]]\nformat = \"protobuf\"\npath = \"proto/**/*.proto\"\n",
        )
        .commit("v1")
        .tag("v1");
    let c = consumer(&p, "v1");
    sync(&c).success();

    let files: Vec<String> = c.vendor_state().into_keys().collect();
    assert_eq!(
        files,
        [
            ".rqtk/vendor/users/files/api/openapi.yaml",
            ".rqtk/vendor/users/files/proto/v1/user.proto",
            ".rqtk/vendor/users/requirements/USR-SYS-0001.toml",
            ".rqtk/vendor/users/requirements/USR-SYS-0002.toml",
            ".rqtk/vendor/users/status.toml",
            "lock",
        ]
    );
    let lock: toml::Table = toml::from_str(&c.read(".rqtk/interfaces.lock")).unwrap();
    let users = &lock["sources"]["users"];
    assert_eq!(
        users["commit"].as_str().unwrap(),
        git(&p.root, &["rev-parse", "v1"])
    );
    assert_eq!(users["ref_kind"].as_str(), Some("tag"));
    assert_eq!(users["files"].as_table().unwrap().len(), 5);
    assert!(
        users["requirements"]["USR-SYS-0001"]
            .as_str()
            .unwrap()
            .starts_with("v1:")
    );
}

#[verifies("VA-CLI-0024-01")]
#[test]
fn status_toml_matches_provider_coverage() {
    let p = provider();
    let junit = passing_test(&p, "VA-SYS-0001-01", "get_user_works");
    rqtk(&p.root)
        .args(["verify", "--results"])
        .arg(&junit)
        .assert()
        .success();
    p.commit("verified").tag("v1");
    let provider_status = status_of(&p.root, "USR-SYS-0001")["status"].clone();
    assert_eq!(provider_status, "verified");

    let c = consumer(&p, "v1");
    sync(&c).success();
    let status: toml::Table = toml::from_str(&c.read(".rqtk/vendor/users/status.toml")).unwrap();
    assert_eq!(
        status["requirements"]["USR-SYS-0001"].as_str(),
        provider_status.as_str()
    );
}

#[verifies("VA-CLI-0012-01")]
#[test]
fn the_providers_own_vendored_interfaces_are_ignored() {
    // `orders` consumes `users`, and is itself consumed by `web`.
    let users = provider();
    let orders = Repo::new("orders", "ORD");
    orders.write("api/openapi.yaml", API).req(
        "ORD-SYS-0001",
        "Interface",
        "The order service shall expose GET /users/{id} with the buyer.",
        &format!("\n[trace]\nconsumes = [\"users:USR-SYS-0001\"]\n{GET_USER}"),
    );
    rqtk(&orders.root)
        .args(["interfaces", "add", "users", &users.url(), "--ref", "v1"])
        .arg("--allow-file-url")
        .assert()
        .success();
    sync(&orders).success();
    assert!(orders.root.join(".rqtk/vendor/users").is_dir());
    orders.commit("v1").tag("v1");

    let web = Repo::new("web", "WEB");
    web.req(
        "WEB-SYS-0001",
        "Functional",
        "The web app shall show the buyer of an order.",
        "\n[trace]\nconsumes = [\"orders:ORD-SYS-0001\"]\n",
    );
    rqtk(&web.root)
        .args(["interfaces", "add", "orders", &orders.url(), "--ref", "v1"])
        .arg("--allow-file-url")
        .assert()
        .success();
    sync(&web).success();
    let files: Vec<String> = web.vendor_state().into_keys().collect();
    assert!(
        files
            .iter()
            .all(|f| f == "lock" || f.starts_with(".rqtk/vendor/orders/")),
        "{files:?}"
    );
    assert!(!files.iter().any(|f| f.contains("USR-SYS")), "{files:?}");
    let lock = web.read(".rqtk/interfaces.lock");
    assert!(!lock.contains("[sources.users]"), "{lock}");
    assert!(
        lint_codes(&web.root).is_empty(),
        "{:?}",
        lint_codes(&web.root)
    );
}

#[verifies("VA-CORE-0017-01")]
#[test]
fn lint_and_coverage_need_no_network() {
    let p = provider();
    let c = consumer(&p, "v1");
    sync(&c).success();
    drop(p);
    for command in ["lint", "coverage"] {
        rqtk(&c.root)
            .arg(command)
            .env("HTTPS_PROXY", "http://127.0.0.1:9")
            .env("HTTP_PROXY", "http://127.0.0.1:9")
            .env("ALL_PROXY", "http://127.0.0.1:9")
            .env("GIT_SSH_COMMAND", "false")
            .assert()
            .success();
    }
}

#[verifies("VA-CLI-0011-03")]
#[test]
fn a_baseline_name_resolves_to_its_rqtk_tag() {
    let p = provider();
    git(&p.root, &["config", "user.name", "Test"]);
    git(&p.root, &["config", "user.email", "test@example.com"]);
    rqtk(&p.root).args(["baseline", "1.0.0"]).assert().success();
    let c = consumer(&p, "1.0.0");
    sync(&c).success();
    let lock = c.read(".rqtk/interfaces.lock");
    assert!(lock.contains("ref = \"1.0.0\""), "{lock}");
    assert!(lock.contains("ref_kind = \"tag\""), "{lock}");
}

#[verifies("VA-CORE-0016-02")]
#[test]
fn an_unreadable_vendored_requirement_is_an_error() {
    let p = provider();
    let c = consumer(&p, "v1");
    sync(&c).success();
    c.write(
        ".rqtk/vendor/users/requirements/USR-SYS-0001.toml",
        "id = \"USR-SYS-0001\"\nunknown_field = 1\n",
    );
    let codes = lint_codes(&c.root);
    assert!(codes.contains(&"RQ043".to_owned()), "{codes:?}");
}

#[verifies("VA-CLI-0013-01")]
#[test]
fn a_newer_provider_schema_is_refused() {
    let p = provider();
    let config = p
        .read(".rqtk/config.toml")
        .replace("schema_version = 1", "schema_version = 2");
    p.write(".rqtk/config.toml", &config).commit("v2").tag("v2");
    let c = consumer(&p, "v2");
    sync(&c)
        .code(3)
        .stderr(predicate::str::contains("`users`"))
        .stderr(predicate::str::contains("schema_version 2"))
        .stderr(predicate::str::contains("schema_version 1"));
    assert!(c.vendor_state().is_empty());
}

#[verifies("VA-CLI-0008-01")]
#[test]
fn an_escaping_spec_path_aborts_sync() {
    let p = Repo::new("users", "USR");
    p.req(
        "USR-SYS-0001",
        "Interface",
        "The service shall leak.",
        "\n[interface]\n[[interface.specs]]\nformat = \"other\"\npath = \"../../../etc/passwd\"\n",
    )
    .commit("v1")
    .tag("v1");
    let c = consumer(&p, "v1");
    sync(&c)
        .failure()
        .stderr(predicate::str::contains("outside the repository"));
    assert!(c.vendor_state().is_empty());
}

#[cfg(unix)]
#[verifies("VA-CORE-0018-01")]
#[test]
fn symlinks_in_provider_content_abort_sync() {
    let p = provider();
    fs::remove_file(p.root.join("api/openapi.yaml")).unwrap();
    std::os::unix::fs::symlink("/etc/hosts", p.root.join("api/openapi.yaml")).unwrap();
    p.commit("link").tag("v1");
    let c = consumer(&p, "v1");
    sync(&c)
        .failure()
        .stderr(predicate::str::contains("symbolic link"));
    assert!(c.vendor_state().is_empty());
}

#[cfg(unix)]
#[verifies("VA-CORE-0018-01")]
#[test]
fn a_symlinked_requirement_file_aborts_sync() {
    let p = provider();
    let real = p.root.join("USR-SYS-0001.toml");
    fs::rename(
        p.root.join(".rqtk/requirements/SYS/USR-SYS-0001.toml"),
        &real,
    )
    .unwrap();
    std::os::unix::fs::symlink(
        &real,
        p.root.join(".rqtk/requirements/SYS/USR-SYS-0001.toml"),
    )
    .unwrap();
    p.commit("link").tag("v1");
    let c = consumer(&p, "v1");
    sync(&c)
        .failure()
        .stderr(predicate::str::contains("symbolic link"));
    assert!(c.vendor_state().is_empty());
}

#[cfg(unix)]
#[verifies("VA-CORE-0018-01")]
#[verifies("VA-CLI-0026-01")]
#[test]
fn a_ref_through_a_symlinked_directory_aborts_sync() {
    let p = provider();
    p.write(
        "shared/user.yaml",
        "type: object\nproperties:\n  id: {type: string}\n",
    );
    std::os::unix::fs::symlink("../shared", p.root.join("api/schemas")).unwrap();
    p.write(
        "api/openapi.yaml",
        &API.replace("#/components/schemas/User", "schemas/user.yaml"),
    )
    .commit("link")
    .tag("v1");
    // The provider itself resolves the $ref through the link on disk.
    assert!(lint_codes(&p.root).is_empty(), "{:?}", lint_codes(&p.root));
    let c = consumer(&p, "v1");
    sync(&c)
        .failure()
        .stderr(predicate::str::contains("symbolic link `api/schemas`"));
    assert!(c.vendor_state().is_empty());
}

#[verifies("VA-CLI-0027-01")]
#[test]
fn vendored_control_characters_are_not_printed() {
    let p = provider();
    p.req(
        "USR-SYS-0001",
        "Interface",
        "The user service shall \\u001b[2Jexpose GET /users/{id}.",
        GET_USER,
    )
    .commit("escape")
    .tag("v1");
    let c = consumer(&p, "v1");
    sync(&c).success();
    let out = rqtk(&c.root)
        .args(["context", "users:USR-SYS-0001"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("[2Jexpose GET"), "{stdout}");
    assert!(!stdout.contains('\u{1b}'), "{stdout:?}");
}

#[cfg(unix)]
#[verifies("VA-CLI-0027-01")]
#[test]
fn a_provider_file_name_with_a_control_character_aborts_sync() {
    let p = provider();
    p.write("proto/a\u{1b}[31m.proto", "message A {}\n")
        .commit("escape")
        .tag("v1");
    let c = consumer(&p, "v1");
    let out = rqtk(&c.root)
        .args(["interfaces", "sync", "--allow-file-url"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.contains("invalid file name `a\\u{1b}[31m.proto`"),
        "{stderr}"
    );
    assert!(!stderr.contains('\u{1b}'), "{stderr:?}");
    assert!(c.vendor_state().is_empty());
}

#[verifies("VA-CLI-0017-02")]
#[verifies("VA-CLI-0014-02")]
#[test]
fn an_unreadable_lockfile_stops_remove_and_list() {
    let p = provider();
    let c = consumer(&p, "v1");
    sync(&c).success();
    c.write(".rqtk/interfaces.lock", "version = [not a lockfile");
    let before = (c.read(".rqtk/config.toml"), c.vendor_state());
    rqtk(&c.root)
        .args(["interfaces", "remove", "users"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "cannot read .rqtk/interfaces.lock",
        ));
    assert_eq!((c.read(".rqtk/config.toml"), c.vendor_state()), before);
    rqtk(&c.root)
        .args(["interfaces", "list"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "cannot read .rqtk/interfaces.lock",
        ));
}

#[verifies("VA-CLI-0026-01")]
#[test]
fn an_unresolved_provider_spec_aborts_sync() {
    let p = provider();
    p.req(
        "USR-SYS-0001",
        "Interface",
        "The user service shall expose GET /users/{id} returning a User.",
        &GET_USER.replace("{id}/get", "{id}/post"),
    )
    .commit("broken")
    .tag("v1");
    let c = consumer(&p, "v1");
    sync(&c).failure().stderr(predicate::str::contains("RQ037"));
    assert!(c.vendor_state().is_empty());
}

#[verifies("VA-CLI-0019-01")]
#[test]
fn a_failed_sync_leaves_vendor_and_lockfile_unchanged() {
    let good = provider();
    let bad = provider();
    let c = consumer(&good, "v1");
    rqtk(&c.root)
        .args([
            "interfaces",
            "add",
            "other",
            &bad.url(),
            "--ref",
            "v1",
            "--allow-file-url",
        ])
        .assert()
        .success();
    sync(&c).success();
    let before = c.vendor_state();

    good.write("api/openapi.yaml", &API.replace("A user", "Changed"))
        .commit("v2")
        .tag("v1");
    bad.write(".rqtk/config.toml", "not = [valid toml")
        .commit("broken")
        .tag("v1");
    sync(&c).failure();
    assert_eq!(c.vendor_state(), before);
}

#[verifies("VA-CLI-0020-01")]
#[test]
fn an_oversized_file_aborts_sync() {
    let p = provider();
    let big = format!("{}\n# {}\n", API, "x".repeat(11 * 1024 * 1024));
    p.write("api/openapi.yaml", &big).commit("big").tag("v1");
    let c = consumer(&p, "v1");
    sync(&c)
        .failure()
        .stderr(predicate::str::contains("`users`"))
        .stderr(predicate::str::contains("10 MiB"));
    assert!(c.vendor_state().is_empty());
}

#[verifies("VA-CLI-0020-02")]
#[test]
fn an_oversized_alias_aborts_sync() {
    let p = Repo::new("users", "USR");
    let chunk = "x".repeat(9 * 1024 * 1024);
    for i in 0..12 {
        p.write(&format!("blobs/{i:02}.bin"), &format!("{i}{chunk}"));
    }
    p.req(
        "USR-SYS-0001",
        "Interface",
        "The service shall be large.",
        "\n[interface]\n[[interface.specs]]\nformat = \"other\"\npath = \"blobs/*.bin\"\n",
    )
    .commit("big")
    .tag("v1");
    let c = consumer(&p, "v1");
    sync(&c)
        .failure()
        .stderr(predicate::str::contains("`users`"))
        .stderr(predicate::str::contains("100 MiB"));
    assert!(c.vendor_state().is_empty());
}

#[verifies("VA-CLI-0022-01")]
#[test]
fn syncing_one_alias_leaves_the_others_unchanged() {
    let a = provider();
    let b = provider();
    let c = consumer(&a, "main");
    rqtk(&c.root)
        .args([
            "interfaces",
            "add",
            "other",
            &b.url(),
            "--ref",
            "main",
            "--allow-file-url",
        ])
        .assert()
        .success();
    sync(&c).success();
    a.write("api/openapi.yaml", &API.replace("A user", "Changed a"))
        .commit("a2");
    b.write("api/openapi.yaml", &API.replace("A user", "Changed b"))
        .commit("b2");
    let before = c.vendor_state();
    rqtk(&c.root)
        .args(["interfaces", "sync", "users", "--allow-file-url"])
        .assert()
        .success();
    let after = c.vendor_state();
    let other = |s: &BTreeMap<String, Vec<u8>>| {
        s.iter()
            .filter(|(k, _)| k.contains("/other/"))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(other(&after), other(&before));
    let users_spec = ".rqtk/vendor/users/files/api/openapi.yaml";
    assert_ne!(after[users_spec], before[users_spec]);
    let lock: toml::Table = toml::from_str(&c.read(".rqtk/interfaces.lock")).unwrap();
    let old: toml::Table = toml::from_str(std::str::from_utf8(&before["lock"]).unwrap()).unwrap();
    assert_eq!(lock["sources"]["other"], old["sources"]["other"]);
    assert_ne!(lock["sources"]["users"], old["sources"]["users"]);
}

// ── consumer side ───────────────────────────────────────────────────────────

#[verifies("VA-CORE-0011-01")]
#[test]
fn an_unknown_alias_is_a_lint_error() {
    let p = provider();
    let c = consumer(&p, "v1");
    sync(&c).success();
    c.req(
        "REQ-SYS-0002",
        "Functional",
        "The app shall list users.",
        "\n[trace]\nconsumes = [\"nobody:USR-SYS-0001\"]\n",
    );
    assert_eq!(lint_codes(&c.root), ["RQ038"]);
}

#[verifies("VA-CORE-0011-02")]
#[test]
fn a_requirement_removed_upstream_is_a_lint_error() {
    let p = provider();
    let c = consumer(&p, "main");
    sync(&c).success();
    fs::remove_file(p.root.join(".rqtk/requirements/SYS/USR-SYS-0001.toml")).unwrap();
    p.commit("drop interface");
    sync(&c).success();
    assert_eq!(lint_codes(&c.root), ["RQ039", "RQ042"]);
}

#[verifies("VA-CORE-0012-01")]
#[verifies("VA-CORE-0012-02")]
#[test]
fn an_upstream_change_makes_consumers_suspect_until_reviewed() {
    let p = provider();
    let c = consumer(&p, "main");
    // A child of the consumer builds on the same interface.
    c.req(
        "REQ-SYS-0002",
        "Functional",
        "The app shall show a user's avatar on the profile.",
        "\n[trace]\nparents = [\"REQ-SYS-0001\"]\n",
    )
    .write(
        "tests/app.rs",
        "#[verifies(\"VA-SYS-0001-01\")]\n#[test]\nfn shows_profile() {}\n\
         #[verifies(\"VA-SYS-0002-01\")]\n#[test]\nfn shows_avatar() {}\n",
    );
    sync(&c).success();
    let junit = c.root.join("junit.xml");
    fs::write(
        &junit,
        "<?xml version=\"1.0\"?><testsuites><testsuite name=\"t\">\
         <testcase classname=\"app\" name=\"shows_profile\"/>\
         <testcase classname=\"app\" name=\"shows_avatar\"/></testsuite></testsuites>",
    )
    .unwrap();
    rqtk(&c.root)
        .args(["verify", "--results"])
        .arg(&junit)
        .assert()
        .success();
    assert_eq!(status_of(&c.root, "REQ-SYS-0001")["status"], "verified");
    assert_eq!(status_of(&c.root, "REQ-SYS-0002")["status"], "verified");

    // An unrelated operation changes: the consumed element did not.
    p.write(
        "api/openapi.yaml",
        &API.replace("{description: ok}", "{description: fine}"),
    )
    .commit("health");
    sync(&c).success();
    assert_eq!(status_of(&c.root, "REQ-SYS-0001")["status"], "verified");

    // The schema the consumed operation returns changes.
    p.write(
        "api/openapi.yaml",
        &API.replace("id: {type: string}", "id: {type: integer}"),
    )
    .commit("schema");
    sync(&c).success();
    let status = status_of(&c.root, "REQ-SYS-0001");
    assert_eq!(status["status"], "suspect");
    assert_eq!(status["suspect_reasons"][0]["reason"], "upstream_changed");
    assert_eq!(
        status["suspect_reasons"][0]["items"][0],
        "users:USR-SYS-0001"
    );

    let child = status_of(&c.root, "REQ-SYS-0002");
    assert_eq!(
        child["status"], "suspect",
        "a descendant of the consumer too"
    );
    assert!(
        child["suspect_reasons"][0]["items"]
            .as_array()
            .unwrap()
            .contains(&Value::from("users:USR-SYS-0001")),
        "{child}"
    );

    for id in ["REQ-SYS-0001", "REQ-SYS-0002"] {
        rqtk(&c.root)
            .args(["review", id, "--note", "still fits"])
            .assert()
            .success();
        assert_eq!(status_of(&c.root, id)["status"], "verified");
    }
}

#[verifies("VA-CORE-0013-01")]
#[verifies("VA-CORE-0013-02")]
#[verifies("VA-CORE-0013-03")]
#[test]
fn edited_added_or_deleted_vendored_files_are_lint_errors() {
    let p = provider();
    let c = consumer(&p, "v1");
    sync(&c).success();
    let spec = c.root.join(".rqtk/vendor/users/files/api/openapi.yaml");
    let original = fs::read_to_string(&spec).unwrap();

    fs::write(&spec, original.replace("A user", "Edited")).unwrap();
    assert!(lint_codes(&c.root).contains(&"RQ040".to_owned()));
    fs::write(&spec, &original).unwrap();
    assert!(lint_codes(&c.root).is_empty());

    c.write(".rqtk/vendor/users/files/extra.yaml", "x: 1\n");
    assert_eq!(lint_codes(&c.root), ["RQ040"]);
    fs::remove_file(c.root.join(".rqtk/vendor/users/files/extra.yaml")).unwrap();

    fs::remove_file(c.root.join(".rqtk/vendor/users/status.toml")).unwrap();
    assert_eq!(lint_codes(&c.root), ["RQ040"]);
}

#[test]
fn crlf_checkouts_of_vendored_files_pass_the_integrity_check() {
    let p = provider();
    let c = consumer(&p, "v1");
    sync(&c).success();
    let spec = c.root.join(".rqtk/vendor/users/files/api/openapi.yaml");
    let crlf = fs::read_to_string(&spec).unwrap().replace('\n', "\r\n");
    fs::write(&spec, crlf).unwrap();
    assert!(lint_codes(&c.root).is_empty(), "{:?}", lint_codes(&c.root));
}

#[verifies("VA-CORE-0014-01")]
#[test]
fn config_and_lockfile_drift_is_a_lint_error() {
    let p = provider();
    let c = consumer(&p, "v1");
    assert_eq!(lint_codes(&c.root), ["RQ041"], "declared but not synced");
    sync(&c).success();
    assert!(lint_codes(&c.root).is_empty());
    p.tag("v2");
    let config = c
        .read(".rqtk/config.toml")
        .replace("ref = \"v1\"", "ref = \"v2\"");
    c.write(".rqtk/config.toml", &config);
    assert_eq!(lint_codes(&c.root), ["RQ041"], "ref changed");
    let config = config
        .replace("ref = \"v2\"", "ref = \"v1\"")
        .replace("file://", "file:///../");
    c.write(".rqtk/config.toml", &config);
    assert_eq!(lint_codes(&c.root), ["RQ041"], "URL changed");
}

#[verifies("VA-CORE-0014-02")]
#[test]
fn orphaned_vendor_content_is_a_lint_error() {
    let p = provider();
    let c = consumer(&p, "v1");
    sync(&c).success();
    let config = c.read(".rqtk/config.toml");
    let declared = config.find("[interfaces.users]").unwrap();
    c.write(".rqtk/config.toml", &config[..declared]);
    let codes = lint_codes(&c.root);
    assert!(codes.contains(&"RQ041".to_owned()), "{codes:?}");
}

#[verifies("VA-CORE-0015-01")]
#[test]
fn pinning_a_branch_warns() {
    let p = provider();
    let c = consumer(&p, "main");
    sync(&c).success();
    rqtk(&c.root).arg("lint").assert().success();
    assert_eq!(lint_codes(&c.root), ["RQ042"]);
    let config = c
        .read(".rqtk/config.toml")
        .replace("ref = \"main\"", "ref = \"v1\"");
    c.write(".rqtk/config.toml", &config);
    sync(&c).success();
    assert!(lint_codes(&c.root).is_empty());
    let sha = git(&p.root, &["rev-parse", "v1"]);
    let config = c
        .read(".rqtk/config.toml")
        .replace("ref = \"v1\"", &format!("ref = \"{sha}\""));
    c.write(".rqtk/config.toml", &config);
    sync(&c).success();
    assert!(lint_codes(&c.root).is_empty());
    assert!(
        c.read(".rqtk/interfaces.lock")
            .contains("ref_kind = \"commit\"")
    );
}

#[verifies("VA-CORE-0016-01")]
#[test]
fn consumer_policy_does_not_apply_to_vendored_requirements() {
    let p = Repo::new("users", "USR");
    let provider_config = config("users", "USR", r#""Endpoint""#);
    p.write(".rqtk/config.toml", &provider_config)
        .write("api/openapi.yaml", API)
        .req(
            "USR-SYS-0001",
            "Endpoint",
            "The service shall expose GET /users/{id}.",
            GET_USER,
        )
        .commit("v1")
        .tag("v1");
    let c = consumer(&p, "v1");
    sync(&c).success();
    // The consumer allows neither type `Endpoint` nor the `USR` prefix.
    assert!(lint_codes(&c.root).is_empty(), "{:?}", lint_codes(&c.root));
}

#[verifies("VA-CORE-0020-01")]
#[test]
fn other_commands_leave_the_vendor_directory_unchanged() {
    let p = provider();
    let c = consumer(&p, "v1");
    sync(&c).success();
    git(&c.root, &["add", "-A"]);
    git(&c.root, &["commit", "-q", "-m", "synced"]);
    git(&c.root, &["config", "user.name", "Test"]);
    git(&c.root, &["config", "user.email", "test@example.com"]);
    let before = c.vendor_state();
    rqtk(&c.root).args(["rehash", "--all"]).assert().success();
    rqtk(&c.root)
        .args(["review", "REQ-SYS-0001"])
        .assert()
        .success();
    rqtk(&c.root)
        .args(["add-activity", "REQ-SYS-0001", "--name", "More"])
        .assert()
        .success();
    rqtk(&c.root).args(["baseline", "1.0.0"]).assert().success();
    assert_eq!(c.vendor_state(), before);
}

#[verifies("VA-CLI-0016-01")]
#[test]
fn mutating_commands_refuse_vendored_ids() {
    let p = provider();
    let c = consumer(&p, "v1");
    sync(&c).success();
    for args in [
        vec!["review", "users:USR-SYS-0001"],
        vec!["open", "users:USR-SYS-0001"],
        vec!["add-activity", "users:USR-SYS-0001", "--name", "x"],
    ] {
        rqtk(&c.root)
            .env("EDITOR", "false")
            .args(&args)
            .assert()
            .code(2)
            .stderr(predicate::str::contains("read-only"))
            .stderr(predicate::str::contains("`users`"));
    }
}

// ── views ───────────────────────────────────────────────────────────────────

#[verifies("VA-CLI-0015-01")]
#[test]
fn views_render_alias_qualified_nodes() {
    let p = provider();
    let c = consumer(&p, "v1");
    sync(&c).success();
    rqtk(&c.root)
        .args(["context", "REQ-SYS-0001"])
        .assert()
        .success()
        .stdout(predicate::str::contains("users:USR-SYS-0001"));
    let vendored = json(rqtk(&c.root).args(["context", "users:USR-SYS-0001"]));
    assert_eq!(vendored["kind"], "interface");
    assert_eq!(vendored["consumers"][0]["id"], "REQ-SYS-0001");
    assert_eq!(vendored["provider_status"], "planned");
    let trace = json(rqtk(&c.root).args(["trace", "REQ-SYS-0001"]));
    assert_eq!(trace["consumes"][0]["id"], "users:USR-SYS-0001");
    rqtk(&c.root)
        .arg("graph")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "\"REQ-SYS-0001\" -> \"users:USR-SYS-0001\"",
        ));
    rqtk(&c.root)
        .arg("report")
        .assert()
        .success()
        .stdout(predicate::str::contains("## Consumed interfaces"))
        .stdout(predicate::str::contains("`users:USR-SYS-0001`"));
    let coverage = json(rqtk(&c.root).arg("coverage"));
    assert_eq!(coverage["consumed"][0]["id"], "users:USR-SYS-0001");
    assert_eq!(coverage["consumed"][0]["provider_status"], "planned");
}

#[verifies("VA-CLI-0015-02")]
#[test]
fn provider_status_does_not_gate_coverage() {
    let p = provider();
    let c = consumer(&p, "v1");
    sync(&c).success();
    let junit = passing_test(&c, "VA-SYS-0001-01", "shows_profile");
    rqtk(&c.root)
        .args(["verify", "--results"])
        .arg(&junit)
        .assert()
        .success();
    // The provider's requirement is only Planned; the consumer's own is Verified.
    rqtk(&c.root)
        .args(["coverage", "--strict"])
        .assert()
        .success();
    let coverage = json(rqtk(&c.root).arg("coverage"));
    assert_eq!(coverage["summary"]["verified"], 1);
    assert!(coverage["summary"].get("planned").is_none(), "{coverage}");
}

#[verifies("VA-CLI-0021-01")]
#[test]
fn vendored_text_is_escaped_in_the_report() {
    let p = Repo::new("users", "USR");
    p.write("api/openapi.yaml", API)
        .req(
            "USR-SYS-0001",
            "Interface",
            "The service shall <script>alert(1)</script> [click](https://evil.invalid) **x**.",
            GET_USER,
        )
        .commit("v1")
        .tag("v1");
    let c = consumer(&p, "v1");
    sync(&c).success();
    let out = rqtk(&c.root).arg("report").output().unwrap();
    let report = String::from_utf8(out.stdout).unwrap();
    let section = &report[report.find("## Consumed interfaces").unwrap()..];
    let row = section
        .lines()
        .find(|l| l.contains("users:USR-SYS-0001"))
        .unwrap();
    assert!(!row.contains("<script>"), "{row}");
    assert!(row.contains("&lt;script&gt;"), "{row}");
    assert!(!row.contains("[click](https"), "{row}");
    assert!(!row.contains("**x**"), "{row}");
}

#[verifies("VA-CLI-0023-01")]
#[test]
fn impact_lists_consumers_of_changed_interfaces() {
    let p = provider();
    let c = consumer(&p, "main");
    c.req(
        "REQ-SYS-0002",
        "Functional",
        "The app shall show a dashboard.",
        "",
    )
    .req(
        "REQ-SYS-0003",
        "Functional",
        "The app shall show a user's avatar on the profile.",
        "\n[trace]\nparents = [\"REQ-SYS-0001\"]\n",
    );
    sync(&c).success();
    git(&c.root, &["add", "-A"]);
    git(&c.root, &["commit", "-q", "-m", "synced"]);
    p.write("api/openapi.yaml", &API.replace("A user", "The user"))
        .commit("v2");
    sync(&c).success();
    let impact = json(rqtk(&c.root).args(["impact", "HEAD"]));
    assert_eq!(impact["interfaces"][0]["id"], "users:USR-SYS-0001");
    assert_eq!(impact["interfaces"][0]["change"], "semantic");
    let downstream: Vec<(&str, &str)> = impact["downstream"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| (d["id"].as_str().unwrap(), d["via"][0].as_str().unwrap()))
        .collect();
    assert_eq!(
        downstream,
        [
            ("REQ-SYS-0001", "users:USR-SYS-0001"),
            ("REQ-SYS-0003", "users:USR-SYS-0001"),
        ],
        "the consumer and its descendant, not the unrelated REQ-SYS-0002: {impact}"
    );
}

// ── add / remove / list ─────────────────────────────────────────────────────

#[verifies("VA-CLI-0009-01")]
#[verifies("VA-CLI-0010-01")]
#[verifies("VA-CLI-0010-02")]
#[test]
fn add_records_the_alias_and_warns() {
    let c = Repo::new("app", "REQ");
    let config = format!("# Project settings\n{}", c.read(".rqtk/config.toml"));
    c.write(".rqtk/config.toml", &config);
    rqtk(&c.root)
        .args([
            "interfaces",
            "add",
            "users",
            "https://example.invalid/users.git",
            "--ref",
            "v1",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains(WARNING));
    let written = c.read(".rqtk/config.toml");
    assert!(
        written.starts_with(&config),
        "existing content is preserved:\n{written}"
    );
    assert!(
        written.contains(
            "[interfaces.users]\ngit = \"https://example.invalid/users.git\"\nref = \"v1\""
        )
    );

    let out = rqtk(&c.root)
        .args([
            "interfaces",
            "add",
            "users",
            "https://example.invalid/x.git",
            "--ref",
            "v1",
        ])
        .arg("--json")
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(2),
        "a duplicate alias is a usage error"
    );
    assert!(String::from_utf8_lossy(&out.stderr).contains(WARNING));

    let report = json(rqtk(&c.root).args([
        "interfaces",
        "add",
        "orders",
        "git@example.invalid:orders.git",
        "--ref",
        "v1",
        "--dry-run",
    ]));
    assert!(report["warning"].as_str().unwrap().starts_with(WARNING));
    assert_eq!(report["written"], false);
    assert!(
        !c.read(".rqtk/config.toml").contains("orders"),
        "a dry run writes nothing"
    );

    rqtk(&c.root)
        .args(["interfaces", "add", "evil", "ext::sh -c id", "--ref", "v1"])
        .assert()
        .code(2);
}

#[verifies("VA-CLI-0009-02")]
#[test]
fn sync_warns_about_untrusted_content() {
    let p = provider();
    let c = consumer(&p, "v1");
    sync(&c).stderr(predicate::str::contains(WARNING));
    let out = rqtk(&c.root)
        .args(["interfaces", "sync", "--allow-file-url", "--json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains(WARNING));
    let report: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(report["warning"].as_str().unwrap().starts_with(WARNING));
}

#[verifies("VA-CLI-0017-01")]
#[test]
fn remove_deletes_config_lockfile_entry_and_vendor_directory() {
    let p = provider();
    let c = consumer(&p, "v1");
    sync(&c).success();
    rqtk(&c.root)
        .args(["interfaces", "remove", "users"])
        .assert()
        .success();
    assert!(!c.read(".rqtk/config.toml").contains("interfaces"));
    assert!(!c.root.join(".rqtk/interfaces.lock").exists());
    assert!(!c.root.join(".rqtk/vendor/users").exists());
    assert_eq!(
        lint_codes(&c.root),
        ["RQ038"],
        "the consumes link that still names it"
    );
    rqtk(&c.root)
        .args(["interfaces", "remove", "users"])
        .assert()
        .code(2);
}

#[verifies("VA-CLI-0014-01")]
#[test]
fn list_shows_every_alias() {
    let p = provider();
    let c = consumer(&p, "v1");
    rqtk(&c.root)
        .args([
            "interfaces",
            "add",
            "later",
            "https://example.invalid/later.git",
            "--ref",
            "v3",
        ])
        .assert()
        .success();
    rqtk(&c.root)
        .args(["interfaces", "sync", "users", "--allow-file-url"])
        .assert()
        .success();
    let listed = json(rqtk(&c.root).args(["interfaces", "list"]));
    assert_eq!(listed["schema"], "rqtk/interfaces-list/2");
    let listed = listed["sources"].as_array().unwrap();
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0]["alias"], "later");
    assert_eq!(listed[0]["commit"], Value::Null);
    assert_eq!(listed[1]["alias"], "users");
    assert_eq!(listed[1]["git"], p.url());
    assert_eq!(listed[1]["ref"], "v1");
    assert_eq!(
        listed[1]["commit"].as_str().unwrap(),
        git(&p.root, &["rev-parse", "v1"])
    );
    assert_eq!(listed[1]["requirements"], 1);
    rqtk(&c.root)
        .args(["interfaces", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("users"))
        .stdout(predicate::str::contains("not synced"));
}
