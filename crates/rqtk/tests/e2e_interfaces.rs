//! End to end: three systems that expose and consume each other's interfaces, through a
//! release history, as their teams would drive rqtk.
//!
//! ```text
//!   users ──GET /users/{id}──▶ orders ──GET /orders/{id}──▶ web
//!     ▲                          │
//!     └────OrderPlaced webhook───┘
//! ```
//!
//! The repositories start from `tests/fixtures/interfaces/`, become git repositories in a
//! temporary directory, publish with `rqtk baseline` and consume each other over `file://`.
//! Test results are written as JUnit, as a CI run would produce them.

use assert_cmd::Command;
use rqtk_macros::verifies;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

const WARNING: &str = "Vendored interface content comes from the provider's repository";

/// One system's repository.
struct System {
    name: &'static str,
    root: PathBuf,
}

impl System {
    fn new(base: &Path, name: &'static str) -> Self {
        let root = base.join(name);
        copy_dir(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/interfaces")
                .join(name),
            &root,
        );
        let system = Self { name, root };
        system.git(&["init", "-q", "-b", "main"]);
        system.git(&["config", "user.name", "Test"]);
        system.git(&["config", "user.email", "test@example.com"]);
        system.git(&["config", "commit.gpgsign", "false"]);
        system.git(&["config", "tag.gpgsign", "false"]);
        system.commit("initial");
        system
    }

    fn cmd(&self) -> Command {
        let mut cmd = Command::cargo_bin("rqtk").unwrap();
        cmd.arg("--repo-root").arg(&self.root);
        cmd
    }

    /// Run `rqtk <args> --json`, expecting `code`; returns stdout as JSON and stderr.
    fn rqtk(&self, args: &[&str], code: i32) -> (Value, String) {
        let out = self.cmd().args(args).arg("--json").output().unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        assert_eq!(
            out.status.code(),
            Some(code),
            "{}: rqtk {}\nstdout: {}\nstderr: {stderr}",
            self.name,
            args.join(" "),
            String::from_utf8_lossy(&out.stdout),
        );
        let json = serde_json::from_slice(&out.stdout).unwrap_or(Value::Null);
        (json, stderr)
    }

