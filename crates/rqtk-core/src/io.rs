use crate::error::RqtkError;
use crate::model::RequirementFile;
use std::fs;
use std::path::Path;

pub fn write_requirement_file(path: &Path, file: &RequirementFile) -> Result<(), RqtkError> {
    let text = toml::to_string_pretty(file)?;
    fs::write(path, text).map_err(|source| RqtkError::Io {
        path: path.to_path_buf(),
        source,
    })
}
