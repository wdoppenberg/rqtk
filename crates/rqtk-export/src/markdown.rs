use crate::{ExportError, Exporter};
use rqtk_core::{RequirementSet, Validated};
use std::path::Path;

pub struct MarkdownExporter;

impl Exporter for MarkdownExporter {
    fn export(&self, set: &RequirementSet<Validated>, out: &Path) -> Result<(), ExportError> {
        let mut md = String::from("# Requirements Export\n\n");
        for (id, req) in &set.requirements {
            let r = &req.requirement;
            md.push_str(&format!(
                "## {}\n\n- title: {}\n- category: {}\n- type: {}\n- state: {}\n- statement: {}\n\n",
                id.0, r.title, r.category, r.req_type, r.status.state, r.statement.text
            ));
        }
        std::fs::write(out, md)?;
        Ok(())
    }

    fn file_extension(&self) -> &str {
        "md"
    }
}
