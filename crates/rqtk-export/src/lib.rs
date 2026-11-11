//! JSON, CSV and Markdown export of an rqtk requirement set.
//!
//! This crate is internal to rqtk: it is published so that `cargo install rqtk` can build, and
//! stays at 0.x, so any minor version may break it. To read an rqtk repository from Rust, use
//! [`rqtk-client`](https://docs.rs/rqtk-client), which follows semantic versioning with the CLI.

mod csv;
pub mod error;
mod json;
mod markdown;

pub use error::ExportError;

use rqtk_core::{RequirementSet, Validated};
use std::path::{Path, PathBuf};

/// Implemented by every format backend. Add new formats by
/// implementing this trait — either in this crate or in a downstream crate.
pub trait Exporter {
    fn export(&self, set: &RequirementSet<Validated>, out: &Path) -> Result<(), ExportError>;
    fn file_extension(&self) -> &str;
}

#[cfg_attr(feature = "clap", derive(clap::ValueEnum))]
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Csv,
    Json,
    Markdown,
}

impl ExportFormat {
    pub fn exporter(&self) -> &dyn Exporter {
        match self {
            ExportFormat::Csv => &csv::CsvExporter,
            ExportFormat::Json => &json::JsonExporter,
            ExportFormat::Markdown => &markdown::MarkdownExporter,
        }
    }
}

pub fn export_set(
    set: &RequirementSet<Validated>,
    format: &ExportFormat,
    out: &Path,
) -> Result<(), ExportError> {
    format.exporter().export(set, out)
}

pub fn default_export_path(root: &Path, format: &ExportFormat) -> PathBuf {
    let ext = format.exporter().file_extension();
    root.join(format!("requirements-export.{ext}"))
}