    fn git(&self, args: &[&str]) -> String {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(&self.root)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}: git {args:?}: {}",
            self.name,
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    }

    fn commit(&self, message: &str) {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "--allow-empty", "-m", message]);
    }

    fn url(&self) -> String {
        format!("file://{}", self.root.display())
    }

    fn write(&self, rel: &str, text: &str) {
        let path = self.root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn read(&self, rel: &str) -> String {
        fs::read_to_string(self.root.join(rel)).unwrap()
    }

    /// Replace `from` (which must occur exactly once) with `to` in file `rel`.
    fn edit(&self, rel: &str, from: &str, to: &str) {
        let text = self.read(rel);
        assert_eq!(
            text.matches(from).count(),
            1,
            "{}: `{from}` in {rel}",
            self.name
        );
        self.write(rel, &text.replacen(from, to, 1));
    }

    /// Record a CI run in which `tests` passed.
    fn tests_pass(&self, tests: &[&str]) {
        let cases: String = tests
            .iter()
            .map(|t| format!("<testcase classname=\"{}\" name=\"{t}\"/>", self.name))
            .collect();
        let junit = self.root.join("target/junit.xml");
        fs::create_dir_all(junit.parent().unwrap()).unwrap();
        fs::write(
            &junit,
            format!("<?xml version=\"1.0\"?><testsuites><testsuite name=\"{}\">{cases}</testsuite></testsuites>", self.name),
        )
        .unwrap();
        self.rqtk(&["verify", "--results", junit.to_str().unwrap()], 0);
    }

    /// Commit everything and publish it as `rqtk/<version>`.
    fn publish(&self, version: &str) {
        self.commit(&format!("release {version}"));
        self.rqtk(&["baseline", version], 0);
    }

    fn consume(&self, alias: &str, provider: &System, reference: &str) -> String {
        let url = provider.url();
        let (_, stderr) = self.rqtk(
            &[
                "interfaces",
                "add",
                alias,
                &url,
                "--ref",
                reference,
                "--allow-file-url",
            ],
            0,
        );
        stderr
    }

    /// Point `alias` at another ref, as a team bumping a dependency would.
    fn pin(&self, alias: &str, reference: &str) {
        let config = self.read(".rqtk/config.toml");
        let table = config.find(&format!("[interfaces.{alias}]")).unwrap();
        let at = table + config[table..].find("ref = \"").unwrap();
        let end = at + config[at..].find('\n').unwrap();
        let mut updated = config.clone();
        updated.replace_range(at..end, &format!("ref = \"{reference}\""));
        self.write(".rqtk/config.toml", &updated);
    }

    fn sync(&self) -> Value {
        self.rqtk(&["interfaces", "sync", "--allow-file-url"], 0).0
    }

    fn lint_codes(&self) -> Vec<String> {
        let out = self.cmd().args(["lint", "--json"]).output().unwrap();
        let report: Value = serde_json::from_slice(&out.stdout).unwrap();
        let mut codes: Vec<String> = report["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d["code"].as_str().unwrap().to_owned())
            .collect();
        codes.sort();
        codes
    }

    /// Requirement `id`'s coverage entry.
    fn status(&self, id: &str) -> Value {
        let (coverage, _) = self.rqtk(&["coverage"], 0);
        coverage["requirements"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == id)
            .unwrap_or_else(|| panic!("{}: no requirement {id}", self.name))
            .clone()
    }

    /// Done, by the project's own definition: lint and `coverage --strict` pass.
    fn assert_green(&self) {
        let codes = self.lint_codes();
        assert!(codes.is_empty(), "{}: lint findings {codes:?}", self.name);
        self.rqtk(&["coverage", "--strict"], 0);
    }

    fn assert_suspect_because(&self, id: &str, upstream: &str) {
        let status = self.status(id);
        assert_eq!(status["status"], "suspect", "{}: {status}", self.name);
        let reasons = status["suspect_reasons"].as_array().unwrap();
        assert!(
            reasons.iter().any(|r| r["reason"] == "upstream_changed"
                && r["items"].as_array().unwrap().iter().any(|i| i == upstream)),
            "{}: {id} not suspect because of {upstream}: {status}",
            self.name
        );
    }

    fn vendored_files(&self) -> Vec<String> {
        let mut out = Vec::new();
        collect_files(&self.root.join(".rqtk/vendor"), &self.root, &mut out);
        out.sort();
        out
    }
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap().flatten() {
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn collect_files(dir: &Path, root: &Path, out: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if entry.path().is_dir() {
            collect_files(&entry.path(), root, out);
        } else {
            out.push(
                entry
                    .path()
                    .strip_prefix(root)
                    .unwrap()
                    .display()
                    .to_string(),
            );
        }
    }
}

/// Mark the start of a step, so a failure names it.
fn phase(name: &str) {
    eprintln!("── {name}");
}

#[verifies("VA-SYS-0006-01")]
#[verifies("VA-CORE-0010-01")]
#[verifies("VA-CORE-0030-01")]
#[verifies("VA-CORE-0030-02")]
#[test]
fn systems_expose_and_consume_interfaces_through_releases() {
    let base = tempfile::tempdir().unwrap();
    let users = System::new(base.path(), "users");
    let orders = System::new(base.path(), "orders");
    let web = System::new(base.path(), "web");

    phase("1. users publishes GET /users/{id}");
    users.tests_pass(&["get_user_returns_the_user"]);
    users.assert_green();
    users.publish("1.0.0");

    phase("2. orders consumes users and publishes its own interfaces");
    let stderr = orders.consume("users", &users, "rqtk/1.0.0");
    assert!(stderr.contains(WARNING), "add warns: {stderr}");
    let (synced, stderr) = orders.rqtk(&["interfaces", "sync", "--allow-file-url"], 0);
    assert!(stderr.contains(WARNING), "sync warns: {stderr}");
    assert_eq!(synced["sources"][0]["ref_kind"], "tag");
    assert_eq!(synced["sources"][0]["warnings"], serde_json::json!([]));
    orders.tests_pass(&[
        "get_order_includes_the_customer",
        "subscribers_receive_order_placed",
    ]);
    orders.assert_green();
    orders.publish("1.0.0");

    phase("3. web consumes orders, and only orders");
    web.consume("orders", &orders, "rqtk/1.0.0");
    web.sync();
    let vendored = web.vendored_files();
    assert!(
        vendored
            .iter()
            .all(|f| f.starts_with(".rqtk/vendor/orders/")),
        "{vendored:?}"
    );
    assert!(
        vendored.iter().all(|f| !f.contains("USR-")),
        "nothing of users reaches web: {vendored:?}"
    );
    web.tests_pass(&["order_page_shows_the_customer"]);
    web.assert_green();
    let (coverage, _) = web.rqtk(&["coverage"], 0);
    assert_eq!(coverage["consumed"][0]["id"], "orders:ORD-SYS-0001");
    assert_eq!(coverage["consumed"][0]["provider_status"], "verified");
    let (context, _) = web.rqtk(&["context", "WEB-SYS-0001"], 0);
    assert_eq!(context["consumes"][0]["id"], "orders:ORD-SYS-0001");
    assert_eq!(context["consumes"][0]["resolved"], true);
    let (context, _) = web.rqtk(&["context", "orders:ORD-SYS-0001"], 0);
    assert_eq!(context["kind"], "interface");
    assert_eq!(context["consumers"][0]["id"], "WEB-SYS-0001");
    web.commit("consume orders 1.0.0");

    phase("3b. users subscribes to OrderPlaced: users and orders now consume each other");
    users.write(
        ".rqtk/requirements/SYS/USR-SYS-0002.toml",
        r#"id = "USR-SYS-0002"
title = "Record a user's last order"
category = "SYS"
type = "Functional"
state = "Approved"
priority = "Medium"
statement = "The user service shall record a user's last order date when it receives OrderPlaced."
rationale = "Support sees recent activity on the user page."

[trace]
consumes = ["orders:ORD-SYS-0002"]

[verification]
method = "Test"
level = "System"
phase = "Development"
success_criteria = "After OrderPlaced for a user, their last order date is the event's date."

[[verification.activities]]
id = "VA-SYS-0002-01"
name = "OrderPlaced updates the last order date"
"#,
    );
    users.write(
        "tests/subscriptions.rs",
        &format!(
            "#[{}(\"VA-SYS-0002-01\")]\n#[test]\nfn order_placed_updates_last_order() {{}}\n",
            "verifies"
        ),
    );
    users.consume("orders", &orders, "rqtk/1.0.0");
    users.sync();
    users.tests_pass(&[
        "get_user_returns_the_user",
        "order_placed_updates_last_order",
    ]);
    users.assert_green();
    users.publish("1.0.1");
    orders.commit("before bump");
    orders.pin("users", "rqtk/1.0.1");
    orders.sync();
    orders.assert_green();
    for system in [&users, &orders] {
        assert!(
            !system.lint_codes().contains(&"RQ017".to_owned()),
            "a cross-repository cycle is not a trace cycle"
        );
    }

    phase("4. users ships a change that is not part of what orders consumes");
    users.edit("api/openapi.yaml", "The service is up", "Healthy");
    users.publish("1.1.0");
    orders.commit("before bump");
    orders.pin("users", "rqtk/1.1.0");
    orders.sync();
    orders.assert_green();
    let (impact, _) = orders.rqtk(&["impact", "HEAD"], 0);
    assert_eq!(impact["interfaces"], serde_json::json!([]), "{impact}");

    phase("5. someone hand-edits orders' vendored copy");
    let vendored_schema = ".rqtk/vendor/users/files/api/schemas/user.yaml";
    orders.edit(
        vendored_schema,
        "name: {type: string}",
        "name: {type: integer}",
    );
    assert_eq!(orders.lint_codes(), ["RQ040"]);
    orders.sync();
    assert!(
        orders
            .read(vendored_schema)
            .contains("name: {type: string}")
    );
    orders.assert_green();

    phase("6. users changes the User schema: one hop at a time");
    users.edit(
        "api/schemas/user.yaml",
        "required: [id, name]",
        "required: [id, name, email]",
    );
    users.edit(
        "api/schemas/user.yaml",
        "    name: {type: string}\n",
        "    name: {type: string}\n    email: {type: string}\n",
    );
    assert_eq!(
        users.status("USR-SYS-0001")["status"],
        "suspect",
        "its own contract changed"
    );
    users.tests_pass(&[
        "get_user_returns_the_user",
        "order_placed_updates_last_order",
    ]);
    // The same tests passed for the changed contract: someone confirms they still prove it.
    users.rqtk(
        &[
            "review",
            "USR-SYS-0001",
            "--note",
            "The test asserts the full User schema.",
        ],
        0,
    );
    users.assert_green();
    users.publish("2.0.0");

    orders.commit("before bump");
    orders.pin("users", "rqtk/2.0.0");
    orders.sync();
    orders.assert_suspect_because("ORD-SYS-0001", "users:USR-SYS-0001");
    assert_eq!(orders.status("ORD-SYS-0002")["status"], "verified");
    let (impact, _) = orders.rqtk(&["impact", "HEAD"], 0);
    assert_eq!(impact["interfaces"][0]["id"], "users:USR-SYS-0001");
    assert_eq!(impact["interfaces"][0]["change"], "semantic");
    assert_eq!(impact["downstream"][0]["id"], "ORD-SYS-0001");

    web.sync();
    web.assert_green();

    // orders adapts its own contract, re-runs its tests and publishes.
    orders.edit(
        "api/schemas/order.yaml",
        "required: [id, name]",
        "required: [id, name, email]",
    );
    orders.edit(
        "api/schemas/order.yaml",
        "Customer:\n  type: object\n  required: [id, name, email]\n  properties:\n    id: {type: string}\n    name: {type: string}\n",
        "Customer:\n  type: object\n  required: [id, name, email]\n  properties:\n    id: {type: string}\n    name: {type: string}\n    email: {type: string}\n",
    );
    orders.tests_pass(&[
        "get_order_includes_the_customer",
        "subscribers_receive_order_placed",
    ]);
    orders.rqtk(
        &[
            "review",
            "ORD-SYS-0001",
            "--note",
            "The test asserts the full Customer schema.",
        ],
        0,
    );
    orders.assert_green();
    orders.publish("2.0.0");

    // The change does not come back to users: the webhook it consumes did not change.
    users.commit("before bump");
    users.pin("orders", "rqtk/2.0.0");
    users.sync();
    users.assert_green();

    web.commit("before bump");
    web.pin("orders", "rqtk/2.0.0");
    web.sync();
    web.assert_suspect_because("WEB-SYS-0001", "orders:ORD-SYS-0001");
    web.rqtk(
        &[
            "review",
            "WEB-SYS-0001",
            "--note",
            "The order page ignores the new email field.",
        ],
        0,
    );
    web.assert_green();

    phase("7. users replaces GET /users/{id} with /v2, and orders migrates");
    users.edit(
        "api/openapi.yaml",
        "  /users/{id}:\n",
        "  /v2/users/{id}:\n",
    );
    fs::remove_file(users.root.join(".rqtk/requirements/SYS/USR-SYS-0001.toml")).unwrap();
    let v2 = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/interfaces/users/.rqtk/requirements/SYS/USR-SYS-0001.toml"),
    )
    .unwrap()
    .replace("USR-SYS-0001", "USR-SYS-0003")
    .replace("VA-SYS-0001-01", "VA-SYS-0003-01")
    .replace("GET /users/{id}", "GET /v2/users/{id}")
    .replace("~1users~1{id}", "~1v2~1users~1{id}");
    users.write(".rqtk/requirements/SYS/USR-SYS-0003.toml", &v2);
    users.write(
        "tests/users.rs",
        &format!(
            "#[{}(\"VA-SYS-0003-01\")]\n#[test]\nfn get_user_v2_returns_the_user() {{}}\n",
            "verifies"
        ),
    );
    users.tests_pass(&[
        "get_user_v2_returns_the_user",
        "order_placed_updates_last_order",
    ]);
    users.assert_green();
    users.publish("3.0.0");

    orders.commit("before bump");
    orders.pin("users", "rqtk/3.0.0");
    orders.sync();
    assert_eq!(
        orders.lint_codes(),
        ["RQ039"],
        "the interface orders consumes is gone"
    );
    orders.edit(
        ".rqtk/requirements/SYS/ORD-SYS-0001.toml",
        "users:USR-SYS-0001",
        "users:USR-SYS-0003",
    );
    orders.tests_pass(&[
        "get_order_includes_the_customer",
        "subscribers_receive_order_placed",
    ]);
    orders.rqtk(
        &[
            "review",
            "ORD-SYS-0001",
            "--note",
            "v2 returns the same User.",
        ],
        0,
    );
    orders.assert_green();
    let (trace, _) = orders.rqtk(&["trace", "ORD-SYS-0001"], 0);
    assert_eq!(trace["consumes"][0]["id"], "users:USR-SYS-0003");
    assert_eq!(trace["consumes"][0]["provider_status"], "verified");
}
