use thiserror::Error;

#[derive(Debug, Error)]
pub enum ReportError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error(
        "typst CLI not found — install it from https://typst.app \
         or pass --output report.typ to write the Typst source instead"
    )]
    TypstNotFound,

    #[error("typst compilation failed:\n{0}")]
    CompilationFailed(String),
}
