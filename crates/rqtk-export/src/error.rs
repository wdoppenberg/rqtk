use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExportError {
    #[error("I/O error writing export: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
}
