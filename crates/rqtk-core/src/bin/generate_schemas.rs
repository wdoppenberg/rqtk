use rqtk_core::schema::{KINDS, json_schema};
use std::fs;
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let schema_dir = repository_root()?.join("schema");
    fs::create_dir_all(&schema_dir)?;
    for (kind, _) in KINDS {
        let schema = json_schema(kind).ok_or("schema kind without a schema")?;
        let mut content = serde_json::to_string_pretty(&schema)?;
        content.push('\n');
        fs::write(schema_dir.join(format!("{kind}.schema.json")), content)?;
    }
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
