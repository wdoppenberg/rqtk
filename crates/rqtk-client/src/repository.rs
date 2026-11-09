//! [`Repository`]: one rqtk repository at one revision, loaded once and queried in memory.

use crate::entries::{
    ActivityEntry, Link, LinkKind, NeedEntry, RequirementEntry, StakeholderEntry,
};
use crate::error::{Error, ErrorCode, Result};
use crate::filter::Filter;
use crate::views::{
    BaselineEntry, BaselinesDoc, Commit, ConsumedStatus, ContextDoc, CoverageDoc, DiffDoc,
    HistoryDoc, ImpactDoc, LintDoc, Modification, Modified, NeedStatus, RequirementStatus,
    TraceDoc, schema_id,
};
use rqtk_core::query::Direction;
use rqtk_core::{
    ActivityState, ClosureStatus, Diagnostic, EntityRef, Evidence, GitContext, RequirementId,
    RequirementSet, RequirementVerification, Severity, SourceLink, Validated,
};
use schemars::JsonSchema;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Where a [`Repository`] was opened from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Origin {
    /// A working tree.
    Checkout { path: PathBuf },
    /// A local repository at a revision, read from git objects.
    Ref { path: PathBuf, reference: String },
    /// A remote git URL at a revision, fetched into the cache.
    Remote { url: String, reference: String },
}

/// What a [`Repository`] is a snapshot of.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct RepositoryInfo {
    pub origin: Origin,
    /// The commit the files come from: the snapshot's for a ref or URL, HEAD for a checkout
    /// (whose working tree may differ from it). `None` outside git or before the first commit.
    pub commit: Option<String>,
    /// `[project] name` from the config.
    pub project: String,
}

/// One repository at one revision. Everything is loaded and checked when it opens; queries
/// answer from that snapshot until [`Repository::reload`].
pub struct Repository {
    set: RequirementSet<Validated>,
    diagnostics: Vec<Diagnostic>,
    checked: usize,
    links: Vec<SourceLink>,
    evidence: Evidence,
    statuses: BTreeMap<RequirementId, RequirementVerification>,
    /// Normalised SHA-256 of every file holding a linked test, as it was when opened.
    test_files: BTreeMap<PathBuf, String>,
    /// Links declared by other items, by the ID they point to.
    incoming: BTreeMap<String, Vec<Link>>,
    /// The tests linked to each activity.
    tests: BTreeMap<String, Vec<SourceLink>>,
    origin: Origin,
    /// For a remote: whether `file://` URLs were allowed, so `reload` can open it again.
    allow_file_url: bool,
    /// The extracted tree of a ref or remote snapshot; removed when the repository is dropped.
    _files: Option<tempfile::TempDir>,
}

impl std::fmt::Debug for Repository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Repository")
            .field("origin", &self.origin)
            .finish_non_exhaustive()
    }
}

