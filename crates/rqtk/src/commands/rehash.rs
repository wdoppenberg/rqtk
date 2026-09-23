use std::{error::Error, path::Path};

use rqtk_core::{RequirementSet, io::{write_need_file, write_requirement_file}};

use crate::output;

pub fn run(repo_root: &Path) -> Result<(), Box<dyn Error>> {
    let mut set = RequirementSet::load_from_repo_root(repo_root)?;
    let mut updated = 0usize;

    for (id, req_file) in &mut set.requirements {
        let computed = req_file.requirement.compute_content_hash();
        if req_file.requirement.content_hash.as_deref() != Some(computed.as_str()) {
            req_file.requirement.content_hash = Some(computed);
            let path = set.files_by_id[id].clone();
            write_requirement_file(&path, req_file)?;
            updated += 1;
        }
    }

    for (id, need_file) in &mut set.needs {
        let computed = need_file.need.compute_content_hash();
        if need_file.need.content_hash.as_deref() != Some(computed.as_str()) {
            need_file.need.content_hash = Some(computed);
            let path = set.needs_by_id[id].clone();
            write_need_file(&path, need_file)?;
            updated += 1;
        }
    }

    if updated == 0 {
        output::success("All content hashes are current", &[]);
    } else {
        output::success("Content hashes updated", &[("count", &updated.to_string())]);
    }
    Ok(())
}
