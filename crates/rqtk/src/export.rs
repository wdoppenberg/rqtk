use std::{error::Error, path::PathBuf};

use clap::ValueEnum;
use rqtk_core::{RequirementSet, Validated};
use serde::Serialize;

#[derive(Clone, Debug, Serialize, ValueEnum)]
pub enum ExportFormat {
    Csv,
    Json,
    Markdown,
}

pub fn default_export_path(requirements_dir: &std::path::Path, format: &ExportFormat) -> PathBuf {
    requirements_dir.join(format!("requirements-export.{format:?}"))
}

fn escape_csv(text: &str) -> String {
    let escaped = text.replace('"', "\"\"");
    format!("\"{escaped}\"")
}

pub fn export_set(
    set: &RequirementSet<Validated>,
    format: &ExportFormat,
    out: &std::path::Path,
) -> Result<(), Box<dyn Error>> {
    use ExportFormat::*;
    match format {
        Json => {
            let payload = serde_json::to_string_pretty(&set.requirements)?;
            std::fs::write(out, payload)?;
        }
        Csv => {
            let mut csv =
                String::from("id,title,category,type,state,priority,verification_method\n");
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
        }
        Markdown => {
            let mut md = String::from("# Requirements Export\n\n");
            for (id, req) in &set.requirements {
                let r = &req.requirement;
                md.push_str(&format!(
                    "## {}\n\n- title: {}\n- category: {}\n- type: {}\n- state: {}\n- statement: {}\n\n",
                    id.0, r.title, r.category, r.req_type, r.status.state, r.statement.text
                ));
            }
            std::fs::write(out, md)?;
        }
    }
    Ok(())
}