impl Repository {
    /// Open the working tree at `path`, the directory holding `.rqtk/`.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let git = GitContext::open(&path)?;
        Self::load(path.clone(), git, Origin::Checkout { path }, None)
    }

    /// Open the repository at `path` as it was at `reference`: a tag, baseline name, branch or
    /// commit. Files are read from git objects; the working tree is not touched.
    pub fn open_ref(path: impl AsRef<Path>, reference: &str) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let git = GitContext::open(&path)?;
        let repo = git.repository().cloned().ok_or_else(|| {
            Error::new(
                ErrorCode::Git,
                format!("`{}` is not inside a git repository", path.display()),
            )
        })?;
        let commit = git
            .resolve_commit(reference)
            .map_err(|e| Error::new(ErrorCode::RefNotFound, format!("`{reference}`: {e}")))?;
        let prefix = relative_to(&path, &git.workdir);
        Self::from_commit(
            repo,
            commit,
            &prefix,
            Origin::Ref {
                path,
                reference: reference.to_owned(),
            },
        )
    }

    /// Load the snapshot of `commit`, with the rqtk root at `prefix` inside its tree.
    pub(crate) fn from_commit(
        repo: rqtk_core::git::gix::Repository,
        commit: rqtk_core::git::gix::ObjectId,
        prefix: &Path,
        origin: Origin,
    ) -> Result<Self> {
        let files = tempfile::tempdir().map_err(|e| {
            Error::new(
                ErrorCode::Io,
                format!("cannot create a temporary directory: {e}"),
            )
        })?;
        let root = files.path().to_path_buf();
        let git = GitContext::snapshot(repo, root.clone(), commit);
        git.extract_commit(commit, &root)?;
        Self::load(root.join(prefix), git, origin, Some(files))
    }

    fn load(
        root: PathBuf,
        git: GitContext,
        origin: Origin,
        files: Option<tempfile::TempDir>,
    ) -> Result<Self> {
        let loaded = RequirementSet::load_with_git(root.clone(), git)?;
        let checked =
            loaded.requirements().len() + loaded.needs().len() + loaded.stakeholders().len();
        let (set, mut diagnostics) = loaded.validate();
        let links = rqtk_core::scan::scan(set.repo_root(), &set.config().scan)?;
        diagnostics.extend(set.link_diagnostics(&links));
        for d in &mut diagnostics {
            if let Some(loc) = d.location.as_mut() {
                loc.path = relative_to(&loc.path, &root);
            }
        }
        // Unreadable evidence is a finding, not a reason to refuse the repository; a newer
        // evidence schema is, like a newer config.
        let evidence = match Evidence::load(set.repo_root()) {
            Ok(evidence) => evidence,
            Err(e @ rqtk_core::RqtkError::UnsupportedSchemaVersion { .. }) => return Err(e.into()),
            Err(e) => {
                diagnostics.push(
                    Diagnostic::new("RQ100", format!("cannot read verification evidence: {e}"))
                        .file(PathBuf::from(rqtk_core::evidence::EVIDENCE_PATH)),
                );
                Evidence::default()
            }
        };
        let statuses = set.verification_status(&links, &evidence);
        let test_files = links
            .iter()
            .map(|l| l.path.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .filter_map(|p| {
                let bytes = std::fs::read(root.join(&p)).ok()?;
                Some((p, content_hash(&bytes)))
            })
            .collect();
        let incoming = incoming_links(&set);
        let mut tests: BTreeMap<String, Vec<SourceLink>> = BTreeMap::new();
        for l in &links {
            tests.entry(l.activity.clone()).or_default().push(l.clone());
        }
        Ok(Self {
            incoming,
            tests,
            set,
            diagnostics,
            checked,
            links,
            evidence,
            statuses,
            test_files,
            origin,
            allow_file_url: false,
            _files: files,
        })
    }

    /// Open it again from the same origin: a fresh snapshot of the working tree, the ref (which
    /// may have moved) or the remote (fetched again).
    pub fn reload(&self) -> Result<Self> {
        match &self.origin {
            Origin::Checkout { path } => Self::open(path),
            Origin::Ref { path, reference } => Self::open_ref(path, reference),
            Origin::Remote { url, reference } => {
                #[cfg(feature = "remote")]
                {
                    crate::remote::open_remote(url, reference, self.allow_file_url)
                }
                #[cfg(not(feature = "remote"))]
                {
                    let _ = (url, reference);
                    Err(Error::new(
                        ErrorCode::RemoteRefused,
                        "this build has no remote support",
                    ))
                }
            }
        }
    }

    #[cfg_attr(not(feature = "remote"), allow(dead_code))]
    pub(crate) fn with_options(mut self, allow_file_url: bool) -> Self {
        self.allow_file_url = allow_file_url;
        self
    }

    // ── what it is ──────────────────────────────────────────────────────────

    pub fn info(&self) -> RepositoryInfo {
        RepositoryInfo {
            origin: self.origin.clone(),
            commit: self.set.git().head_commit().map(|c| c.full().to_owned()),
            project: self.set.config().project.name.clone(),
        }
    }

    pub fn config(&self) -> &rqtk_core::Config {
        self.set.config()
    }

    // ── entities and links ──────────────────────────────────────────────────

    fn path_of(&self, item: &EntityRef) -> PathBuf {
        self.set
            .path_of(item)
            .map(|p| relative_to(p, self.set.repo_root()))
            .unwrap_or_default()
    }

    fn status_of(&self, id: &RequirementId) -> ClosureStatus {
        self.statuses
            .get(id)
            .map_or(ClosureStatus::Gap, |v| v.status)
    }

    fn requirement_links(&self, id: &RequirementId) -> Vec<Link> {
        let req = &self.set.requirements()[id];
        let mut links = Vec::new();
        for (field, target) in req.trace.requirement_links() {
            links.push(link(field, Direction::Outgoing, &target.0));
        }
        for need in &req.trace.satisfies {
            links.push(Link {
                kind: LinkKind::Satisfies,
                direction: Direction::Outgoing,
                id: need.0.clone(),
            });
        }
        for consumed in &req.trace.consumes {
            links.push(Link {
                kind: LinkKind::Consumes,
                direction: Direction::Outgoing,
                id: consumed.0.clone(),
            });
        }
        for activity in &req.verification.activities {
            links.push(Link {
                kind: LinkKind::Activities,
                direction: Direction::Outgoing,
                id: activity.id.clone(),
            });
        }
        links.extend(
            self.incoming
                .get(&id.0)
                .into_iter()
                .flatten()
                .filter(|l| l.kind != LinkKind::Satisfies)
                .cloned(),
        );
        links
    }

    fn requirement_entry(&self, id: &RequirementId) -> RequirementEntry {
        RequirementEntry {
            id: id.0.clone(),
            path: self.path_of(&EntityRef::Requirement(id.clone())),
            status: self.status_of(id),
            requirement: self.set.requirements()[id].clone(),
            links: self.requirement_links(id),
        }
    }

    /// Every requirement, by ID.
    pub fn requirements(&self) -> Vec<RequirementEntry> {
        self.set
            .requirements()
            .keys()
            .map(|id| self.requirement_entry(id))
            .collect()
    }

    pub fn requirement(&self, id: &str) -> Option<RequirementEntry> {
        let id = RequirementId(id.to_owned());
        self.set
            .requirements()
            .contains_key(&id)
            .then(|| self.requirement_entry(&id))
    }

    /// The requirements matching `filter`, by ID.
    pub fn find_requirements(&self, filter: &Filter) -> Vec<RequirementEntry> {
        let below: Option<BTreeSet<RequirementId>> = filter.ancestor.as_ref().map(|a| {
            self.set
                .trace_view(&RequirementId(a.clone()))
                .map(|v| v.downward.into_iter().collect())
                .unwrap_or_default()
        });
        self.set
            .requirements()
            .iter()
            .filter(|(id, req)| {
                below.as_ref().is_none_or(|b| b.contains(*id))
                    && filter.matches_lists(self.status_of(id), req)
            })
            .map(|(id, _)| self.requirement_entry(id))
            .collect()
    }

    fn need_entry(&self, id: &rqtk_core::NeedId) -> NeedEntry {
        let need = &self.set.needs()[id];
        let mut links: Vec<Link> = need
            .stakeholders
            .iter()
            .map(|s| Link {
                kind: LinkKind::Stakeholders,
                direction: Direction::Outgoing,
                id: s.0.clone(),
            })
            .collect();
        links.extend(self.incoming.get(&id.0).into_iter().flatten().cloned());
        NeedEntry {
            id: id.0.clone(),
            path: self.path_of(&EntityRef::Need(id.clone())),
            satisfied: links.iter().any(|l| l.kind == LinkKind::Satisfies),
            need: need.clone(),
            links,
        }
    }

    pub fn needs(&self) -> Vec<NeedEntry> {
        self.set
            .needs()
            .keys()
            .map(|id| self.need_entry(id))
            .collect()
    }

    pub fn need(&self, id: &str) -> Option<NeedEntry> {
        let id = rqtk_core::NeedId(id.to_owned());
        self.set
            .needs()
            .contains_key(&id)
            .then(|| self.need_entry(&id))
    }

    fn stakeholder_entry(&self, id: &rqtk_core::StakeholderId) -> StakeholderEntry {
        StakeholderEntry {
            id: id.0.clone(),
            path: self.path_of(&EntityRef::Stakeholder(id.clone())),
            stakeholder: self.set.stakeholders()[id].clone(),
            links: self.incoming.get(&id.0).cloned().unwrap_or_default(),
        }
    }

    pub fn stakeholders(&self) -> Vec<StakeholderEntry> {
        self.set
            .stakeholders()
            .keys()
            .map(|id| self.stakeholder_entry(id))
            .collect()
    }

    pub fn stakeholder(&self, id: &str) -> Option<StakeholderEntry> {
        let id = rqtk_core::StakeholderId(id.to_owned());
        self.set
            .stakeholders()
            .contains_key(&id)
            .then(|| self.stakeholder_entry(&id))
    }

    /// Every verification activity, in requirement and file order.
    pub fn activities(&self) -> Vec<ActivityEntry> {
        let mut out = Vec::new();
        for (req_id, req) in self.set.requirements() {
            let states = self.statuses.get(req_id);
            for activity in &req.verification.activities {
                let state = states
                    .and_then(|v| v.activities.iter().find(|a| a.id == activity.id))
                    .map_or(ActivityState::NotRun, |a| a.state.clone());
                out.push(ActivityEntry {
                    id: activity.id.clone(),
                    requirement: req_id.0.clone(),
                    activity: activity.clone(),
                    state,
                    tests: self.tests.get(&activity.id).cloned().unwrap_or_default(),
                    evidence: self.evidence.activities.get(&activity.id).cloned(),
                });
            }
        }
        out
    }

    // ── views ───────────────────────────────────────────────────────────────

    /// Everything relevant to one requirement, need, stakeholder or consumed interface.
    pub fn context(&self, id: &str) -> Result<ContextDoc> {
        let context = self
            .set
            .context(
                id,
                &self.links,
                &self.evidence,
                &self.statuses,
                &self.diagnostics,
            )
            .ok_or_else(|| not_found(id))?;
        Ok(ContextDoc {
            schema: schema_id("context"),
            context,
        })
    }

    pub fn trace(&self, id: &str) -> Result<TraceDoc> {
        let req_id = RequirementId(id.to_owned());
        let req = self
            .set
            .requirements()
            .get(&req_id)
            .ok_or_else(|| not_found(id))?;
        let view = self.set.trace_view(&req_id)?;
        Ok(TraceDoc {
            schema: schema_id("trace"),
            id: id.to_owned(),
            ancestors: view.upward.into_iter().map(|r| r.0).collect(),
            descendants: view.downward.into_iter().map(|r| r.0).collect(),
            consumes: req
                .trace
                .consumes
                .iter()
                .map(|c| self.set.consumed_summary(c))
                .collect(),
        })
    }

    pub fn coverage(&self) -> CoverageDoc {
        let mut summary: BTreeMap<ClosureStatus, usize> = BTreeMap::new();
        for v in self.statuses.values() {
            *summary.entry(v.status).or_default() += 1;
        }
        let needs: Vec<NeedStatus> = self
            .set
            .satisfaction_closure()
            .into_keys()
            .map(|id| NeedStatus {
                satisfied_by: self
                    .set
                    .requirements()
                    .iter()
                    .filter(|(_, r)| r.trace.satisfies.contains(&id))
                    .map(|(rid, _)| rid.0.clone())
                    .collect(),
                id: id.0,
            })
            .collect();
        CoverageDoc {
            schema: schema_id("coverage"),
            summary,
            requirements: self
                .statuses
                .iter()
                .map(|(id, v)| RequirementStatus {
                    id: id.0.clone(),
                    verification: v.clone(),
                })
                .collect(),
            unsatisfied_needs: needs.iter().filter(|n| n.satisfied_by.is_empty()).count(),
            needs,
            consumed: self
                .set
                .consumed_interfaces()
                .into_iter()
                .map(|(interface, consumers)| ConsumedStatus {
                    interface,
                    consumers: consumers.into_iter().map(|r| r.0).collect(),
                })
                .collect(),
        }
    }

    pub fn lint(&self) -> LintDoc {
        let errors = self
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .count();
        LintDoc {
            schema: schema_id("lint"),
            errors,
            warnings: self.diagnostics.len() - errors,
            checked: self.checked,
            diagnostics: self.diagnostics.clone(),
        }
    }

    /// What changed since revision `base` and what that puts at risk. Test files count as
    /// changed when they differ from `base` as they were when the repository was opened.
    pub fn impact(&self, base: &str) -> Result<ImpactDoc> {
        let git = self.set.git();
        let root = self.set.repo_root();
        let mut changed = Vec::new();
        for (path, hash) in &self.test_files {
            let at_base = git
                .read_at(base, root, &path.to_string_lossy().replace('\\', "/"))
                .map_err(|e| ref_error(base, e))?;
            if at_base.is_none_or(|bytes| content_hash(&bytes) != *hash) {
                changed.push(path.clone());
            }
        }
        let impact = self
            .set
            .impact(base, &self.links, &self.evidence, &changed)?;
        Ok(ImpactDoc {
            schema: schema_id("impact"),
            impact,
        })
    }

    /// Requirements added, removed and modified between revisions `from` and `to`.
    pub fn diff(&self, from: &str, to: &str) -> Result<DiffDoc> {
        let diff = self.set.diff(from, to)?;
        Ok(DiffDoc {
            schema: schema_id("diff"),
            from: from.to_owned(),
            to: to.to_owned(),
            added: diff.added.into_iter().map(|r| r.0).collect(),
            removed: diff.removed.into_iter().map(|r| r.0).collect(),
            modified: diff
                .modified
                .into_iter()
                .map(|m| Modified {
                    id: m.id.0,
                    change: match m.change_kind {
                        rqtk_core::ChangeKind::Semantic => Modification::Semantic,
                        _ => Modification::Cosmetic,
                    },
                })
                .collect(),
        })
    }

    /// Every baseline (`rqtk/<name>` tag), oldest first.
    pub fn baselines(&self) -> Result<BaselinesDoc> {
        let baselines = self.set.git().baselines()?;
        Ok(BaselinesDoc {
            schema: schema_id("baselines"),
            baselines: baselines
                .into_iter()
                .map(|b| BaselineEntry {
                    tag: format!("{}{}", rqtk_core::git::BASELINE_TAG_PREFIX, b.name),
                    name: b.name.to_string(),
                    timestamp: b.timestamp.map(|t| t.to_rfc3339()),
                    message: b.message.trim_end().to_owned(),
                })
                .collect(),
        })
    }

    /// The commits that changed requirement `id`'s file, newest first.
    pub fn history(&self, id: &str) -> Result<HistoryDoc> {
        let req_id = RequirementId(id.to_owned());
        let path = self
            .set
            .path_of(&EntityRef::Requirement(req_id))
            .ok_or_else(|| not_found(id))?;
        let commits = self.set.git().requirement_history(path)?;
        Ok(HistoryDoc {
            schema: schema_id("history"),
            id: id.to_owned(),
            commits: commits
                .into_iter()
                .map(|c| Commit {
                    hash: c.hash.full().to_owned(),
                    author: c.author,
                    timestamp: c.timestamp.map(|t| t.to_rfc3339()),
                    message: c.message.trim_end().to_owned(),
                })
                .collect(),
        })
    }

    /// The loaded set, for the CLI's own commands. Not part of the stable API.
    #[doc(hidden)]
    pub fn requirement_set(&self) -> &RequirementSet<Validated> {
        &self.set
    }
}

