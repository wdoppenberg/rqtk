use std::{error::Error, path::PathBuf, process::Command};

use chrono::Utc;
use rqtk_core::{BaselineName, EntityRef, RequirementSet};
use semver::Version;
use serde::Serialize;

use crate::cli::output::{self, Ctx, Exit, Usage};

#[derive(Serialize)]
struct Report {
    tag: String,
    by: String,
    stamped: Vec<PathBuf>,
    dry_run: bool,
}

pub fn run(ctx: &Ctx, version: String, dry_run: bool) -> Result<Exit, Box<dyn Error>> {
    let parsed = Version::parse(&version)
        .map_err(|e| Usage(format!("baseline version `{version}` is not semver: {e}")))?;
    let name: BaselineName = parsed.to_string().parse()?;
    let mut set = RequirementSet::load_from_repo_root(&ctx.root)?;
    let by = set.git().committer_name()?;
    let tag = format!("rqtk/{name}");

    let stamped = if dry_run {
        set.requirements()
            .keys()
            .filter_map(|id| set.path_of(&EntityRef::Requirement(id.clone())))
            .map(|p| p.to_path_buf())
            .collect()
    } else {
        let stamped = set.stamp_baseline(&by, Utc::now().date_naive())?;
        if !stamped.is_empty() {
            git(ctx, Command::new("git").arg("add").args(&stamped))?;
            let msg = format!("baseline: stamp requirements at {name}");
            git(ctx, Command::new("git").args(["commit", "-m", &msg]))?;
        }
        let message = format!(
            "Requirements baseline {name}\n\nProject: {}",
            set.config().project.name
        );
        set.git().create_baseline_tag(&name, &message)?;
        stamped
    };

    let report = Report {
        tag,
        by,
        stamped: stamped
            .iter()
            .map(|p| output::relative(p, &ctx.root))
            .collect(),
        dry_run,
    };
    if ctx.json() {
        output::document("baseline", &report)?;
    } else {
        let label = if dry_run {
            "Baseline would be created"
        } else {
            "Baseline created"
        };
        output::success(
            label,
            &[
                ("tag", report.tag.as_str()),
                ("stamped", &report.stamped.len().to_string()),
                ("by", &report.by),
            ],
        );
    }
    Ok(Exit::Ok)
}

/// `rqtk baseline --list`: every baseline, oldest first.
pub fn list(ctx: &Ctx) -> Result<Exit, Box<dyn Error>> {
    let repo = super::open(ctx)?;
    let doc = repo.baselines()?;
    if ctx.json() {
        output::json(&doc)?;
        return Ok(Exit::Ok);
    }
    if doc.baselines.is_empty() {
        output::success(
            "No baselines yet",
            &[("create one", "rqtk baseline <version>")],
        );
        return Ok(Exit::Ok);
    }
    output::section("Baselines", &format!("{} tags", doc.baselines.len()));
    for b in &doc.baselines {
        let date = b
            .timestamp
            .as_deref()
            .and_then(|t| t.get(..10))
            .unwrap_or("unknown");
        output::item(&format!(
            "{}  {date}  {}",
            b.tag,
            b.message.lines().next().unwrap_or("")
        ));
    }
    println!();
    Ok(Exit::Ok)
}

fn git(ctx: &Ctx, command: &mut Command) -> Result<(), Box<dyn Error>> {
    // Keep git's own chatter off stdout so `--json` output stays parseable.
    let out = command.current_dir(&ctx.root).output()?;
    if !out.status.success() {
        return Err(format!(
            "git failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )
        .into());
    }
    Ok(())
}
