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
    #[error("failed to serialize TOML: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
    #[error("invalid id pattern regex `{pattern}`: {source}")]
    InvalidIdPattern {
        pattern: String,
        #[source]
        source: regex::Error,
    },
    #[error("duplicate requirement id `{0}`")]
    DuplicateRequirement(RequirementId),
    #[error("requirement `{0}` not found")]
    RequirementNotFound(RequirementId),
    #[error("unsupported export format `{0}`")]
    UnsupportedExportFormat(String),
}