/// Every link, filed under the item it points to, as that item sees it (`incoming`).
fn incoming_links(set: &RequirementSet<Validated>) -> BTreeMap<String, Vec<Link>> {
    let mut incoming: BTreeMap<String, Vec<Link>> = BTreeMap::new();
    let mut add = |target: &str, kind: LinkKind, from: &str| {
        incoming.entry(target.to_owned()).or_default().push(Link {
            kind,
            direction: Direction::Incoming,
            id: from.to_owned(),
        });
    };
    for (id, req) in set.requirements() {
        for (field, target) in req.trace.requirement_links() {
            add(
                &target.0,
                LinkKind::from_field(field).unwrap_or(LinkKind::Related),
                &id.0,
            );
        }
        for need in &req.trace.satisfies {
            add(&need.0, LinkKind::Satisfies, &id.0);
        }
    }
    for (id, need) in set.needs() {
        for stakeholder in &need.stakeholders {
            add(&stakeholder.0, LinkKind::Stakeholders, &id.0);
        }
    }
    incoming
}

fn link(field: &str, direction: Direction, id: &str) -> Link {
    Link {
        kind: LinkKind::from_field(field).unwrap_or(LinkKind::Related),
        direction,
        id: id.to_owned(),
    }
}

fn not_found(id: &str) -> Error {
    Error::new(
        ErrorCode::NotFound,
        format!("no requirement, need or stakeholder with ID `{id}`"),
    )
}

fn ref_error(reference: &str, e: rqtk_core::RqtkError) -> Error {
    let err = Error::from(e);
    if err.code == ErrorCode::Git && !err.message.contains("not inside a git repository") {
        Error::new(
            ErrorCode::RefNotFound,
            format!("`{reference}`: {}", err.message),
        )
    } else {
        err
    }
}

/// `path` relative to `root` when it lies inside it, comparing canonical paths.
pub(crate) fn relative_to(path: &Path, root: &Path) -> PathBuf {
    if let Ok(rel) = path.strip_prefix(root) {
        return rel.to_path_buf();
    }
    let canonical = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    canonical(path)
        .strip_prefix(canonical(root))
        .map(Path::to_path_buf)
        .unwrap_or_else(|_| path.to_path_buf())
}

fn content_hash(bytes: &[u8]) -> String {
    format!(
        "{:x}",
        Sha256::digest(rqtk_core::spec::normalize_line_endings(bytes))
    )
}
