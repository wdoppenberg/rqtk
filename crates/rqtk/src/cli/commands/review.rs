use std::error::Error;

use rqtk_core::evidence::EVIDENCE_PATH;
use rqtk_core::{RequirementId, RequirementSet, Review, SuspectReason};
use serde::Serialize;

use crate::cli::output::{self, Ctx, Exit, Usage};

pub struct ReviewArgs {
    pub id: String,
    pub note: Option<String>,
    pub dry_run: bool,
}

#[derive(Serialize)]
struct Report<'a> {
    review: &'a Review,
    /// The Suspect reasons this review settles.
    cleared: Vec<SuspectReason>,
    written: bool,
}

/// Record that a requirement still holds after something it depends on changed: its tests
/// were kept as they are after it was reworded, or an ancestor or need changed. Clears the
/// `tests_unchanged` and `upstream_changed` Suspect reasons until the requirement or its
/// upstream changes again. A requirement whose own tests haven't been rerun stays Suspect.
pub fn run(ctx: &Ctx, args: ReviewArgs) -> Result<Exit, Box<dyn Error>> {
    let (set, _) = RequirementSet::load_from_repo_root(&ctx.root)?.validate();
    let id = RequirementId(args.id.clone());
    if !set.requirements().contains_key(&id) {
        return Err(Usage(format!("no requirement with ID `{}`", args.id)).into());
    }
    let (links, mut evidence) = super::load_links_and_evidence(&set)?;
    let before = set.verification_status(&links, &evidence);
    let cleared: Vec<SuspectReason> = before[&id]
        .suspect_reasons
        .iter()
        .filter(|r| !matches!(r, SuspectReason::RequirementChanged { .. }))
        .cloned()
        .collect();

    let date = chrono::Local::now().date_naive().to_string();
    let commit = set.git().head_commit().map(|c| c.full().to_owned());
    let review = evidence
        .review(&set, &id, date, commit.as_deref(), args.note)
        .expect("requirement exists");
    if !args.dry_run {
        evidence.save(set.repo_root(), env!("CARGO_PKG_VERSION"))?;
    }
    let after = set.verification_status(&links, &evidence);
    let still_suspect = after[&id]
        .suspect_reasons
        .iter()
        .any(|r| matches!(r, SuspectReason::RequirementChanged { .. }));

    if ctx.json() {
        output::json(&Report {
            review: &review,
            cleared,
            written: !args.dry_run,
        })?;
        return Ok(Exit::Ok);
    }

    if cleared.is_empty() {
        output::section(
            "Nothing to settle",
            "(no unchanged-test or upstream finding; review recorded anyway)",
        );
    } else {
        output::section("Settled", "");
        for reason in &cleared {
            output::item(&describe(reason));
        }
    }
    if still_suspect {
        output::section(
            "Still Suspect",
            "(the requirement changed since its tests ran; rerun them and `rqtk verify`)",
        );
    }
    println!();
    let label = if args.dry_run {
        "Dry run: review not written"
    } else {
        "Review recorded"
    };
    output::success(label, &[("requirement", &id.0), ("file", EVIDENCE_PATH)]);
    Ok(Exit::Ok)
}

pub fn describe(reason: &SuspectReason) -> String {
    match reason {
        SuspectReason::RequirementChanged { activities } => {
            format!("changed since its tests passed: {}", activities.join(", "))
        }
        SuspectReason::TestsUnchanged { activities } => format!(
            "re-verified by the same tests after it changed: {}",
            activities.join(", ")
        ),
        SuspectReason::UpstreamChanged { items } => {
            format!("changed upstream: {}", items.join(", "))
        }
    }
}
