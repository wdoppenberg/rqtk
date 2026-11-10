//! Read-only views over a validated requirement set, shaped for agents and reviewers:
//! everything relevant to one item ([`RequirementSet::context`]) and what a change touches
//! ([`RequirementSet::impact`]).

use crate::diagnostic::Diagnostic;
use crate::error::RqtkError;
use crate::evidence::{ActivityEvidence, Evidence};
use crate::model::{
    ConsumedRef, EntityRef, Need, NeedId, Requirement, RequirementId, Stakeholder, StakeholderId,
};
use crate::repository::{RequirementSet, Validated};
use crate::scan::SourceLink;
use crate::status::RequirementVerification;
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};

/// A one-line reference to another item.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Summary {
    pub id: String,
    pub title: String,
    pub statement: Option<String>,
}

/// A link from or to the item in context, labelled with the `trace` field it comes from.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct TraceLink {
    pub field: String,
    pub direction: Direction,
    pub id: String,
    pub title: Option<String>,
}

/// Which end of a link an item is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    /// The item declares the link.
    Outgoing,
    /// The other item declares the link.
    Incoming,
}

/// A vendored interface requirement as seen from a consumer.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ConsumedSummary {
    /// `<alias>:<ID>`.
    pub id: String,
    pub title: Option<String>,
    pub statement: Option<String>,
    /// The provider's verification status at the pinned commit. Informational: it does not
    /// count toward this repository's coverage.
    pub provider_status: Option<crate::status::ClosureStatus>,
    /// Whether the vendored snapshot has it.
    pub resolved: bool,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Context {
    Requirement(Box<RequirementContext>),
    Need(NeedContext),
    Stakeholder(StakeholderContext),
    /// A vendored interface requirement (`<alias>:<ID>`), read-only.
    Interface(Box<InterfaceContext>),
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct InterfaceContext {
    pub id: String,
    pub alias: String,
    pub path: PathBuf,
    pub requirement: Requirement,
    pub provider_status: Option<crate::status::ClosureStatus>,
    /// Local requirements that consume it.
    pub consumers: Vec<Summary>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct RequirementContext {
    pub path: PathBuf,
    pub requirement: Requirement,
    /// Ancestors via `trace.parents`, nearest first.
    pub ancestors: Vec<Summary>,
    /// Requirements that list this one in `trace.parents`.
    pub children: Vec<Summary>,
    /// Every other requirement link, in both directions.
    pub links: Vec<TraceLink>,
    pub needs: Vec<Summary>,
    /// Vendored interface requirements it consumes.
    pub consumes: Vec<ConsumedSummary>,
    pub verification: Option<RequirementVerification>,
    /// `verifies` annotations for this requirement's activities.
    pub tests: Vec<SourceLink>,
    pub evidence: Vec<ActivityEvidence>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct NeedContext {
    pub path: PathBuf,
    pub need: Need,
    pub stakeholders: Vec<Stakeholder>,
    pub satisfied_by: Vec<Summary>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct StakeholderContext {
    pub path: PathBuf,
    pub stakeholder: Stakeholder,
    pub needs: Vec<Summary>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ChangeKind {
    Added,
    Removed,
    /// Content hash changed: what is demanded or how it is verified.
    Semantic,
    /// Other fields changed.
    Cosmetic,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ItemChange {
    pub id: String,
    pub change: ChangeKind,
}

/// A requirement that did not change itself but depends on one that did.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Downstream {
    pub id: String,
    /// The changed requirements or needs it is reached from.
    pub via: Vec<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum ReverifyReason {
    /// The requirement is new, or its content hash changed.
    RequirementChanged,
    /// A file containing a linked test changed.
    TestChanged { path: PathBuf },
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Reverify {
    pub activity: String,
    pub requirement: RequirementId,
    pub reasons: Vec<ReverifyReason>,
    /// The linked tests to run, as `path:line name`.
    pub tests: Vec<String>,
    /// Recorded evidence already covers the change: the tests passed against the current
    /// wording, with their current source.
    pub done: bool,
}

/// What changed since `base` and what that puts at risk.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Impact {
    pub base: String,
    pub requirements: Vec<ItemChange>,
    pub needs: Vec<ItemChange>,
    /// Vendored interface requirements (`<alias>:<ID>`) whose hash in
    /// `.rqtk/interfaces.lock` changed since `base`.
    pub interfaces: Vec<ItemChange>,
    pub downstream: Vec<Downstream>,
    pub reverify: Vec<Reverify>,
}

impl Impact {
    pub fn is_empty(&self) -> bool {
        self.requirements.is_empty()
            && self.needs.is_empty()
            && self.interfaces.is_empty()
            && self.reverify.is_empty()
    }
}

impl RequirementSet<Validated> {
    fn relative(&self, path: &Path) -> PathBuf {
        path.strip_prefix(&self.repo_root)
            .unwrap_or(path)
            .to_path_buf()
    }

    fn summary(&self, id: &RequirementId) -> Summary {
        let req = self.requirements.get(id);
        Summary {
            id: id.0.clone(),
            title: req.map(|r| r.title.clone()).unwrap_or_default(),
            statement: req.map(|r| r.statement.clone()),
        }
    }

    /// A consumed reference, resolved against the vendored snapshots.
    pub fn consumed_summary(&self, reference: &ConsumedRef) -> ConsumedSummary {
        let vendored = self.consumed(reference);
        ConsumedSummary {
            id: reference.0.clone(),
            title: vendored.map(|v| v.requirement.title.clone()),
            statement: vendored.map(|v| v.requirement.statement.clone()),
            provider_status: vendored.and_then(|v| v.provider_status),
            resolved: vendored.is_some(),
        }
    }

    /// Every `<alias>:<ID>` some local requirement consumes, with its consumers.
    pub fn consumed_interfaces(&self) -> Vec<(ConsumedSummary, Vec<RequirementId>)> {
        let mut by_ref: BTreeMap<&ConsumedRef, Vec<RequirementId>> = BTreeMap::new();
        for (id, req) in &self.requirements {
            for c in &req.trace.consumes {
                by_ref.entry(c).or_default().push(id.clone());
            }
        }
        by_ref
            .into_iter()
            .map(|(c, consumers)| (self.consumed_summary(c), consumers))
            .collect()
    }

    fn need_summary(&self, id: &NeedId) -> Summary {
        let need = self.needs.get(id);
        Summary {
            id: id.0.clone(),
            title: need.map(|n| n.title.clone()).unwrap_or_default(),
            statement: need.map(|n| n.statement.clone()),
        }
    }

    /// Everything relevant to one item, or `None` if no item has that ID.
    /// `diagnostics` should be the lint findings for the whole set; the ones about this item
    /// are included.
    pub fn context(
        &self,
        id: &str,
        links: &[SourceLink],
        evidence: &Evidence,
        verification: &BTreeMap<RequirementId, RequirementVerification>,
        diagnostics: &[Diagnostic],
    ) -> Option<Context> {
        let about = |subject: EntityRef| -> Vec<Diagnostic> {
            diagnostics
                .iter()
                .filter(|d| d.subject.as_ref() == Some(&subject))
                .cloned()
                .collect()
        };

        let req_id = RequirementId(id.to_owned());
        if let Some(req) = self.requirements.get(&req_id) {
            let view = self.trace_view(&req_id).ok()?;
            let children = self
                .requirements
                .iter()
                .filter(|(_, r)| r.trace.parents.contains(&req_id))
                .map(|(child, _)| self.summary(child))
                .collect();

            let mut trace_links = Vec::new();
            for (field, target) in req.trace.requirement_links() {
                if field != "parents" {
                    trace_links.push(TraceLink {
                        field: field.to_owned(),
                        direction: Direction::Outgoing,
                        id: target.0.clone(),
                        title: self.requirements.get(target).map(|r| r.title.clone()),
                    });
                }
            }
            for (other_id, other) in &self.requirements {
                for (field, target) in other.trace.requirement_links() {
                    if field != "parents" && *target == req_id {
                        trace_links.push(TraceLink {
                            field: field.to_owned(),
                            direction: Direction::Incoming,
                            id: other_id.0.clone(),
                            title: Some(other.title.clone()),
                        });
                    }
                }
            }

            let activities: BTreeSet<&str> = req
                .verification
                .activities
                .iter()
                .map(|a| a.id.as_str())
                .collect();
            return Some(Context::Requirement(Box::new(RequirementContext {
                path: self.relative(&self.files_by_id[&req_id]),
                requirement: req.clone(),
                ancestors: view.upward.iter().map(|p| self.summary(p)).collect(),
                children,
                links: trace_links,
                needs: req
                    .trace
                    .satisfies
                    .iter()
                    .map(|n| self.need_summary(n))
                    .collect(),
                consumes: req
                    .trace
                    .consumes
                    .iter()
                    .map(|c| self.consumed_summary(c))
                    .collect(),
                verification: verification.get(&req_id).cloned(),
                tests: links
                    .iter()
                    .filter(|l| activities.contains(l.activity.as_str()))
                    .cloned()
                    .collect(),
                evidence: evidence
                    .activities
                    .values()
                    .filter(|e| activities.contains(e.id.as_str()))
                    .cloned()
                    .collect(),
                diagnostics: about(EntityRef::Requirement(req_id.clone())),
            })));
        }

        if let Some(v) = self.consumed(&ConsumedRef(id.to_owned())) {
            return Some(Context::Interface(Box::new(InterfaceContext {
                id: id.to_owned(),
                alias: v.alias.clone(),
                path: self.relative(&v.path),
                requirement: v.requirement.clone(),
                provider_status: v.provider_status,
                consumers: self
                    .requirements
                    .iter()
                    .filter(|(_, r)| r.trace.consumes.iter().any(|c| c.0 == id))
                    .map(|(rid, _)| self.summary(rid))
                    .collect(),
            })));
        }

        let need_id = NeedId(id.to_owned());
        if let Some(need) = self.needs.get(&need_id) {
            return Some(Context::Need(NeedContext {
                path: self.relative(&self.needs_by_id[&need_id]),
                need: need.clone(),
                stakeholders: need
                    .stakeholders
                    .iter()
                    .filter_map(|s| self.stakeholders.get(s))
                    .cloned()
                    .collect(),
                satisfied_by: self
                    .requirements
                    .iter()
                    .filter(|(_, r)| r.trace.satisfies.contains(&need_id))
                    .map(|(rid, _)| self.summary(rid))
                    .collect(),
                diagnostics: about(EntityRef::Need(need_id.clone())),
            }));
        }

        let stk_id = StakeholderId(id.to_owned());
        let stakeholder = self.stakeholders.get(&stk_id)?;
        Some(Context::Stakeholder(StakeholderContext {
            path: self.relative(&self.stakeholders_by_id[&stk_id]),
            stakeholder: stakeholder.clone(),
            needs: self
                .needs
                .iter()
                .filter(|(_, n)| n.stakeholders.contains(&stk_id))
                .map(|(nid, _)| self.need_summary(nid))
                .collect(),
            diagnostics: about(EntityRef::Stakeholder(stk_id.clone())),
        }))
    }

    /// Compare requirements and needs with their state at git revision `base`, and report
    /// what the difference puts at risk. `changed_files` are repository-relative paths that
    /// differ from `base` (see [`crate::git::GitContext::changed_files_since`]).
    pub fn impact(
        &self,
        base: &str,
        links: &[SourceLink],
        evidence: &Evidence,
        changed_files: &[PathBuf],
    ) -> Result<Impact, RqtkError> {
        // A requirement whose file is byte-for-byte the same at `base` is unchanged, unless an
        // interface's spec files changed: only the other files are parsed.
        let base_blobs = self.git.blobs_at(base, &self.root)?;
        // Paths as git has them: relative to the work tree, computed from one canonical prefix.
        let prefix = self.git.relative(&self.root);
        let in_git = |path: &Path| prefix.join(path.strip_prefix(&self.root).unwrap_or(path));
        let unchanged: BTreeSet<&RequirementId> = self
            .files_by_id
            .iter()
            .filter(|(id, path)| {
                self.requirements[*id].interface.is_none()
                    && self
                        .blob_ids
                        .get(*path)
                        .is_some_and(|oid| base_blobs.get(&in_git(path)) == Some(oid))
            })
            .map(|(id, _)| id)
            .collect();
        let unchanged_paths: BTreeSet<PathBuf> = unchanged
            .iter()
            .map(|id| in_git(&self.files_by_id[*id]))
            .collect();
        let base_reqs: BTreeMap<RequirementId, Requirement> = base_blobs
            .iter()
            .filter(|(path, _)| {
                path.extension().is_some_and(|e| e == "toml") && !unchanged_paths.contains(*path)
            })
            .filter_map(|(_, oid)| self.git.parse_blob::<Requirement>(*oid))
            .map(|r| (r.id.clone(), r))
            .collect();
        let current_reqs: BTreeMap<RequirementId, Requirement> = self
            .requirements
            .iter()
            .filter(|(id, _)| !unchanged.contains(id))
            .map(|(id, r)| (id.clone(), r.clone()))
            .collect();
        let base_needs: BTreeMap<NeedId, Need> = self
            .git
            .items_at_ref::<Need>(base, &self.needs_root)?
            .into_iter()
            .map(|n| (n.id.clone(), n))
            .collect();

        // Spec files at `base` are read only when an interface requirement is compared.
        let base_files = if base_reqs.values().any(|r| r.interface.is_some()) {
            Some(self.git.spec_files_at(base, &self.repo_root)?)
        } else {
            None
        };
        let requirements = compare(
            &base_reqs,
            &current_reqs,
            |_, r| match &base_files {
                Some(files) => crate::git::hash_in(r, files),
                None => r.compute_content_hash(None),
            },
            |id, _| self.content_hash(id),
        );
        let needs = compare(
            &base_needs,
            &self.needs,
            |_, n| n.compute_content_hash(),
            |_, n| n.compute_content_hash(),
        );
        let interfaces = self.interface_changes(base)?;

        // Walk from each semantically changed or removed item to everything that depends on it.
        let mut dependents: BTreeMap<String, Vec<&RequirementId>> = BTreeMap::new();
        for (id, req) in &self.requirements {
            let t = &req.trace;
            let upstream = t
                .parents
                .iter()
                .chain(&t.depends_on)
                .chain(&t.derived_from)
                .chain(&t.refines)
                .map(|r| r.0.clone())
                .chain(t.satisfies.iter().map(|n| n.0.clone()));
            for up in upstream.chain(t.consumes.iter().map(|c| c.0.clone())) {
                dependents.entry(up).or_default().push(id);
            }
        }
        let changed_ids: BTreeSet<&str> = requirements
            .iter()
            .chain(&needs)
            .map(|c| c.id.as_str())
            .collect();
        let mut via: BTreeMap<&RequirementId, BTreeSet<String>> = BTreeMap::new();
        for change in requirements.iter().chain(&needs).chain(&interfaces) {
            if !matches!(change.change, ChangeKind::Semantic | ChangeKind::Removed) {
                continue;
            }
            let mut queue = VecDeque::from([change.id.clone()]);
            let mut seen = BTreeSet::from([change.id.clone()]);
            while let Some(current) = queue.pop_front() {
                for &dep in dependents.get(&current).into_iter().flatten() {
                    if seen.insert(dep.0.clone()) {
                        queue.push_back(dep.0.clone());
                        if !changed_ids.contains(dep.0.as_str()) {
                            via.entry(dep).or_default().insert(change.id.clone());
                        }
                    }
                }
            }
        }
        let downstream = via
            .into_iter()
            .map(|(id, via)| Downstream {
                id: id.0.clone(),
                via: via.into_iter().collect(),
            })
            .collect();

        let changed_reqs: BTreeSet<&str> = requirements
            .iter()
            .filter(|c| matches!(c.change, ChangeKind::Semantic | ChangeKind::Added))
            .map(|c| c.id.as_str())
            .collect();
        let changed_files: BTreeSet<&Path> = changed_files.iter().map(PathBuf::as_path).collect();
        let mut links_of: BTreeMap<&str, Vec<&SourceLink>> = BTreeMap::new();
        for l in links {
            links_of.entry(l.activity.as_str()).or_default().push(l);
        }
        let mut reverify = Vec::new();
        for (req_id, req) in &self.requirements {
            for activity in &req.verification.activities {
                let linked: Vec<&SourceLink> = links_of
                    .get(activity.id.as_str())
                    .cloned()
                    .unwrap_or_default();
                let mut reasons = Vec::new();
                if changed_reqs.contains(req_id.0.as_str()) {
                    reasons.push(ReverifyReason::RequirementChanged);
                }
                let test_files: BTreeSet<&Path> = linked
                    .iter()
                    .map(|l| l.path.as_path())
                    .filter(|p| changed_files.contains(p))
                    .collect();
                reasons.extend(test_files.into_iter().map(|p| ReverifyReason::TestChanged {
                    path: p.to_path_buf(),
                }));
                if !reasons.is_empty() {
                    let tests = linked
                        .iter()
                        .map(|l| {
                            format!(
                                "{}:{} {}",
                                l.path.display(),
                                l.line,
                                l.test_name.as_deref().unwrap_or("?")
                            )
                        })
                        .collect();
                    let current_tests = crate::evidence::combined_hash(&linked);
                    let done = evidence.activities.get(&activity.id).is_some_and(|e| {
                        e.requirement == *req_id
                            && e.outcome == crate::evidence::Outcome::Passed
                            && e.requirement_hash == self.content_hash(req_id)
                            && !e.unchanged_tests
                            && (e.tests_hash.is_none() || e.tests_hash == current_tests)
                    });
                    reverify.push(Reverify {
                        activity: activity.id.clone(),
                        requirement: req_id.clone(),
                        reasons,
                        tests,
                        done,
                    });
                }
            }
        }

        Ok(Impact {
            base: base.to_owned(),
            requirements,
            needs,
            interfaces,
            downstream,
            reverify,
        })
    }
}

impl RequirementSet<Validated> {
    /// Requirements added, removed and modified between revisions `from` and `to`.
    pub fn diff(&self, from: &str, to: &str) -> Result<crate::git::RequirementDiff, RqtkError> {
        // Blobs loaded here need no second read to tell whether they are interfaces.
        let known = self
            .files_by_id
            .iter()
            .filter_map(|(id, path)| {
                let oid = self.blob_ids.get(path)?;
                Some((*oid, self.requirements[id].interface.is_some()))
            })
            .collect();
        self.git
            .diff_refs(from, to, &self.root, &self.repo_root, &known)
    }

    /// Vendored interface requirements whose hash in the lockfile differs from `base`.
    fn interface_changes(&self, base: &str) -> Result<Vec<ItemChange>, RqtkError> {
        let hashes = |lock: Option<&crate::vendor::Lockfile>| -> BTreeMap<String, String> {
            lock.into_iter()
                .flat_map(|l| &l.sources)
                .flat_map(|(alias, s)| {
                    s.requirements
                        .iter()
                        .map(move |(id, h)| (format!("{alias}:{id}"), h.clone()))
                })
                .collect()
        };
        let base_lock = match self
            .git
            .read_at(base, &self.repo_root, crate::vendor::LOCK_PATH)?
        {
            Some(bytes) => Some(
                crate::vendor::Lockfile::parse(&String::from_utf8_lossy(&bytes))
                    .map_err(|e| crate::vendor::Lockfile::unreadable(&format!("at {base}: {e}")))?,
            ),
            None => None,
        };
        let before = hashes(base_lock.as_ref());
        let after = hashes(self.vendored.lock.as_ref());
        let mut changes: Vec<ItemChange> = after
            .iter()
            .filter_map(|(id, h)| {
                let change = match before.get(id) {
                    None => ChangeKind::Added,
                    Some(old) if old != h => ChangeKind::Semantic,
                    Some(_) => return None,
                };
                Some(ItemChange {
                    id: id.clone(),
                    change,
                })
            })
            .collect();
        changes.extend(
            before
                .keys()
                .filter(|id| !after.contains_key(*id))
                .map(|id| ItemChange {
                    id: id.clone(),
                    change: ChangeKind::Removed,
                }),
        );
        changes.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(changes)
    }
}

fn compare<K: Ord + AsRef<str>, T>(
    base: &BTreeMap<K, T>,
    current: &BTreeMap<K, T>,
    base_hash: impl Fn(&K, &T) -> String,
    current_hash: impl Fn(&K, &T) -> String,
) -> Vec<ItemChange>
where
    T: Serialize,
{
    let mut changes = Vec::new();
    for (id, item) in current {
        let change = match base.get(id) {
            None => Some(ChangeKind::Added),
            Some(old) if base_hash(id, old) != current_hash(id, item) => Some(ChangeKind::Semantic),
            Some(old) if toml::to_string(old).ok() != toml::to_string(item).ok() => {
                Some(ChangeKind::Cosmetic)
            }
            Some(_) => None,
        };
        if let Some(change) = change {
            changes.push(ItemChange {
                id: id.as_ref().to_owned(),
                change,
            });
        }
    }
    for id in base.keys().filter(|id| !current.contains_key(*id)) {
        changes.push(ItemChange {
            id: id.as_ref().to_owned(),
            change: ChangeKind::Removed,
        });
    }
    changes.sort_by(|a, b| a.id.cmp(&b.id));
    changes
}
