use std::{error::Error, path::PathBuf, process::Command};

use chrono::Utc;
use rqtk_core::{BaselineName, RequirementSet};
use semver::Version;
use serde::Serialize;

use crate::output::{self, Ctx, Exit, Usage};

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
    let by = set.git.committer_name()?;
    let tag = format!("rqtk/{name}");

    let stamped = if dry_run {
        set.files_by_id.values().cloned().collect()
    } else {
        let stamped = set.stamp_baseline(&by, Utc::now().date_naive())?;
        if !stamped.is_empty() {
            git(ctx, Command::new("git").arg("add").args(&stamped))?;
            let msg = format!("baseline: stamp requirements at {name}");
            git(ctx, Command::new("git").args(["commit", "-m", &msg]))?;
        }
        let message = format!(
            "Requirements baseline {name}\n\nProject: {}",
            set.config.project.name
        );
        set.git.create_baseline_tag(&name, &message)?;
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
        output::json(&report)?;
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
