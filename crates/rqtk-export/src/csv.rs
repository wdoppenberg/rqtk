use crate::{ExportError, Exporter};
use rqtk_core::{RequirementSet, Validated};
use std::path::Path;

pub struct CsvExporter;

impl Exporter for CsvExporter {
    fn export(&self, set: &RequirementSet<Validated>, out: &Path) -> Result<(), ExportError> {
        let mut csv = String::from("id,title,category,type,state,priority,verification_method\n");
        for (id, r) in set.requirements() {
            let row = [
                id.0.as_str(),
                &r.title,
                &r.category,
                &r.req_type,
                &r.state,
                &r.priority,
                &r.verification.method,
            ]
            .map(escape_csv)
            .join(",");
            csv.push_str(&row);
            csv.push('\n');
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
