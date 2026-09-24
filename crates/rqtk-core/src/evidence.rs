//! Verification evidence: test results matched to verification activities, recorded in
//! `.rqtk/evidence.toml` against the content hash of the requirement they verified.

use crate::error::RqtkError;
use crate::model::{Requirement, RequirementId, SCHEMA_VERSION};
use crate::repository::{RequirementSet, Validated};
use crate::scan::SourceLink;
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

/// Location of the evidence file relative to the repository root.
pub const EVIDENCE_PATH: &str = ".rqtk/evidence.toml";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    Passed,
    Failed,
}

/// The latest recorded result for one verification activity.
///
/// Unknown fields are ignored so that files written by a later rqtk still load.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ActivityEvidence {
    /// Verification activity ID.
    pub id: String,
    /// Requirement that owned the activity when the evidence was recorded.
    pub requirement: RequirementId,
    /// The requirement's content hash when the evidence was recorded. If the requirement's
    /// current hash differs, the evidence is stale and the activity is Suspect.
    pub requirement_hash: String,
    pub outcome: Outcome,
    /// Test cases that produced the outcome, as `classname::name`.
    pub tests: Vec<String>,
    /// Commit checked out when the evidence was recorded.
    pub commit: Option<String>,
    /// Hash of the linked tests' source when they last ran.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tests_hash: Option<String>,
    /// The requirement was re-verified after it changed, by the same tests that passed for
    /// its earlier wording. It stays Suspect until someone runs `rqtk review`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub unchanged_tests: bool,
    /// Content hashes of the requirement's ancestors and of the needs they satisfy, when this
    /// version of the requirement was first verified. A later change to any of them makes
    /// the requirement Suspect until it is reviewed.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub upstream: BTreeMap<String, String>,
}

impl ActivityEvidence {
    /// Equal apart from the commit and the test source hash: editing a test that still
    /// passes doesn't make committed evidence out of date.
    fn same_result(&self, other: &Self) -> bool {
        self.requirement == other.requirement
            && self.requirement_hash == other.requirement_hash
            && self.outcome == other.outcome
            && self.tests == other.tests
            && self.unchanged_tests == other.unchanged_tests
            && (self.upstream.is_empty()
                || other.upstream.is_empty()
                || self.upstream == other.upstream)
    }
}

/// A person's or agent's confirmation that a requirement still holds after something it
/// depends on changed, recorded with `rqtk review`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Review {
    pub requirement: RequirementId,
    /// The requirement's content hash when it was reviewed; a later change voids the review.
    pub requirement_hash: String,
    /// Content hashes of its ancestors and their needs when it was reviewed.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub upstream: BTreeMap<String, String>,
    /// Date of the review, `YYYY-MM-DD`.
    pub date: String,
    /// Commit checked out when the review was recorded.
    pub commit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EvidenceFile {
    pub schema_version: u32,
    /// Version of rqtk that last wrote the file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub written_by: Option<String>,
    #[serde(default, rename = "activity")]
    pub activities: Vec<ActivityEvidence>,
    #[serde(default, rename = "review", skip_serializing_if = "Vec::is_empty")]
    pub reviews: Vec<Review>,
}

/// All recorded evidence, keyed by activity ID, and reviews, keyed by requirement.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Evidence {
    pub activities: BTreeMap<String, ActivityEvidence>,
    pub reviews: BTreeMap<RequirementId, Review>,
    /// Set when `apply` refreshed bookkeeping (test source hashes, fields added by a newer
    /// rqtk) without changing any result; saving is worthwhile but not required.
    pub refreshed: bool,
}

/// How one activity's entry changed when new results were applied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum EvidenceChange {
    Added(ActivityEvidence),
    Updated {
        before: ActivityEvidence,
        after: ActivityEvidence,
    },
    Removed(ActivityEvidence),
}

