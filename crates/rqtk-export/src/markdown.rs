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

        if !set.stakeholders.is_empty() {
            md.push_str("# Stakeholders\n\n");
            for (id, stk) in &set.stakeholders {
                let s = &stk.stakeholder;
                md.push_str(&format!("## {id}\n\n- name: {}\n", s.name));
                if let Some(role) = &s.role {
                    md.push_str(&format!("- role: {role}\n"));
                }
                if let Some(org) = &s.organization {
                    md.push_str(&format!("- organization: {org}\n"));
                }
                md.push('\n');
            }
        }

        if !set.needs.is_empty() {
            md.push_str("# Stakeholder Needs\n\n");
            for (id, need_file) in &set.needs {
                let n = &need_file.need;
                md.push_str(&format!(
                    "## {}\n\n- title: {}\n- state: {}\n- statement: {}\n",
                    id.0, n.title, n.status.state, n.statement.text
                ));
                if !n.stakeholders.is_empty() {
                    md.push_str(&format!("- stakeholders: {}\n", n.stakeholders.join(", ")));
                }
                md.push('\n');
            }
        }

        std::fs::write(out, md)?;
        Ok(())
    }

    fn file_extension(&self) -> &str {
        "md"
    }
}
