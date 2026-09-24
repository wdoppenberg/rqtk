use std::error::Error;

use rqtk_core::{RequirementSet, StakeholderId};

use crate::cli::output::{Ctx, Exit};

pub struct AddStakeholderArgs {
    pub id: Option<String>,
    pub name: String,
    pub role: Option<String>,
    pub organization: Option<String>,
    pub dry_run: bool,
}

pub fn run(ctx: &Ctx, args: AddStakeholderArgs) -> Result<Exit, Box<dyn Error>> {
    let set = RequirementSet::load_from_repo_root(&ctx.root)?;
    let id = args
        .id
        .map_or_else(|| set.next_stakeholder_id(), StakeholderId);
    let mut stakeholder = set.scaffold_stakeholder(id.clone(), &args.name);
    stakeholder.role = args.role;
    stakeholder.organization = args.organization;

    let path = set.stakeholders_dir().join(format!("{id}.toml"));
    super::add::finish(
        ctx,
        "Stakeholder",
        &id.0,
        path,
        &stakeholder,
        &stakeholder,
        args.dry_run,
    )
}
