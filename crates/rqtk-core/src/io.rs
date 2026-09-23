use crate::error::RqtkError;
use serde::Serialize;
use std::fs;
use std::io::Write;
use std::path::Path;

/// Serialize `value` into a new file at `path`, creating parent directories.
/// Fails if the file already exists.
pub fn create_toml_file<T: Serialize>(path: &Path, value: &T) -> Result<(), RqtkError> {
    let io_err = |source| RqtkError::Io {
        path: path.to_path_buf(),
        source,
    };
    let text = toml::to_string_pretty(value)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(io_err)?;
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(io_err)?;
    file.write_all(text.as_bytes()).map_err(io_err)
}

/// Edit an existing TOML file in place, preserving comments, ordering and formatting of
/// everything `edit` does not touch. The file is only rewritten if its text changed.
pub fn edit_toml_file(
    path: &Path,
    edit: impl FnOnce(&mut toml_edit::DocumentMut),
) -> Result<bool, RqtkError> {
    let io_err = |source| RqtkError::Io {
        path: path.to_path_buf(),
        source,
    };
    let text = fs::read_to_string(path).map_err(io_err)?;
    let mut doc: toml_edit::DocumentMut = text.parse().map_err(|source| RqtkError::TomlEdit {
        path: path.to_path_buf(),
        source,
    })?;
    edit(&mut doc);
    let updated = doc.to_string();
    if updated == text {
        return Ok(false);
    }
    fs::write(path, updated).map_err(io_err)?;
    Ok(true)
}
