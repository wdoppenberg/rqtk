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
    /// Each activity's ID and state, in file order.
    pub activities: Vec<(String, ActivityState)>,
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
                let activities: Vec<(String, ActivityState)> = req
                    .verification
                    .activities
                    .iter()
                    .map(|a| {
                        let state = activity_state(req, a, &hash, &linked, evidence);
                        (a.id.clone(), state)
                    })
                    .collect();
                let status = closure_status(req, &activities);
                (id.clone(), RequirementVerification { status, activities })
            })
            .collect()
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
                        "`verifies(\"{}\")` is not followed by a function, so no test result can be matched to it",
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
