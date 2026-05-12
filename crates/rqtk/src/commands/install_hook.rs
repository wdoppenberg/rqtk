use std::{error::Error, path::Path};

use crate::output;

const HOOK_SCRIPT: &str = r#"#!/bin/sh
# rqtk pre-commit hook — installed by `rqtk install-hook`
set -e
rqtk rehash
rqtk lint
"#;

pub fn run(repo_root: &Path, force: bool) -> Result<(), Box<dyn Error>> {
    let git_dir = find_git_dir(repo_root)?;
    let hooks_dir = git_dir.join("hooks");
    std::fs::create_dir_all(&hooks_dir)?;

    let hook_path = hooks_dir.join("pre-commit");
    if hook_path.exists() && !force {
        return Err(format!(
            "pre-commit hook already exists at {}; pass --force to overwrite it",
            hook_path.display()
        )
        .into());
    }

    std::fs::write(&hook_path, HOOK_SCRIPT)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&hook_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&hook_path, perms)?;
    }

    output::success(
        "Pre-commit hook installed",
        &[("hook", &hook_path.display().to_string())],
    );
    Ok(())
}

fn find_git_dir(start: &Path) -> Result<std::path::PathBuf, Box<dyn Error>> {
    let mut dir = start.canonicalize()?;
    loop {
        let candidate = dir.join(".git");
        if candidate.exists() {
            return Ok(candidate);
        }
        if !dir.pop() {
            break;
        }
    }
    Err(format!("no .git directory found from {}", start.display()).into())
}