impl EvidenceChange {
    pub fn activity(&self) -> &str {
        match self {
            EvidenceChange::Added(e) | EvidenceChange::Removed(e) => &e.id,
            EvidenceChange::Updated { after, .. } => &after.id,
        }
    }
}

impl Evidence {
    /// Load `.rqtk/evidence.toml`; a missing file is empty evidence.
    pub fn load(repo_root: &Path) -> Result<Self, RqtkError> {
        let path = repo_root.join(EVIDENCE_PATH);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(source) => return Err(RqtkError::Io { path, source }),
        };
        let file: EvidenceFile = toml::from_str(&text).map_err(|source| RqtkError::TomlParse {
            path: path.clone(),
            source,
        })?;
        if file.schema_version != SCHEMA_VERSION {
            return Err(RqtkError::UnsupportedSchemaVersion {
                path,
                found: file.schema_version,
                supported: SCHEMA_VERSION,
            });
        }
        Ok(Self {
            activities: file
                .activities
                .into_iter()
                .map(|e| (e.id.clone(), e))
                .collect(),
            reviews: file
                .reviews
                .into_iter()
                .map(|r| (r.requirement.clone(), r))
                .collect(),
            refreshed: false,
        })
    }

    /// Write `.rqtk/evidence.toml`, sorted by activity ID so diffs stay minimal.
    /// `written_by` is the version of the tool writing it, recorded in the file.
    pub fn save(&self, repo_root: &Path, written_by: &str) -> Result<(), RqtkError> {
        let path = repo_root.join(EVIDENCE_PATH);
        let file = EvidenceFile {
            schema_version: SCHEMA_VERSION,
            written_by: Some(written_by.to_owned()),
            activities: self.activities.values().cloned().collect(),
            reviews: self.reviews.values().cloned().collect(),
        };
        let text = format!(
            "# Generated by `rqtk verify` and `rqtk review`. Commit this file; do not edit by hand.\n{}",
            toml::to_string_pretty(&file)?
        );
        std::fs::write(&path, text).map_err(|source| RqtkError::Io { path, source })
    }

    /// Record the outcomes of a test run. Only activities with a complete result in `runs`
    /// are touched, so partial runs (e.g. one language's tests) keep other evidence. An entry
    /// whose result is unchanged keeps its original commit. Evidence for activities that no
    /// longer exist in any requirement is removed, and so are reviews of requirements that
    /// no longer exist.
    ///
    /// When a requirement changed since its tests last passed and the same tests (by source
    /// hash) pass again, the entry is marked `unchanged_tests`: rerunning a test proves
    /// nothing about a statement it was never updated for.
    pub fn apply(
        &mut self,
        set: &RequirementSet<Validated>,
        runs: &BTreeMap<String, ActivityRun>,
        commit: Option<&str>,
    ) -> Vec<EvidenceChange> {
        let owners: BTreeMap<&str, &Requirement> = set
            .requirements()
            .values()
            .flat_map(|r| {
                r.verification
                    .activities
                    .iter()
                    .map(move |a| (a.id.as_str(), r))
            })
            .collect();

        let mut changes = Vec::new();
        for (activity, run) in runs {
            let (Some(req), Some(outcome)) = (owners.get(activity.as_str()), run.outcome()) else {
                continue;
            };
            let requirement_hash = req.compute_content_hash();
            let previous = self
                .activities
                .get(activity)
                .filter(|p| p.requirement == req.id);
            let (upstream, unchanged_tests) = match previous {
                // Same wording: keep the baseline the requirement was first verified against.
                // Tests that were unchanged stay so until their source changes.
                Some(p) if p.requirement_hash == requirement_hash => {
                    let upstream = if p.upstream.is_empty() {
                        set.upstream_hashes(&req.id)
                    } else {
                        p.upstream.clone()
                    };
                    let still_unchanged = p.unchanged_tests
                        && (p.tests_hash.is_none() || p.tests_hash == run.tests_hash);
                    (upstream, still_unchanged)
                }
                Some(p) => (
                    set.upstream_hashes(&req.id),
                    p.tests_hash.is_some() && p.tests_hash == run.tests_hash,
                ),
                None => (set.upstream_hashes(&req.id), false),
            };
            // A review answers the evidence it saw. New unchanged-test evidence for this
            // wording needs a review recorded after it.
            let newly_unchanged =
                unchanged_tests && previous.is_some_and(|p| p.requirement_hash != requirement_hash);
            if newly_unchanged {
                self.reviews.remove(&req.id);
            }
            let entry = ActivityEvidence {
                id: activity.clone(),
                requirement: req.id.clone(),
                requirement_hash,
                outcome,
                tests: run.tests.clone(),
                commit: commit.map(str::to_owned),
                tests_hash: run.tests_hash.clone(),
                unchanged_tests,
                upstream,
            };
            match self.activities.get_mut(activity) {
                Some(existing) if existing.same_result(&entry) => {
                    let fill_upstream = existing.upstream.is_empty() && !entry.upstream.is_empty();
                    if existing.tests_hash != entry.tests_hash || fill_upstream {
                        existing.tests_hash = entry.tests_hash;
                        existing.upstream = entry.upstream;
                        self.refreshed = true;
                    }
                }
                Some(existing) => {
                    changes.push(EvidenceChange::Updated {
                        before: existing.clone(),
                        after: entry.clone(),
                    });
                    *existing = entry;
                }
                None => {
                    changes.push(EvidenceChange::Added(entry.clone()));
                    self.activities.insert(activity.clone(), entry);
                }
            }
        }

        let stale: Vec<String> = self
            .activities
            .keys()
            .filter(|id| !owners.contains_key(id.as_str()))
            .cloned()
            .collect();
        for id in stale {
            if let Some(removed) = self.activities.remove(&id) {
                changes.push(EvidenceChange::Removed(removed));
            }
        }
        let before = self.reviews.len();
        self.reviews
            .retain(|id, _| set.requirements().contains_key(id));
        self.refreshed |= self.reviews.len() != before;
        changes.sort_by(|a, b| a.activity().cmp(b.activity()));
        changes
    }

    /// Record a review of `id` against its current wording and upstream. Returns the review.
    pub fn review(
        &mut self,
        set: &RequirementSet<Validated>,
        id: &RequirementId,
        date: String,
        commit: Option<&str>,
        note: Option<String>,
    ) -> Option<Review> {
        let req = set.requirements().get(id)?;
        let review = Review {
            requirement: id.clone(),
            requirement_hash: req.compute_content_hash(),
            upstream: set.upstream_hashes(id),
            date,
            commit: commit.map(str::to_owned),
            note,
        };
        self.reviews.insert(id.clone(), review.clone());
        Some(review)
    }
}

