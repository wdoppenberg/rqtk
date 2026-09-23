use std::{collections::BTreeMap, path::Path};

use crate::{ExportError, Exporter};
use rqtk_core::{NeedFile, NeedId, RequirementFile, RequirementId, RequirementSet, StakeholderFile, Validated};
use serde::Serialize;

#[derive(Serialize)]
struct ExportPayload<'a> {
    requirements: &'a BTreeMap<RequirementId, RequirementFile>,
    stakeholders: &'a BTreeMap<String, StakeholderFile>,
    needs: &'a BTreeMap<NeedId, NeedFile>,
}

pub struct JsonExporter;

impl Exporter for JsonExporter {
    fn export(&self, set: &RequirementSet<Validated>, out: &Path) -> Result<(), ExportError> {
        let payload = serde_json::to_string_pretty(&ExportPayload {
            requirements: &set.requirements,
            stakeholders: &set.stakeholders,
            needs: &set.needs,
        })?;
        std::fs::write(out, payload)?;
        Ok(())
    }

    fn file_extension(&self) -> &str {
        "json"
    }
}
