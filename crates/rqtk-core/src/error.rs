use crate::model::RequirementId;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RqtkError {
    #[error("io error on {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse TOML {path}: {source}")]
    TomlParse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("failed to parse TOML {path}: {source}")]
    TomlEdit {
        path: PathBuf,
        #[source]
        source: toml_edit::TomlError,
    },
    #[error("failed to serialize TOML: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
    #[error("{path} has schema_version {found}, but this rqtk supports schema_version {supported}")]
    UnsupportedSchemaVersion {
        path: PathBuf,
        found: u32,
        supported: u32,
    },
    #[error("invalid id pattern regex `{pattern}`: {source}")]
    InvalidIdPattern {
        pattern: String,
        #[source]
        source: regex::Error,
    },
    #[error("requirement `{0}` not found")]
    RequirementNotFound(RequirementId),
    #[error("git error: {0}")]
    Git(String),
    #[error("baseline `{0}` not found — create it with `rqtk baseline {0}`")]
    BaselineNotFound(String),
    #[error("invalid baseline name `{0}`: must be non-empty and contain no `/` or whitespace")]
    InvalidBaselineName(String),
}
