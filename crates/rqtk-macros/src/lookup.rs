//! Finding a verification activity in the requirement files, with nothing but a TOML parser,
//! so that depending on the macros stays light. (`rqtk-core::verification` does the same for
//! the Python package.)

use std::path::{Path, PathBuf};

pub struct ActivityInfo {
    /// Requirement file that defines the activity.
    pub path: PathBuf,
    pub req_id: String,
    pub req_title: String,
    pub req_statement: String,
    pub activity_name: String,
    pub activity_status: Option<String>,
    pub activity_procedure: Option<String>,
    pub activity_expected: Option<String>,
}

/// The rustdoc a `#[verifies]` item gets: the requirement and the activity it proves.
pub fn verification_doc(activity_id: &str, info: &ActivityInfo) -> String {
    let mut doc = format!(
        "**Verifies** `{}` — {}\n\n> {}\n\n**Activity** `{}` — {}",
        info.req_id, info.req_title, info.req_statement, activity_id, info.activity_name,
    );
    if let Some(status) = &info.activity_status {
        doc.push_str(&format!("\n\n- **Status**: {status}"));
    }
    if let Some(procedure) = &info.activity_procedure {
        doc.push_str(&format!("\n- **Procedure**: {procedure}"));
    }
    if let Some(expected) = &info.activity_expected {
        doc.push_str(&format!("\n- **Expected result**: {expected}"));
    }
    doc
}

/// Look up `activity_id` in the requirements of the nearest `.rqtk/config.toml` above
/// `CARGO_MANIFEST_DIR`.
pub fn find_activity_from_manifest_dir(activity_id: &str) -> Result<Option<ActivityInfo>, String> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .map_err(|_| "CARGO_MANIFEST_DIR is not set".to_owned())?;
    let requirements_dir =
        requirements_dir_from(PathBuf::from(manifest_dir))?.ok_or_else(|| {
            "no `.rqtk/config.toml` found by walking up from CARGO_MANIFEST_DIR".to_owned()
        })?;
    scan_dir(&requirements_dir, activity_id)
}

fn requirements_dir_from(mut dir: PathBuf) -> Result<Option<PathBuf>, String> {
    loop {
        let config = dir.join(".rqtk/config.toml");
        if config.is_file() {
            let value = parse(&config)?;
            let requirements_dir = value
                .get("repository")
                .and_then(|r| r.get("requirements_dir"))
                .and_then(|d| d.as_str())
                .unwrap_or(".rqtk/requirements");
            return Ok(Some(dir.join(requirements_dir)));
        }
        if !dir.pop() {
            return Ok(None);
        }
    }
}

fn parse(path: &Path) -> Result<toml::Value, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read `{}`: {e}", path.display()))?;
    toml::from_str(&text).map_err(|e| format!("cannot parse `{}`: {e}", path.display()))
}

fn scan_dir(dir: &Path, activity_id: &str) -> Result<Option<ActivityInfo>, String> {
    let entries = std::fs::read_dir(dir)
        .map_err(|e| format!("cannot read directory `{}`: {e}", dir.display()))?;
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.is_dir() {
            if let Some(info) = scan_dir(&path, activity_id)? {
                return Ok(Some(info));
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("toml")
            && let Some(info) = activity_in(&path, activity_id)?
        {
            return Ok(Some(info));
        }
    }
    Ok(None)
}

fn activity_in(path: &Path, activity_id: &str) -> Result<Option<ActivityInfo>, String> {
    let req = parse(path)?;
    let Some(activities) = req
        .get("verification")
        .and_then(|v| v.get("activities"))
        .and_then(|a| a.as_array())
    else {
        return Ok(None);
    };
    let text =
        |table: &toml::Value, key: &str| table.get(key).and_then(|v| v.as_str()).map(str::to_owned);
    Ok(activities
        .iter()
        .find(|a| a.get("id").and_then(|v| v.as_str()) == Some(activity_id))
        .map(|a| ActivityInfo {
            path: path.to_path_buf(),
            req_id: text(&req, "id").unwrap_or_else(|| "?".to_owned()),
            req_title: text(&req, "title").unwrap_or_default(),
            req_statement: text(&req, "statement").unwrap_or_default(),
            activity_name: text(a, "name").unwrap_or_default(),
            activity_status: text(a, "status"),
            activity_procedure: text(a, "procedure"),
            activity_expected: text(a, "expected_result"),
        }))
}