// ── Test results ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestOutcome {
    Passed,
    Failed,
    Skipped,
}

/// One `<testcase>` from a JUnit XML report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestResult {
    pub name: String,
    pub classname: String,
    pub outcome: TestOutcome,
    /// Source file of the test, when the runner reports it (Bun, vitest, GoogleTest, …).
    pub file: Option<String>,
    /// 1-based line of the test in `file`.
    pub line: Option<usize>,
}

impl TestResult {
    pub fn id(&self) -> String {
        if self.classname.is_empty() {
            self.name.clone()
        } else {
            format!("{}::{}", self.classname, self.name)
        }
    }
}

/// Read test results from a JUnit XML file, as written by cargo-nextest, pytest
/// (`--junitxml`), go-junit-report, jest-junit, Maven Surefire and most CI tools.
pub fn read_junit(path: &Path) -> Result<Vec<TestResult>, RqtkError> {
    let xml = std::fs::read_to_string(path).map_err(|source| RqtkError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    parse_junit(&xml).map_err(|message| RqtkError::TestResults {
        path: PathBuf::from(path),
        message,
    })
}

/// Parse JUnit XML. Several documents may be concatenated, as `cargo test -- --format junit`
/// prints one per test binary.
pub fn parse_junit(xml: &str) -> Result<Vec<TestResult>, String> {
    let mut results = Vec::new();
    for doc in xml.split("<?xml").filter(|d| !d.trim().is_empty()) {
        // Drop the remainder of the XML declaration, if this chunk had one.
        let body = match doc.find("?>") {
            Some(end) if doc.trim_start().starts_with("version") => &doc[end + 2..],
            _ => doc,
        };
        results.extend(parse_junit_document(body)?);
    }
    Ok(results)
}

fn parse_junit_document(xml: &str) -> Result<Vec<TestResult>, String> {
    let mut reader = Reader::from_str(xml);
    let mut results = Vec::new();
    let mut open: Option<TestResult> = None;
    loop {
        match reader.read_event().map_err(|e| e.to_string())? {
            Event::Start(e) if e.local_name().as_ref() == b"testcase" => {
                open = Some(testcase(&e)?);
            }
            Event::Empty(e) if e.local_name().as_ref() == b"testcase" => {
                results.push(testcase(&e)?);
            }
            Event::Start(e) | Event::Empty(e) => {
                if let Some(case) = open.as_mut() {
                    match e.local_name().as_ref() {
                        b"failure" | b"error" => case.outcome = TestOutcome::Failed,
                        b"skipped" if case.outcome == TestOutcome::Passed => {
                            case.outcome = TestOutcome::Skipped;
                        }
                        _ => {}
                    }
                }
            }
            Event::End(e) if e.local_name().as_ref() == b"testcase" => {
                results.extend(open.take());
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(results)
}

fn testcase(e: &BytesStart<'_>) -> Result<TestResult, String> {
    let mut name = None;
    let mut classname = String::new();
    let mut file = None;
    let mut line = None;
    let mut outcome = TestOutcome::Passed;
    for attr in e.attributes() {
        let attr = attr.map_err(|e| e.to_string())?;
        let value = attr
            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
            .map_err(|e| e.to_string())?;
        match attr.key.local_name().as_ref() {
            b"name" => name = Some(value.into_owned()),
            b"classname" => classname = value.into_owned(),
            b"file" => file = Some(value.into_owned()),
            b"line" => line = value.trim().parse().ok(),
            // CTest and GoogleTest mark disabled tests with an attribute, not <skipped/>.
            b"status" | b"result" => match value.trim().to_ascii_lowercase().as_str() {
                "disabled" | "notrun" | "not run" | "skipped" | "skip" | "suppressed" => {
                    outcome = TestOutcome::Skipped;
                }
                "fail" | "failed" | "failure" | "error" => outcome = TestOutcome::Failed,
                _ => {}
            },
            _ => {}
        }
    }
    Ok(TestResult {
        name: name.ok_or("<testcase> without a name attribute")?,
        classname,
        outcome,
        file,
        line,
    })
}

// ── Matching results to links ────────────────────────────────────────────────

/// What a test run says about one activity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ActivityRun {
    /// IDs of the matched test cases, sorted.
    pub tests: Vec<String>,
    pub any_failed: bool,
    /// Linked test functions with no passing or failing result in the run
    /// (absent, or skipped).
    pub missing: Vec<String>,
    /// Linked tests whose name matches several distinct tests in the results, so the
    /// run can't say which one is linked.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ambiguous: Vec<Ambiguity>,
    /// Combined source hash of the linked tests, when the scan could read them.
    #[serde(skip)]
    pub tests_hash: Option<String>,
}

/// A linked test that matched several distinct tests in the results.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ambiguity {
    /// The linked test, as `path:line name`.
    pub link: String,
    /// The matching test cases.
    pub candidates: Vec<String>,
}

impl ActivityRun {
    /// `Failed` if any linked test failed; `Passed` only if every linked test ran and passed
    /// unambiguously; `None` when the run is incomplete for this activity.
    pub fn outcome(&self) -> Option<Outcome> {
        if self.any_failed {
            Some(Outcome::Failed)
        } else if self.missing.is_empty() && self.ambiguous.is_empty() && !self.tests.is_empty() {
            Some(Outcome::Passed)
        } else {
            None
        }
    }
}

/// Match test results to the activities their tests are linked to. Activities none of
/// whose linked tests appear in `results` are left out, so a partial run (one language's
/// tests) says nothing about them.
pub fn match_results(
    links: &[SourceLink],
    results: &[TestResult],
) -> BTreeMap<String, ActivityRun> {
    let mut by_activity: BTreeMap<&str, Vec<&SourceLink>> = BTreeMap::new();
    for link in links {
        by_activity.entry(&link.activity).or_default().push(link);
    }

    let mut runs = BTreeMap::new();
    for (activity, links) in by_activity {
        let mut tests = BTreeSet::new();
        let mut any_failed = false;
        let mut missing = Vec::new();
        let mut ambiguous = Vec::new();
        let mut seen = false;
        for link in &links {
            let Some(test_name) = &link.test_name else {
                continue;
            };
            let matched = results_for(link, results);
            seen |= !matched.tests.is_empty();
            if matched.distinct.len() > 1 {
                ambiguous.push(Ambiguity {
                    link: format!("{}:{} {test_name}", link.path.display(), link.line),
                    candidates: matched.distinct,
                });
                continue;
            }
            let ran: Vec<_> = matched
                .tests
                .iter()
                .filter(|r| r.outcome != TestOutcome::Skipped)
                .collect();
            if ran.is_empty() {
                missing.push(test_name.clone());
            }
            for r in ran {
                any_failed |= r.outcome == TestOutcome::Failed;
                tests.insert(r.id());
            }
        }
        if !seen {
            continue;
        }
        let tests_hash = combined_hash(&links);
        runs.insert(
            activity.to_owned(),
            ActivityRun {
                tests: tests.into_iter().collect(),
                any_failed,
                missing,
                ambiguous,
                tests_hash,
            },
        );
    }
    runs
}

/// One hash over the source of every test linked to an activity, or `None` if any of them
/// couldn't be read.
pub(crate) fn combined_hash(links: &[&SourceLink]) -> Option<String> {
    let mut hashes: Vec<&str> = links
        .iter()
        .map(|l| l.source_hash.as_deref())
        .collect::<Option<_>>()?;
    hashes.sort_unstable();
    hashes.dedup();
    Some(hashes.join("+"))
}

struct Matched<'a> {
    tests: Vec<&'a TestResult>,
    /// Distinct tests among `tests` (parameterised instances of one test count once);
    /// more than one means the link is ambiguous.
    distinct: Vec<String>,
}

/// Results for one link: by name (with suite, case or group), then narrowed by the source
/// file the runner reports, then by what the link's path says about the classname.
fn results_for<'a>(link: &SourceLink, results: &'a [TestResult]) -> Matched<'a> {
    let pattern = NamePattern::new(link);
    let mut candidates: Vec<&TestResult> = results.iter().filter(|r| pattern.matches(r)).collect();

    if candidates.iter().any(|r| r.file.is_some()) {
        candidates.retain(|r| r.file.as_deref().is_none_or(|f| same_file(f, &link.path)));
    }
    let hinted: Vec<&TestResult> = candidates
        .iter()
        .copied()
        .filter(|r| path_hint_matches(&link.path, r))
        .collect();
    if !hinted.is_empty() {
        candidates = hinted;
    }

    let mut distinct: Vec<String> = if link.group {
        Vec::new()
    } else {
        let mut keys: Vec<String> = candidates
            .iter()
            .map(|r| {
                r.file
                    .clone()
                    .unwrap_or_else(|| base_name(&r.classname).to_owned())
            })
            .collect();
        keys.sort();
        keys.dedup();
        if keys.len() > 1 {
            candidates.iter().map(|r| r.id()).collect()
        } else {
            Vec::new()
        }
    };
    distinct.sort();
    distinct.dedup();
    Matched {
        tests: candidates,
        distinct,
    }
}

