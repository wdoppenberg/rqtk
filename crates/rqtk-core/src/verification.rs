use crate::model::RqtkConfig;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ActivityInfo {
    pub req_id: String,
    pub req_title: String,
    pub req_statement: String,
    pub activity_name: String,
    pub activity_status: Option<String>,
    pub activity_procedure: Option<String>,
    pub activity_expected: Option<String>,
}

pub fn build_verification_doc(activity_id: &str, info: &ActivityInfo) -> String {
    let mut doc = format!(
        "**Verifies** `{}` — {}\n\n> {}\n\n**Activity** `{}` — {}",
        info.req_id, info.req_title, info.req_statement, activity_id, info.activity_name,
    );
    if let Some(status) = &info.activity_status {
        doc.push_str(&format!("\n\n- **Status**: {}", status));
    }
    if let Some(procedure) = &info.activity_procedure {
        doc.push_str(&format!("\n- **Procedure**: {}", procedure));
    }
    if let Some(expected) = &info.activity_expected {
        doc.push_str(&format!("\n- **Expected result**: {}", expected));
    }
    doc
}

pub fn find_activity_from_manifest_dir(activity_id: &str) -> Result<Option<ActivityInfo>, String> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .map_err(|_| "CARGO_MANIFEST_DIR is not set".to_owned())?;
    let requirements_dir = find_requirements_dir_from(PathBuf::from(manifest_dir))?.ok_or_else(
        || "no `rqtk.toml` (or legacy `requirements/` directory) found by walking up from CARGO_MANIFEST_DIR".to_owned(),
    )?;
    scan_dir_for_activity(&requirements_dir, activity_id)
}

pub fn find_activity_from_current_dir(activity_id: &str) -> Result<Option<ActivityInfo>, String> {
    let start =
        std::env::current_dir().map_err(|e| format!("cannot read current directory: {e}"))?;
    let requirements_dir = find_requirements_dir_from(start)?.ok_or_else(
        || "no `rqtk.toml` (or legacy `requirements/` directory) found by walking up from current working directory".to_owned(),
    )?;
    scan_dir_for_activity(&requirements_dir, activity_id)
}

fn find_requirements_dir_from(mut dir: PathBuf) -> Result<Option<PathBuf>, String> {
    loop {
        let config_path = dir.join("rqtk.toml");
        if config_path.is_file() {
            let text = std::fs::read_to_string(&config_path)
                .map_err(|e| format!("cannot read `{}`: {}", config_path.display(), e))?;
            let config: RqtkConfig = toml::from_str(&text)
                .map_err(|e| format!("cannot parse `{}`: {}", config_path.display(), e))?;
            return Ok(Some(dir.join(config.repository.requirements_dir.clone())));
        }
        let candidate = dir.join("requirements");
        if candidate.is_dir() {
            return Ok(Some(candidate));
        }
        if !dir.pop() {
            return Ok(None);
        }
    }
}

fn scan_dir_for_activity(dir: &Path, activity_id: &str) -> Result<Option<ActivityInfo>, String> {
    let entries = std::fs::read_dir(dir)
        .map_err(|e| format!("cannot read directory `{}`: {}", dir.display(), e))?;

    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            if let Some(info) = scan_dir_for_activity(&path, activity_id)? {
                return Ok(Some(info));
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("toml")
            && let Some(info) = extract_activity_info(&path, activity_id)?
        {
            return Ok(Some(info));
        }
    }
    Ok(None)
}

fn extract_activity_info(path: &Path, activity_id: &str) -> Result<Option<ActivityInfo>, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read `{}`: {}", path.display(), e))?;

    let value: toml::Value = toml::from_str(&content)
        .map_err(|e| format!("cannot parse `{}`: {}", path.display(), e))?;

    let req = match value.get("requirement") {
        Some(r) => r,
        None => return Ok(None),
    };

    let activities = match req
        .get("verification")
        .and_then(|v| v.get("activities"))
        .and_then(|a| a.as_array())
    {
        Some(a) => a,
        None => return Ok(None),
    };

    for entry in activities {
        let id = entry.get("id").and_then(|v| v.as_str()).unwrap_or("");
        if id != activity_id {
            continue;
        }

        let req_id = req
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("?")
            .to_owned();
        let req_title = req
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_owned();
        let req_statement = req
            .get("statement")
            .and_then(|s| s.get("text"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_owned();
        let activity_name = entry
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_owned();
        let activity_status = entry
            .get("status")
            .and_then(|v| v.as_str())
            .map(str::to_owned);
        let activity_procedure = entry
            .get("procedure")
            .and_then(|v| v.as_str())
            .map(str::to_owned);
        let activity_expected = entry
            .get("expected_result")
            .and_then(|v| v.as_str())
            .map(str::to_owned);

        return Ok(Some(ActivityInfo {
            req_id,
            req_title,
            req_statement,
            activity_name,
            activity_status,
            activity_procedure,
            activity_expected,
        }));
    }

    Ok(None)
}
