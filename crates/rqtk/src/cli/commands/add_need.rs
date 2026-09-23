use std::error::Error;

use rqtk_core::{NeedId, RequirementSet, StakeholderId};

use crate::cli::output::{Ctx, Exit};

pub struct AddNeedArgs {
    pub id: Option<String>,
    pub title: String,
    pub statement: String,
    pub stakeholders: Option<Vec<String>>,
    pub dry_run: bool,
}

pub fn run(ctx: &Ctx, args: AddNeedArgs) -> Result<Exit, Box<dyn Error>> {
    let set = RequirementSet::load_from_repo_root(&ctx.root)?;
    let id = args.id.map_or_else(|| set.next_need_id(), NeedId);
    let mut need = set.scaffold_need(id.clone(), &args.title, &args.statement);
    need.stakeholders = args
        .stakeholders
        .unwrap_or_default()
        .into_iter()
        .map(StakeholderId)
        .collect();
    need.content_hash = Some(need.compute_content_hash());

    let path = set.needs_dir().join(format!("{id}.toml"));
    super::add::finish(ctx, "Need", &id.0, path, &need, args.dry_run)
}