/// Whether two paths name the same file: one is a suffix of the other, component-wise, so a
/// runner's absolute path or a path relative to a sub-package matches the scanned path.
fn same_file(reported: &str, scanned: &Path) -> bool {
    let reported: Vec<&str> = reported
        .split(['/', '\\'])
        .filter(|c| !c.is_empty() && *c != ".")
        .collect();
    let scanned: Vec<&str> = scanned
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .filter(|c| *c != ".")
        .collect();
    let n = reported.len().min(scanned.len());
    n > 0 && reported[reported.len() - n..] == scanned[scanned.len() - n..]
}

/// Whether a result's classname or name is consistent with the link's source file.
fn path_hint_matches(path: &Path, r: &TestResult) -> bool {
    let hay = format!("{} {}", r.classname, r.name);
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    let stem = stem.split('.').next().unwrap_or(stem);
    if path.extension().is_some_and(|e| e == "rs") {
        // libtest names the classname after the module (`tests`) for unit tests and
        // `integration` for the top level of an integration test binary.
        let integration = path.components().any(|c| c.as_os_str() == "tests");
        return contains_word(&hay, stem) || (r.classname == "integration") == integration;
    }
    let as_path = path.to_string_lossy().replace('\\', "/");
    contains_word(&hay, stem) || hay.contains(&as_path)
}

