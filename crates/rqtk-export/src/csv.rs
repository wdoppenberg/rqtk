use crate::{ExportError, Exporter};
use rqtk_core::{RequirementSet, Validated};
use std::path::Path;

pub struct CsvExporter;

impl Exporter for CsvExporter {
    fn export(&self, set: &RequirementSet<Validated>, out: &Path) -> Result<(), ExportError> {
        let mut csv = String::from("id,title,category,type,state,priority,verification_method\n");
        for (id, req) in &set.requirements {
            let r = &req.requirement;
            csv.push_str(&format!(
                "{},{},{},{},{},{},{}\n",
                id.0,
                escape_csv(&r.title),
                r.category,
                r.req_type,
                r.status.state,
                r.status.priority,
                r.verification.method
            ));
        }
        std::fs::write(out, csv)?;
        Ok(())
    }

    fn file_extension(&self) -> &str {
        "csv"
    }
}

fn escape_csv(text: &str) -> String {
    let escaped = text.replace('"', "\"\"");
    format!("\"{escaped}\"")
}
