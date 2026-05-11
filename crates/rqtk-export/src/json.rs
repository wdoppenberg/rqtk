use crate::{ExportError, Exporter};
use rqtk_core::{RequirementSet, Validated};
use std::path::Path;

pub struct JsonExporter;

impl Exporter for JsonExporter {
    fn export(&self, set: &RequirementSet<Validated>, out: &Path) -> Result<(), ExportError> {
        let payload = serde_json::to_string_pretty(&set.requirements)?;
        std::fs::write(out, payload)?;
        Ok(())
    }

    fn file_extension(&self) -> &str {
        "json"
    }
}
