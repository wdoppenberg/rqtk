use crate::repository::{RequirementSet, Validated, load_config};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
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
    let requirements_dir =
        find_requirements_dir_from(PathBuf::from(manifest_dir))?.ok_or_else(|| {
            "no `.rqtk/config.toml` found by walking up from CARGO_MANIFEST_DIR".to_owned()
        })?;
    scan_dir_for_activity(&requirements_dir, activity_id)
}

pub fn find_activity_from_current_dir(activity_id: &str) -> Result<Option<ActivityInfo>, String> {
    let start =
        std::env::current_dir().map_err(|e| format!("cannot read current directory: {e}"))?;
    let requirements_dir = find_requirements_dir_from(start)?.ok_or_else(|| {
        "no `.rqtk/config.toml` found by walking up from current working directory".to_owned()
    })?;
    scan_dir_for_activity(&requirements_dir, activity_id)
}

pub fn build_requirements_doc_from_manifest_dir() -> Result<String, String> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .map_err(|_| "CARGO_MANIFEST_DIR is not set".to_owned())?;
    let repo_root = find_repo_root_from(PathBuf::from(manifest_dir))?.ok_or_else(|| {
        "no `.rqtk/config.toml` found by walking up from CARGO_MANIFEST_DIR".to_owned()
    })?;
    build_requirements_doc_from_repo_root(&repo_root)
}

pub fn build_requirements_doc_from_repo_root(repo_root: &Path) -> Result<String, String> {
    let set = RequirementSet::load_from_repo_root(repo_root).map_err(|e| {
        format!(
            "cannot load requirements from `{}`: {e}",
            repo_root.display()
        )
    })?;
    let (set, issues) = set.validate();
    let errors: Vec<_> = issues
        .into_iter()
        .filter(|issue| issue.is_error())
        .collect();
    if !errors.is_empty() {
        let mut msg = format!(
            "cannot generate docs: {} validation error(s) in requirements",
            errors.len()
        );
        for issue in errors.iter().take(8) {
            msg.push_str(&format!("\n- [{}] {}", issue.code, issue.message));
        }
        if errors.len() > 8 {
            msg.push_str("\n- ...");
        }
        return Err(msg);
    }
    Ok(render_requirements_doc(&set))
}

fn find_requirements_dir_from(mut dir: PathBuf) -> Result<Option<PathBuf>, String> {
    loop {
        let config_path = dir.join(".rqtk/config.toml");
        if config_path.is_file() {
            let config = load_config(&config_path).map_err(|e| e.to_string())?;
            return Ok(Some(dir.join(config.repository.requirements_dir)));
        }
        if !dir.pop() {
            return Ok(None);
        }
    }
}

fn find_repo_root_from(mut dir: PathBuf) -> Result<Option<PathBuf>, String> {
    loop {
        let config_path = dir.join(".rqtk/config.toml");
        if config_path.is_file() {
            return Ok(Some(dir));
        }
        if !dir.pop() {
            return Ok(None);
        }
    }
}

fn render_requirements_doc(set: &RequirementSet<Validated>) -> String {
    let requirements_dir_display = match set.root.strip_prefix(&set.repo_root) {
        Ok(path) if path.as_os_str().is_empty() => ".".to_owned(),
        Ok(path) => path.display().to_string(),
        Err(_) => set.root.display().to_string(),
    };

    let mut doc = String::new();
    doc.push_str("# Requirements\n\n");
    doc.push_str(&format!(
        "**Project**: {}  \n**Requirements Dir**: `{}`\n\n",
        set.config.project.name, requirements_dir_display
    ));
    doc.push_str(&format!(
        "Total requirements: **{}**\n\n",
        set.requirements.len()
    ));

    if set.requirements.is_empty() {
        doc.push_str("No requirements were found.\n");
        return doc;
    }

    for (category_id, category) in &set.config.categories {
        let reqs: Vec<_> = set
            .requirements
            .iter()
            .filter(|(_, req)| req.category == *category_id)
            .collect();
        doc.push_str(&format!(
            "## {} — {}\n\nRequirements: **{}**\n\n",
            category_id,
            category.name,
            reqs.len()
        ));
        if let Some(description) = &category.description {
            push_blockquote(&mut doc, description);
            doc.push('\n');
        }
        if reqs.is_empty() {
            doc.push_str("_No requirements in this category._\n\n");
            continue;
        }

        for (id, req) in reqs {
            doc.push_str(&format!("### `{}` — {}\n\n", id.0, req.title));
            doc.push_str(&format!(
                "- Type: `{}`\n- State: `{}`\n- Priority: `{}`\n- Verification Method: `{}`\n\n",
                req.req_type, req.state, req.priority, req.verification.method
            ));
            doc.push_str("**Statement**\n\n");
            push_blockquote(&mut doc, &req.statement);
            if let Some(rationale) = &req.rationale {
                doc.push_str("\n**Rationale**\n\n");
                push_blockquote(&mut doc, rationale);
            }
            if !req.trace.parents.is_empty() {
                doc.push_str("\n**Parents**\n\n");
                for parent in &req.trace.parents {
                    doc.push_str(&format!("- `{}`\n", parent.0));
                }
            }
            if let Some(criteria) = &req.verification.success_criteria {
                doc.push_str("\n**Verification Success Criteria**\n\n");
                push_blockquote(&mut doc, criteria);
            }
            if req.verification.activities.is_empty() {
                doc.push_str("\n**Verification Activities**\n\n- _None defined_\n");
            } else {
                doc.push_str("\n**Verification Activities**\n\n");
                for activity in &req.verification.activities {
                    doc.push_str(&format!("- `{}` {}", activity.id, activity.name));
                    if let Some(status) = &activity.status {
                        doc.push_str(&format!(" _(status: {})_", status));
                    }
                    doc.push('\n');
                }
            }
            doc.push('\n');
        }
    }

    doc
}

fn push_blockquote(doc: &mut String, text: &str) {
    for line in text.lines() {
        doc.push_str("> ");
        doc.push_str(line);
        doc.push('\n');
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

    let req = &value;

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
            path: path.to_path_buf(),
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
