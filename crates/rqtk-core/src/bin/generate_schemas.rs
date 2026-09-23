use rqtk_core::evidence::EvidenceFile;
use rqtk_core::model::{Config, Need, Requirement, Stakeholder};
use schemars::schema_for;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repo_root = repository_root()?;
    let schema_dir = repo_root.join("schema");
    fs::create_dir_all(&schema_dir)?;

    write_schema(&schema_dir.join("config.schema.json"), &schema_for!(Config))?;
    write_schema(
        &schema_dir.join("requirement.schema.json"),
        &schema_for!(Requirement),
    )?;
    write_schema(
        &schema_dir.join("stakeholder.schema.json"),
        &schema_for!(Stakeholder),
    )?;
    write_schema(&schema_dir.join("need.schema.json"), &schema_for!(Need))?;

    write_schema(
        &schema_dir.join("evidence.schema.json"),
        &schema_for!(EvidenceFile),
    )?;

    Ok(())
}

fn repository_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .ok_or("failed to resolve repository root from CARGO_MANIFEST_DIR")?;
    Ok(root.to_path_buf())
}

fn write_schema(path: &Path, schema: &impl Serialize) -> Result<(), Box<dyn std::error::Error>> {
    let mut content = serde_json::to_string_pretty(schema)?;
    content.push('\n');
    fs::write(path, content)?;
    Ok(())
}