/// `needle` occurs in `hay` delimited by non-alphanumeric characters (or the ends).
fn contains_word(hay: &str, needle: &str) -> bool {
    !needle.is_empty()
        && hay.match_indices(needle).any(|(i, _)| {
            let before = hay[..i].chars().next_back();
            let after = hay[i + needle.len()..].chars().next();
            before.is_none_or(|c| !c.is_alphanumeric() && c != '_')
                && after.is_none_or(|c| !c.is_alphanumeric() && c != '_')
        })
}

/// A test name as a runner reports it, without parameters: pytest's `test_x[case]`,
/// JUnit's `method(int, int)[1]`, GoogleTest's `Name/0`, and `DISABLED_` prefixes.
fn base_name(name: &str) -> &str {
    static PARAMS: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^(.*?[\w$])(?:\(.*\))?(?:\[[^\]]*\])?(?:/\d+)?$").unwrap());
    let name = name.trim();
    let name = name.strip_suffix("()").unwrap_or(name);
    PARAMS
        .captures(name)
        .and_then(|c| c.get(1))
        .map_or(name, |m| m.as_str())
}

/// What a link's test is called in the results.
struct NamePattern {
    /// `Suite.Name` or `Name`, without `DISABLED_`.
    expected: String,
    /// Set when the name has placeholders (`it.each`'s `%i`, `$x`, `${x}`).
    template: Option<Regex>,
    case: Option<String>,
    group: bool,
}

