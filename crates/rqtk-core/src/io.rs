use crate::error::RqtkError;
use crate::model::RequirementFile;
use crate::repository::RequirementSet;
use semver::Version;
use std::fs;
use std::path::Path;

pub fn write_requirement_file(path: &Path, file: &RequirementFile) -> Result<(), RqtkError> {
    let text = toml::to_string_pretty(file)?;
    fs::write(path, text).map_err(|source| RqtkError::Io {
        path: path.to_path_buf(),
        source,
    })
}

pub fn bump_baseline_version<S>(
    set: &mut RequirementSet<S>,
    next: &Version,
) -> Result<(), RqtkError> {
    set.config.project.version = next.clone();
    set.config.project.updated = Some(chrono::Utc::now().date_naive());
    let config_path = set.config_path.clone();
    let text = toml::to_string_pretty(&set.config)?;
    fs::write(&config_path, text).map_err(|source| RqtkError::Io {
        path: config_path,
        source,
    })
}
