//! Verification status derived from source links and recorded evidence.

use crate::diagnostic::Diagnostic;
use crate::evidence::{Evidence, Outcome};
use crate::model::{EntityRef, Requirement, RequirementId, VerificationActivity};
use crate::repository::{RequirementSet, Validated};
use crate::scan::SourceLink;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// Status of a single verification activity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", content = "status", rename_all = "snake_case")]
pub enum ActivityState {
    /// Linked tests passed against the requirement's current content.
    Passed,
    /// A linked test failed in the recorded evidence.
    Failed,
    /// Linked tests passed, but the requirement has changed since.
    Suspect,
    /// Linked to tests, but no evidence has been recorded yet.
    NotRun,
    /// Not linked to any test; the activity's hand-written `status` applies. Used for
    /// inspections, analyses and demonstrations.
    Manual(Option<String>),
}

impl ActivityState {
    fn is_terminal(&self) -> bool {
        match self {
            ActivityState::Passed => true,
            ActivityState::Manual(Some(s)) => s == "Passed" || s == "Waived",
            _ => false,
        }
    }

    fn is_failed(&self) -> bool {
        match self {
            ActivityState::Failed => true,
            ActivityState::Manual(Some(s)) => s == "Failed",
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClosureStatus {
    /// Every activity passed (tests against current content, or manual Passed/Waived).
    Verified,
    /// A test passed for an earlier version of the requirement; re-verify.
    Suspect,
    /// At least one activity failed.
    Failed,
    /// Some activities passed or started, not all.
    InProgress,
    /// Activities and success criteria defined, none executed yet.
    Planned,
    /// No activities defined, or success criteria missing.
    Gap,
}

#[derive(Debug, Clone, Serialize)]
pub struct RequirementVerification {
    pub status: ClosureStatus,
    /// Each activity's ID and state, in file order. Serialised as `[id, state]` pairs;
    /// `activity_states` has the same as objects.
    pub activities: Vec<(String, ActivityState)>,
    /// Each activity's ID and state as `{"id": …, "state": …}`, in file order.
    pub activity_states: Vec<ActivityStatus>,
    /// Why the requirement is Suspect, when it is.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub suspect_reasons: Vec<SuspectReason>,
}

/// One activity's state, as reported in `activity_states`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ActivityStatus {
    pub id: String,
    #[serde(flatten)]
    pub state: ActivityState,
}

/// Why recorded evidence no longer settles a requirement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum SuspectReason {
    /// The requirement changed after these activities' tests passed. Rerun them.
    RequirementChanged { activities: Vec<String> },
    /// The requirement changed, and the same tests that passed for its earlier wording passed
    /// again. Update the tests, or confirm they still prove it with `rqtk review`.
    TestsUnchanged { activities: Vec<String> },
    /// An ancestor requirement, or a need one of them satisfies, changed since this
    /// requirement was verified. Check it still fits, then `rqtk review` it.
    UpstreamChanged { items: Vec<String> },
}

impl RequirementSet<Validated> {
    /// Verification status of every requirement, given the `verifies` links found in source
    /// and the recorded evidence.
    pub fn verification_status(
        &self,
        links: &[SourceLink],
        evidence: &Evidence,
    ) -> BTreeMap<RequirementId, RequirementVerification> {
        let linked: BTreeSet<&str> = links.iter().map(|l| l.activity.as_str()).collect();
        self.requirements
            .iter()
            .map(|(id, req)| {
                let hash = req.compute_content_hash();
                let review = evidence
                    .reviews
                    .get(id)
                    .filter(|r| r.requirement_hash == hash);
                let mut changed = Vec::new();
                let mut unchanged_tests = Vec::new();
                let activities: Vec<(String, ActivityState)> = req
                    .verification
                    .activities
                    .iter()
                    .map(|a| {
                        let mut state = activity_state(req, a, &hash, &linked, evidence);
                        let entry = evidence.activities.get(&a.id);
                        if state == ActivityState::Suspect {
                            changed.push(a.id.clone());
                        } else if state == ActivityState::Passed
                            && entry.is_some_and(|e| e.unchanged_tests)
                            && review.is_none()
                        {
                            state = ActivityState::Suspect;
                            unchanged_tests.push(a.id.clone());
                        }
                        (a.id.clone(), state)
                    })
                    .collect();

                let mut suspect_reasons = Vec::new();
                if !changed.is_empty() {
                    suspect_reasons.push(SuspectReason::RequirementChanged {
                        activities: changed,
                    });
                }
                if !unchanged_tests.is_empty() {
                    suspect_reasons.push(SuspectReason::TestsUnchanged {
                        activities: unchanged_tests,
                    });
                }
                let upstream = self.upstream_changes(id, req, &hash, evidence);
                if !upstream.is_empty() {
                    suspect_reasons.push(SuspectReason::UpstreamChanged { items: upstream });
                }

                let mut status = closure_status(req, &activities);
                if !suspect_reasons.is_empty() && status != ClosureStatus::Failed {
                    status = ClosureStatus::Suspect;
                }
                let activity_states = activities
                    .iter()
                    .map(|(id, state)| ActivityStatus {
                        id: id.clone(),
                        state: state.clone(),
                    })
                    .collect();
                let verification = RequirementVerification {
                    status,
                    activities,
                    activity_states,
                    suspect_reasons,
                };
                (id.clone(), verification)
            })
            .collect()
    }

