use std::{error::Error, path::Path};

use crate::cli::output::{self, Ctx, Exit};

const REGION_BEGIN: &str = "# BEGIN rqtk-managed";
const REGION_END: &str = "# END rqtk-managed";

const REGION_BODY: &str = "set -e\nrqtk rehash\nrqtk lint";

const DEFAULT_SHEBANG: &str = "#!/bin/sh";

#[cfg(test)]
fn managed_region() -> String {
    format!("{REGION_BEGIN}\n{REGION_BODY}\n{REGION_END}\n")
}

/// Insert or replace the rqtk-managed region in `existing`, returning the new content.
fn splice_region(existing: &str) -> String {
    super::splice_managed(existing, REGION_BEGIN, REGION_END, REGION_BODY)
}

pub fn run(ctx: &Ctx) -> Result<Exit, Box<dyn Error>> {
    let (hook_path, action) = install(&ctx.root, false)?;
    if ctx.json() {
        output::json(&serde_json::json!({ "hook": hook_path, "action": action }))?;
    } else {
        output::success(
            &format!("Pre-commit hook region {action}"),
            &[("hook", &hook_path.display().to_string())],
        );
    }
    Ok(Exit::Ok)
}

/// Add or refresh the rqtk region in the repository's pre-commit hook. Returns the hook's
/// path and whether it was "installed" or "updated".
pub fn install(
    root: &Path,
    dry_run: bool,
) -> Result<(std::path::PathBuf, &'static str), Box<dyn Error>> {
    let git_dir = find_git_dir(root)?;
    let hooks_dir = git_dir.join("hooks");
    let hook_path = hooks_dir.join("pre-commit");
    let existing = if hook_path.exists() {
        std::fs::read_to_string(&hook_path)?
    } else {
        format!("{DEFAULT_SHEBANG}\n")
    };
    let action = if existing.contains(REGION_BEGIN) {
        "updated"
    } else {
        "installed"
    };
    if dry_run {
        return Ok((hook_path, action));
    }
    std::fs::create_dir_all(&hooks_dir)?;
    std::fs::write(&hook_path, splice_region(&existing))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&hook_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&hook_path, perms)?;
    }
    Ok((hook_path, action))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_file_gets_region_appended() {
        let result = splice_region("#!/bin/sh\n");
        assert!(result.contains(REGION_BEGIN));
        assert!(result.contains(REGION_END));
        assert!(result.contains("rqtk lint"));
        assert!(result.starts_with("#!/bin/sh\n"));
    }

    #[test]
    fn existing_region_is_replaced_not_duplicated() {
        let initial = splice_region("#!/bin/sh\n");
        let second = splice_region(&initial);
        assert_eq!(second.matches(REGION_BEGIN).count(), 1);
    }

    #[test]
    fn existing_hook_content_is_preserved() {
        let existing = "#!/bin/sh\necho 'other hook'\n";
        let result = splice_region(existing);
        assert!(result.contains("echo 'other hook'"));
        assert!(result.contains(REGION_BEGIN));
    }

    #[test]
    fn empty_file_gets_just_region() {
        let result = splice_region("");
        assert_eq!(result, managed_region());
    }
}