impl NamePattern {
    fn new(link: &SourceLink) -> Self {
        let name = link
            .test_name
            .as_deref()
            .unwrap_or_default()
            .replace("DISABLED_", "");
        let expected = match &link.suite {
            Some(suite) => format!("{}.{name}", suite.replace("DISABLED_", "")),
            None => name,
        };
        static PLACEHOLDER: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"%[sdifjoc#]|\$\{[^}]*\}|\$\w+").unwrap());
        let template = PLACEHOLDER.is_match(&expected).then(|| {
            let mut rx = String::from(r"(?:^|[.:/> #$])");
            let mut last = 0;
            for m in PLACEHOLDER.find_iter(&expected) {
                rx.push_str(&regex::escape(&expected[last..m.start()]));
                rx.push_str(".+?");
                last = m.end();
            }
            rx.push_str(&regex::escape(&expected[last..]));
            rx.push('$');
            Regex::new(&rx).expect("escaped pattern")
        });
        NamePattern {
            expected,
            template,
            case: link.case.clone(),
            group: link.group,
        }
    }

    fn matches(&self, r: &TestResult) -> bool {
        let mut names = vec![r.name.replace("DISABLED_", "")];
        if !r.classname.is_empty() && !r.name.starts_with(&r.classname) {
            names.push(format!("{}.{}", r.classname, r.name).replace("DISABLED_", ""));
        }
        names.iter().any(|name| {
            if self.group {
                let hay = format!("{} {name}", r.classname);
                return contains_segment(&hay, &self.expected);
            }
            match &self.case {
                Some(case) => self.matches_case(name, case),
                None => self.matches_name(base_name(name)),
            }
        })
    }

    fn matches_name(&self, name: &str) -> bool {
        if let Some(template) = &self.template {
            return template.is_match(name);
        }
        match name.strip_suffix(self.expected.as_str()) {
            Some("") => true,
            Some(prefix) => prefix.ends_with(['.', ':', ' ', '/', '>', '#', '$']),
            None => false,
        }
    }

    /// Go's `TestX/case_name` and pytest's `test_x[case]`.
    fn matches_case(&self, name: &str, case: &str) -> bool {
        let underscored = case.replace(' ', "_");
        if let Some((head, rest)) = name.split_once('[') {
            let id = rest.strip_suffix(']').unwrap_or(rest);
            return self.matches_name(head)
                && (id == case || id.split('-').any(|part| part == case));
        }
        name.rsplit_once('/').is_some_and(|(head, tail)| {
            self.matches_name(head) && (tail == case || tail == underscored)
        })
    }
}

