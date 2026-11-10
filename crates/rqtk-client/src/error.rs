//! Errors, each with a [`ErrorCode`] that is the same in every client language.

use rqtk_core::RqtkError;
use schemars::JsonSchema;
use serde::Serialize;

/// What went wrong, as a stable code. Branch on this, not on the message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// No `.rqtk/config.toml` at the repository root.
    NotARepository,
    /// `.rqtk/config.toml` exists but cannot be read or parsed.
    InvalidConfig,
    /// The repository uses a `schema_version` this rqtk does not read.
    UnsupportedSchema,
    /// A ref, tag, baseline or commit that does not exist.
    RefNotFound,
    /// A git URL whose transport is not allowed (only https and SSH, plus `file://` when
    /// explicitly allowed).
    RemoteRefused,
    /// Fetching a remote repository failed.
    RemoteFailed,
    /// No requirement, need or stakeholder with the given ID.
    NotFound,
    /// The operation needs git history the repository doesn't have, or git failed.
    Git,
    /// Reading a file failed.
    Io,
}

impl ErrorCode {
    /// The code as it appears in JSON and in the Python and TypeScript clients.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotARepository => "not_a_repository",
            Self::InvalidConfig => "invalid_config",
            Self::UnsupportedSchema => "unsupported_schema",
            Self::RefNotFound => "ref_not_found",
            Self::RemoteRefused => "remote_refused",
            Self::RemoteFailed => "remote_failed",
            Self::NotFound => "not_found",
            Self::Git => "git",
            Self::Io => "io",
        }
    }

    /// Every code, for documentation and the conformance suite.
    pub const ALL: [ErrorCode; 9] = [
        Self::NotARepository,
        Self::InvalidConfig,
        Self::UnsupportedSchema,
        Self::RefNotFound,
        Self::RemoteRefused,
        Self::RemoteFailed,
        Self::NotFound,
        Self::Git,
        Self::Io,
    ];
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A client error: a stable [`ErrorCode`] and a message for people.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct Error {
    pub code: ErrorCode,
    pub message: String,
}

impl Error {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

impl From<RqtkError> for Error {
    fn from(e: RqtkError) -> Self {
        let code = match &e {
            RqtkError::Io { path, source }
                if source.kind() == std::io::ErrorKind::NotFound
                    && path.ends_with(rqtk_core::repository::CONFIG_PATH) =>
            {
                ErrorCode::NotARepository
            }
            RqtkError::Io { .. } => ErrorCode::Io,
            RqtkError::TomlParse { path, .. } | RqtkError::TomlEdit { path, .. }
                if path.ends_with(rqtk_core::repository::CONFIG_PATH) =>
            {
                ErrorCode::InvalidConfig
            }
            RqtkError::InvalidIdPattern { .. } | RqtkError::InvalidScanPattern(..) => {
                ErrorCode::InvalidConfig
            }
            RqtkError::UnsupportedSchemaVersion { .. } => ErrorCode::UnsupportedSchema,
            RqtkError::RequirementNotFound(_) => ErrorCode::NotFound,
            RqtkError::BaselineNotFound(_) | RqtkError::InvalidBaselineName(_) => {
                ErrorCode::RefNotFound
            }
            RqtkError::Git(message) if message.starts_with("could not resolve") => {
                ErrorCode::RefNotFound
            }
            _ => ErrorCode::Git,
        };
        Self::new(code, e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
