//! Every way a command can fail, and what each means for the exit code and `--json` output.

use std::path::PathBuf;

use crate::cli::output::Usage;

/// A failed command. The variant decides the exit code ([`CliError::exit_code`]) and the
/// `kind` and `code` of the JSON error document, so no caller inspects error types at runtime.
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    /// The command was invoked wrongly: bad arguments, an unknown ID or kind.
    #[error(transparent)]
    Usage(#[from] Usage),
    /// Opening or querying the repository through the client failed.
    #[error(transparent)]
    Client(#[from] rqtk_client::Error),
    /// Loading, checking or writing requirement files failed.
    #[error(transparent)]
    Core(#[from] rqtk_core::RqtkError),
    /// Exporting or rendering a report failed.
    #[error(transparent)]
    Export(#[from] rqtk_export::ExportError),
    /// Reading or writing a file failed.
    #[error("{}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// Serialising a document failed.
    #[error("cannot serialise output: {0}")]
    Json(#[from] serde_json::Error),
    /// Serialising TOML failed.
    #[error("cannot serialise TOML: {0}")]
    Toml(#[from] toml::ser::Error),
    /// The `git` executable failed.
    #[error("git failed: {0}")]
    Git(String),
    /// No `.git` directory at or above the repository root.
    #[error("no .git directory found from {}", .0.display())]
    NotAGitRepository(PathBuf),
    /// `$EDITOR` exited unsuccessfully.
    #[error("editor `{editor}` exited with status {status}")]
    Editor {
        editor: String,
        status: std::process::ExitStatus,
    },
    /// A search pattern could not be compiled.
    #[error("invalid search pattern: {0}")]
    Pattern(#[from] regex::Error),
    /// Running another program (`$EDITOR`, `git`) failed to start.
    #[error("cannot run `{program}`: {source}")]
    Spawn {
        program: String,
        #[source]
        source: std::io::Error,
    },
}

/// Attach the path an I/O operation was working on: `fs::write(&p, x).at(&p)?`.
pub trait IoContext<T> {
    fn at(self, path: impl AsRef<std::path::Path>) -> Result<T, CliError>;
}

impl<T> IoContext<T> for std::io::Result<T> {
    fn at(self, path: impl AsRef<std::path::Path>) -> Result<T, CliError> {
        self.map_err(|source| CliError::Io {
            path: path.as_ref().to_path_buf(),
            source,
        })
    }
}

impl CliError {
    /// 2 for usage errors, 3 for everything else.
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Usage(_) => 2,
            _ => 3,
        }
    }

    /// The JSON error document's `kind`.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Usage(_) => "usage",
            _ => "error",
        }
    }

    /// The client's error code, when the error came from opening or querying the repository.
    pub fn code(&self) -> Option<rqtk_client::ErrorCode> {
        match self {
            Self::Client(e) => Some(e.code),
            _ => None,
        }
    }

    /// A client error, as a usage error when it names an item that doesn't exist.
    pub fn usage_if_not_found(e: rqtk_client::Error) -> Self {
        match e.code {
            rqtk_client::ErrorCode::NotFound => Self::Usage(Usage(e.message)),
            _ => Self::Client(e),
        }
    }
}