/// `needle` occurs in `hay` between separators (` > `, `.`, `::`, `/`, spaces) or the ends.
fn contains_segment(hay: &str, needle: &str) -> bool {
    let sep =
        |c: Option<char>| c.is_none_or(|c| matches!(c, '.' | ':' | '/' | '>' | ' ' | '#' | '$'));
    hay.match_indices(needle).any(|(i, _)| {
        sep(hay[..i].chars().next_back()) && sep(hay[i + needle.len()..].chars().next())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const JUNIT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<testsuites>
  <testsuite name="rqtk::integration_test" tests="4">
    <testcase name="lint_ok" classname="rqtk::integration_test" time="0.1"/>
    <testcase name="tests::lint_bad" classname="rqtk::integration_test">
      <failure message="assertion failed">boom &amp; more</failure>
    </testcase>
    <testcase name="later" classname="rqtk::integration_test"><skipped/></testcase>
    <testcase name="test_param[a-1]" classname="tests.test_mod"></testcase>
  </testsuite>
</testsuites>"#;

    fn link(activity: &str, path: &str, test: &str) -> SourceLink {
        SourceLink {
            activity: activity.to_owned(),
            path: PathBuf::from(path),
            line: 1,
            test_name: Some(test.to_owned()),
            ..SourceLink::default()
        }
    }

    // rqtk: verifies VA-CORE-006-02
    #[test]
    fn parses_outcomes() {
        let results = parse_junit(JUNIT).unwrap();
        let outcomes: Vec<_> = results
            .iter()
            .map(|r| (r.name.as_str(), r.outcome))
            .collect();
        assert_eq!(
            outcomes,
            vec![
                ("lint_ok", TestOutcome::Passed),
                ("tests::lint_bad", TestOutcome::Failed),
                ("later", TestOutcome::Skipped),
                ("test_param[a-1]", TestOutcome::Passed),
            ]
        );
    }

    // rqtk: verifies VA-CORE-006-02
    #[test]
    fn parses_concatenated_documents() {
        let one = r#"<?xml version="1.0" encoding="UTF-8"?><testsuites><testsuite name="a"><testcase classname="x" name="t1"/></testsuite></testsuites>"#;
        let two = r#"<?xml version="1.0" encoding="UTF-8"?><testsuites><testsuite name="b"><testcase classname="y" name="t2"><failure/></testcase></testsuite></testsuites>"#;
        let results = parse_junit(&format!("{one}\n{two}\n")).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[1].outcome, TestOutcome::Failed);
    }

    // rqtk: verifies VA-CORE-006-02
    #[test]
    fn matches_by_name_suffix_and_parametrisation() {
        let matches = |result: &str, test: &str| {
            NamePattern::new(&link("A", "x", test)).matches(&result_named(result, ""))
        };
        assert!(matches("tests::lint_bad", "lint_bad"));
        assert!(matches("test_param[a-1]", "test_param"));
        assert!(matches("Suite boots quickly", "boots quickly"));
        assert!(matches("parameterized(int, int)[1]", "parameterized"));
        assert!(matches(
            "rejects 450-480 outside hours",
            "rejects %i-%i outside hours"
        ));
        assert!(!matches("not_lint_bad", "lint_bad"));
        assert!(!matches("TestTable/some_case", "TestTable"));
    }

    fn result_named(name: &str, classname: &str) -> TestResult {
        TestResult {
            name: name.into(),
            classname: classname.into(),
            outcome: TestOutcome::Passed,
            file: None,
            line: None,
        }
    }

    // rqtk: verifies VA-CORE-006-02
    #[test]
    fn disabled_status_attributes_count_as_skipped() {
        let xml = r#"<testsuite><testcase name="a" status="disabled"/><testcase name="b" status="notrun" result="suppressed"/><testcase name="c" status="fail"/></testsuite>"#;
        let outcomes: Vec<_> = parse_junit(xml)
            .unwrap()
            .into_iter()
            .map(|r| r.outcome)
            .collect();
        assert_eq!(
            outcomes,
            vec![
                TestOutcome::Skipped,
                TestOutcome::Skipped,
                TestOutcome::Failed
            ]
        );
    }

    // rqtk: verifies VA-CORE-006-02
    #[test]
    fn same_name_in_two_places_is_ambiguous_not_both() {
        let results = vec![
            TestResult {
                outcome: TestOutcome::Failed,
                ..result_named("cut_off", "alpha")
            },
            result_named("cut_off", "beta"),
        ];
        let runs = match_results(&[link("A", "src/gamma.py", "cut_off")], &results);
        assert_eq!(runs["A"].outcome(), None);
        assert_eq!(
            runs["A"].ambiguous[0].candidates,
            vec!["alpha::cut_off", "beta::cut_off"]
        );
    }

    // rqtk: verifies VA-CORE-006-02
    #[test]
    fn activity_outcomes() {
        let results = parse_junit(JUNIT).unwrap();
        let links = [
            link("A", "tests/integration_test.rs", "lint_ok"),
            link("B", "tests/integration_test.rs", "lint_ok"),
            link("B", "tests/integration_test.rs", "lint_bad"),
            link("C", "tests/integration_test.rs", "lint_ok"),
            link("C", "tests/integration_test.rs", "later"),
            link("D", "tests/other.rs", "never_ran"),
        ];
        let runs = match_results(&links, &results);
        assert_eq!(runs["A"].outcome(), Some(Outcome::Passed));
        assert_eq!(runs["A"].tests, vec!["rqtk::integration_test::lint_ok"]);
        assert_eq!(runs["B"].outcome(), Some(Outcome::Failed));
        assert_eq!(
            runs["C"].outcome(),
            None,
            "skipped test leaves C incomplete"
        );
        assert_eq!(runs["C"].missing, vec!["later"]);
        assert!(!runs.contains_key("D"));
    }

    // rqtk: verifies VA-CORE-006-02
    #[test]
    fn prefers_results_from_the_links_file() {
        let results = vec![
            TestResult {
                outcome: TestOutcome::Failed,
                ..result_named("works", "crate::alpha")
            },
            result_named("works", "crate::beta"),
        ];
        let runs = match_results(&[link("A", "tests/beta.rs", "works")], &results);
        assert_eq!(runs["A"].outcome(), Some(Outcome::Passed));
    }
}
