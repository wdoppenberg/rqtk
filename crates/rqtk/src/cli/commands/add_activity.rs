use std::error::Error;

use rqtk_core::{EntityRef, RequirementId, RequirementSet, VerificationActivity};

use crate::cli::output::{Ctx, Exit, Usage};

pub struct AddActivityArgs {
    pub requirement: String,
    pub name: String,
    pub id: Option<String>,
    pub dry_run: bool,
}

/// Append a verification activity to a requirement file, keeping its comments and layout.
pub fn run(ctx: &Ctx, args: AddActivityArgs) -> Result<Exit, Box<dyn Error>> {
    let set = RequirementSet::load_from_repo_root(&ctx.root)?;
    let req_id = RequirementId(args.requirement.clone());
    let Some(req) = set.requirements().get(&req_id) else {
        return Err(Usage(format!("no requirement with ID `{}`", args.requirement)).into());
    };
    let id = match args.id {
        Some(id) => {
            let taken = set
                .requirements()
                .values()
                .flat_map(|r| &r.verification.activities)
                .any(|a| a.id == id);
            if taken {
                return Err(Usage(format!("activity ID `{id}` is already in use")).into());
            }
            id
        }
        None => set.next_activity_id(req),
    };
    let activity = VerificationActivity::new(id.clone(), args.name.clone());
    let path = if args.dry_run {
        set.path_of(&EntityRef::Requirement(req_id.clone()))
            .expect("a loaded requirement has a file")
            .to_path_buf()
    } else {
        set.append_activity(&req_id, &activity)?
    };
    super::add::finish_edit(ctx, "Activity", &id, &path, &activity, args.dry_run)
}
