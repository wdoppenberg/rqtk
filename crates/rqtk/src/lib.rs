use rqtk_core::{LintSeverity, RequirementId, RequirementSet, RqtkError};
use semver::Version;
use std::fs;
use std::path::Path;

pub fn load_requirements(path: &Path) -> Result<RequirementSet, RqtkError> {
    RequirementSet::load_from_requirements_dir(path)
}

pub fn write_requirement_file(path: &Path, file: &rqtk_core::RequirementFile) -> Result<(), RqtkError> {
    let text = toml::to_string_pretty(file)?;
    fs::write(path, text).map_err(|source| RqtkError::Io {
        path: path.to_path_buf(),
        source,
    })
}

pub fn bump_baseline_version(set: &mut RequirementSet, next: &Version) -> Result<(), RqtkError> {
    set.config.project.version = next.clone();
    set.config.project.updated = Some(chrono::Utc::now().date_naive());
    let config_path = set.root.join("requirements.toml");
    let text = toml::to_string_pretty(&set.config)?;
    fs::write(&config_path, text).map_err(|source| RqtkError::Io {
        path: config_path,
        source,
    })
}

pub fn format_lint(
    issues: &[rqtk_core::LintIssue],
) -> (usize, usize, Vec<String>) {
    let mut errors = 0usize;
    let mut warnings = 0usize;
    let mut lines = Vec::with_capacity(issues.len());
    for issue in issues {
        let severity = match issue.severity {
            LintSeverity::Error => {
                errors += 1;
                "error"
            }
            LintSeverity::Warning => {
                warnings += 1;
                "warning"
            }
        };
        let target = issue
            .requirement_id
            .as_ref()
            .map(RequirementId::to_string)
            .or_else(|| issue.path.as_ref().map(|p| p.display().to_string()))
            .unwrap_or_else(|| "-".to_owned());
        lines.push(format!("[{severity}] {} {}: {}", issue.code, target, issue.message));
    }
    (errors, warnings, lines)
}