    /// Content hashes of everything a requirement depends on: its ancestors (transitively)
    /// and the needs it and its ancestors satisfy, keyed by ID.
    pub fn upstream_hashes(&self, id: &RequirementId) -> BTreeMap<String, String> {
        let mut out = BTreeMap::new();
        let mut stack = vec![id.clone()];
        let mut seen = BTreeSet::new();
        while let Some(current) = stack.pop() {
            if !seen.insert(current.clone()) {
                continue;
            }
            let Some(req) = self.requirements.get(&current) else {
                continue;
            };
            if current != *id {
                out.insert(current.to_string(), req.compute_content_hash());
            }
            for need in &req.trace.satisfies {
                if let Some(n) = self.needs.get(need) {
                    out.insert(need.to_string(), n.compute_content_hash());
                }
            }
            stack.extend(req.trace.parents.iter().cloned());
        }
        out
    }

    /// Upstream items that changed since the requirement's current wording was verified or
    /// last reviewed.
    fn upstream_changes(
        &self,
        id: &RequirementId,
        req: &Requirement,
        hash: &str,
        evidence: &Evidence,
    ) -> Vec<String> {
        let baseline: BTreeMap<&String, &String> = match evidence
            .reviews
            .get(id)
            .filter(|r| r.requirement_hash == hash)
        {
            Some(review) => review.upstream.iter().collect(),
            None => req
                .verification
                .activities
                .iter()
                .filter_map(|a| evidence.activities.get(&a.id))
                .filter(|e| e.requirement == *id && e.requirement_hash == hash)
                .flat_map(|e| e.upstream.iter())
                .collect(),
        };
        if baseline.is_empty() {
            return Vec::new();
        }
        let current = self.upstream_hashes(id);
        let mut changed: Vec<String> = baseline
            .into_iter()
            .filter(|(item, recorded)| current.get(*item).is_some_and(|now| now != *recorded))
            .map(|(item, _)| item.clone())
            .collect();
        changed.sort();
        changed.dedup();
        changed
    }

    /// Cross-check source links against the requirement set:
    /// RQ028 for links to unknown activities, RQ029 for hand-written status on activities
    /// whose status comes from tests, RQ030 for annotations not attached to a function.
    pub fn link_diagnostics(&self, links: &[SourceLink]) -> Vec<Diagnostic> {
        let mut owners: BTreeMap<&str, (&RequirementId, &VerificationActivity)> = BTreeMap::new();
        for (id, req) in &self.requirements {
            for a in &req.verification.activities {
                owners.entry(&a.id).or_insert((id, a));
            }
        }

        let mut issues = Vec::new();
        let mut linked = BTreeSet::new();
        for link in links {
            let at = |d: Diagnostic| d.at(self.repo_root.join(&link.path), link.line, 1);
            if !owners.contains_key(link.activity.as_str()) {
                issues.push(at(Diagnostic::new(
                    "RQ028",
                    format!(
                        "`verifies` names unknown verification activity `{}`",
                        link.activity
                    ),
                )));
                continue;
            }
            linked.insert(link.activity.as_str());
            if link.test_name.is_none() {
                issues.push(at(Diagnostic::new(
                    "RQ030",
                    format!(
                        "`verifies(\"{}\")` is not followed by a test declaration rqtk recognises, so no test result can be matched to it",
                        link.activity
                    ),
                )));
            }
        }

        for activity in linked {
            let (req_id, a) = owners[activity];
            if a.status.is_some() {
                issues.push(
                    Diagnostic::new(
                        "RQ029",
                        format!(
                            "activity `{activity}` is linked to tests, so its status comes from `rqtk verify`; remove the hand-written `status`"
                        ),
                    )
                    .subject(EntityRef::Requirement(req_id.clone()))
                    .file(&self.files_by_id[req_id])
                    .field("verification.activities"),
                );
            }
        }
        issues
    }
}

fn activity_state(
    req: &Requirement,
    activity: &VerificationActivity,
    current_hash: &str,
    linked: &BTreeSet<&str>,
    evidence: &Evidence,
) -> ActivityState {
    if !linked.contains(activity.id.as_str()) {
        return ActivityState::Manual(activity.status.clone().filter(|s| !s.trim().is_empty()));
    }
    match evidence.activities.get(&activity.id) {
        None => ActivityState::NotRun,
        Some(e) if e.outcome == Outcome::Failed => ActivityState::Failed,
        Some(e) if e.requirement != req.id || e.requirement_hash != current_hash => {
            ActivityState::Suspect
        }
        Some(_) => ActivityState::Passed,
    }
}

fn closure_status(req: &Requirement, activities: &[(String, ActivityState)]) -> ClosureStatus {
    let criteria = req.verification.success_criteria.as_deref().unwrap_or("");
    if activities.is_empty() || criteria.trim().is_empty() {
        return ClosureStatus::Gap;
    }
    let states = || activities.iter().map(|(_, s)| s);
    if states().any(ActivityState::is_failed) {
        return ClosureStatus::Failed;
    }
    if states().any(|s| *s == ActivityState::Suspect) {
        return ClosureStatus::Suspect;
    }
    if states().all(ActivityState::is_terminal) {
        return ClosureStatus::Verified;
    }
    let executed = req
        .verification
        .activities
        .iter()
        .any(|a| a.executed_at.is_some());
    let started =
        states().any(|s| matches!(s, ActivityState::Passed | ActivityState::Manual(Some(_))));
    if started || executed {
        ClosureStatus::InProgress
    } else {
        ClosureStatus::Planned
    }
}
