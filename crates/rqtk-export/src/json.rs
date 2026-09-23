use std::{collections::BTreeMap, path::Path};

use crate::{ExportError, Exporter};
use rqtk_core::{
    Need, NeedId, Requirement, RequirementId, RequirementSet, Stakeholder, StakeholderId, Validated,
};
use serde::Serialize;

#[derive(Serialize)]
struct ExportPayload<'a> {
    schema_version: u32,
    requirements: &'a BTreeMap<RequirementId, Requirement>,
    stakeholders: &'a BTreeMap<StakeholderId, Stakeholder>,
    needs: &'a BTreeMap<NeedId, Need>,
}

pub struct JsonExporter;

impl Exporter for JsonExporter {
    fn export(&self, set: &RequirementSet<Validated>, out: &Path) -> Result<(), ExportError> {
        let payload = serde_json::to_string_pretty(&ExportPayload {
            schema_version: rqtk_core::SCHEMA_VERSION,
            requirements: set.requirements(),
            stakeholders: set.stakeholders(),
            needs: set.needs(),
        })?;
        std::fs::write(out, payload)?;
        Ok(())
    }

    fn file_extension(&self) -> &str {
        "json"
    }
}
