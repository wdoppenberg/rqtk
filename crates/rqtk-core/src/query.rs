//! Read-only views over a validated requirement set, shaped for agents and reviewers:
//! everything relevant to one item ([`RequirementSet::context`]) and what a change touches
//! ([`RequirementSet::impact`]).

use crate::diagnostic::Diagnostic;
use crate::error::RqtkError;
use crate::evidence::{ActivityEvidence, Evidence};
use crate::model::{
    EntityRef, Need, NeedId, Requirement, RequirementId, Stakeholder, StakeholderId,
};
use crate::repository::{RequirementSet, Validated};
use crate::scan::SourceLink;
use crate::status::RequirementVerification;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};

/// A one-line reference to another item.
#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    pub id: String,
    pub title: String,
    pub statement: Option<String>,
}

/// A link from or to the item in context, labelled with the `trace` field it comes from.
#[derive(Debug, Clone, Serialize)]
pub struct TraceLink {
    pub field: &'static str,
    /// `outgoing` if the item in context declares the link, `incoming` if the other one does.
    pub direction: &'static str,
    pub id: String,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Context<'a> {
    Requirement(Box<RequirementContext<'a>>),
    Need(NeedContext<'a>),
    Stakeholder(StakeholderContext<'a>),
}

#[derive(Debug, Clone, Serialize)]
pub struct RequirementContext<'a> {
    pub path: PathBuf,
    pub requirement: &'a Requirement,
    /// Ancestors via `trace.parents`, nearest first.
    pub ancestors: Vec<Summary>,
    /// Requirements that list this one in `trace.parents`.
    pub children: Vec<Summary>,
    /// Every other requirement link, in both directions.
    pub links: Vec<TraceLink>,
    pub needs: Vec<Summary>,
    pub verification: Option<RequirementVerification>,
    /// `verifies` annotations for this requirement's activities.
    pub tests: Vec<SourceLink>,
    pub evidence: Vec<&'a ActivityEvidence>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NeedContext<'a> {
    pub path: PathBuf,
    pub need: &'a Need,
    pub stakeholders: Vec<&'a Stakeholder>,
    pub satisfied_by: Vec<Summary>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StakeholderContext<'a> {
    pub path: PathBuf,
    pub stakeholder: &'a Stakeholder,
    pub needs: Vec<Summary>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeKind {
    Added,
    Removed,
    /// Content hash changed: what is demanded or how it is verified.
    Semantic,
    /// Other fields changed.
    Cosmetic,
}

#[derive(Debug, Clone, Serialize)]
pub struct ItemChange {
    pub id: String,
    pub change: ChangeKind,
}

/// A requirement that did not change itself but depends on one that did.
#[derive(Debug, Clone, Serialize)]
pub struct Downstream {
    pub id: String,
    /// The changed requirements or needs it is reached from.
    pub via: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum ReverifyReason {
    /// The requirement is new, or its content hash changed.
    RequirementChanged,
    /// A file containing a linked test changed.
    TestChanged { path: PathBuf },
}

#[derive(Debug, Clone, Serialize)]
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
#[derive(Debug, Clone, Serialize)]
pub struct Impact {
    pub base: String,
    pub requirements: Vec<ItemChange>,
    pub needs: Vec<ItemChange>,
    pub downstream: Vec<Downstream>,
    pub reverify: Vec<Reverify>,
}

impl Impact {
    pub fn is_empty(&self) -> bool {
        self.requirements.is_empty() && self.needs.is_empty() && self.reverify.is_empty()
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
    pub fn context<'a>(
        &'a self,
        id: &str,
        links: &[SourceLink],
        evidence: &'a Evidence,
        verification: &BTreeMap<RequirementId, RequirementVerification>,
        diagnostics: &[Diagnostic],
    ) -> Option<Context<'a>> {
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
                        field,
                        direction: "outgoing",
                        id: target.0.clone(),
                        title: self.requirements.get(target).map(|r| r.title.clone()),
                    });
                }
            }
            for (other_id, other) in &self.requirements {
                for (field, target) in other.trace.requirement_links() {
                    if field != "parents" && *target == req_id {
                        trace_links.push(TraceLink {
                            field,
                            direction: "incoming",
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
                requirement: req,
                ancestors: view.upward.iter().map(|p| self.summary(p)).collect(),
                children,
                links: trace_links,
                needs: req
                    .trace
                    .satisfies
                    .iter()
                    .map(|n| self.need_summary(n))
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
                    .collect(),
                diagnostics: about(EntityRef::Requirement(req_id.clone())),
            })));
        }

        let need_id = NeedId(id.to_owned());
        if let Some(need) = self.needs.get(&need_id) {
            return Some(Context::Need(NeedContext {
                path: self.relative(&self.needs_by_id[&need_id]),
                need,
                stakeholders: need
                    .stakeholders
                    .iter()
                    .filter_map(|s| self.stakeholders.get(s))
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
            stakeholder,
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
        let base_reqs: BTreeMap<RequirementId, Requirement> = self
            .git
            .items_at_ref::<Requirement>(base, &self.root)?
            .into_iter()
            .map(|r| (r.id.clone(), r))
            .collect();
        let base_needs: BTreeMap<NeedId, Need> = self
            .git
            .items_at_ref::<Need>(base, &self.needs_root)?
            .into_iter()
            .map(|n| (n.id.clone(), n))
            .collect();

        let requirements = compare(
            &base_reqs,
            &self.requirements,
            Requirement::compute_content_hash,
        );
        let needs = compare(&base_needs, &self.needs, Need::compute_content_hash);

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
            for up in upstream {
                dependents.entry(up).or_default().push(id);
            }
        }
        let changed_ids: BTreeSet<&str> = requirements
            .iter()
            .chain(&needs)
            .map(|c| c.id.as_str())
            .collect();
        let mut via: BTreeMap<&RequirementId, BTreeSet<String>> = BTreeMap::new();
        for change in requirements.iter().chain(&needs) {
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
        let mut reverify = Vec::new();
        for (req_id, req) in &self.requirements {
            for activity in &req.verification.activities {
                let mut reasons = Vec::new();
                if changed_reqs.contains(req_id.0.as_str()) {
                    reasons.push(ReverifyReason::RequirementChanged);
                }
                let test_files: BTreeSet<&Path> = links
                    .iter()
                    .filter(|l| l.activity == activity.id)
                    .map(|l| l.path.as_path())
                    .filter(|p| changed_files.contains(p))
                    .collect();
                reasons.extend(test_files.into_iter().map(|p| ReverifyReason::TestChanged {
                    path: p.to_path_buf(),
                }));
                if !reasons.is_empty() {
                    let linked: Vec<&SourceLink> =
                        links.iter().filter(|l| l.activity == activity.id).collect();
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
                            && e.requirement_hash == req.compute_content_hash()
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
            downstream,
            reverify,
        })
    }
}

fn compare<K: Ord + AsRef<str>, T>(
    base: &BTreeMap<K, T>,
    current: &BTreeMap<K, T>,
    hash: impl Fn(&T) -> String,
) -> Vec<ItemChange>
where
    T: Serialize,
{
    let mut changes = Vec::new();
    for (id, item) in current {
        let change = match base.get(id) {
            None => Some(ChangeKind::Added),
            Some(old) if hash(old) != hash(item) => Some(ChangeKind::Semantic),
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
