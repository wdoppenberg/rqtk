use std::{error::Error, path::PathBuf};

use rqtk_core::io::create_toml_file;
use rqtk_core::{NeedId, RequirementId, RequirementSet, ScaffoldInput, VerificationActivity};
use serde::Serialize;

use crate::cli::output::{self, Ctx, Exit, Usage};

pub struct AddArgs {
    pub category: String,
    pub req_type: String,
    pub title: String,
    pub statement: String,
    pub rationale: Option<String>,
    pub parents: Vec<String>,
    pub satisfies: Vec<String>,
    pub priority: Option<String>,
    pub method: Option<String>,
    pub level: Option<String>,
    pub phase: Option<String>,
    pub criteria: Option<String>,
    pub activities: Vec<String>,
    pub dry_run: bool,
}

/// JSON shape shared by the `add*` commands.
#[derive(Serialize)]
pub struct Created<'a, T: Serialize> {
    pub id: &'a str,
    pub path: PathBuf,
    pub dry_run: bool,
    pub item: &'a T,
}

/// Write `item` to `path` (unless dry-running) and report it. JSON output shows `view`,
/// which may add computed fields (such as the content hash) that the file leaves out.
pub fn finish<T: Serialize>(
    ctx: &Ctx,
    kind: &str,
    id: &str,
    path: PathBuf,
    item: &T,
    view: &T,
    dry_run: bool,
) -> Result<Exit, Box<dyn Error>> {
    if !dry_run {
        create_toml_file(&path, item)?;
    }
    let path = output::relative(&path, &ctx.root);
    if ctx.json() {
        output::json(&Created {
            id,
            path,
            dry_run,
            item: view,
        })?;
    } else if dry_run {
        println!(
            "# would create {}\n{}",
            path.display(),
            toml::to_string_pretty(item)?
        );
    } else {
        output::success(
            &format!("{kind} created"),
            &[("id", id), ("file", &path.display().to_string())],
        );
    }
    Ok(Exit::Ok)
}

/// Report an item added to an existing file (which the caller has already written).
pub fn finish_edit<T: Serialize>(
    ctx: &Ctx,
    kind: &str,
    id: &str,
    path: &std::path::Path,
    item: &T,
    dry_run: bool,
) -> Result<Exit, Box<dyn Error>> {
    let path = output::relative(path, &ctx.root);
    if ctx.json() {
        output::json(&Created {
            id,
            path,
            dry_run,
            item,
        })?;
    } else if dry_run {
        println!(
            "# would add to {}\n{}",
            path.display(),
            toml::to_string_pretty(item)?
        );
    } else {
        output::success(
            &format!("{kind} added"),
            &[("id", id), ("file", &path.display().to_string())],
        );
    }
    Ok(Exit::Ok)
}

pub fn run(ctx: &Ctx, args: AddArgs) -> Result<Exit, Box<dyn Error>> {
    let set = RequirementSet::load_from_repo_root(&ctx.root)?;
    let cfg = set.config();
    if !cfg.categories.contains_key(&args.category) {
        let known: Vec<&str> = cfg.categories.keys().map(String::as_str).collect();
        return Err(Box::new(Usage(format!(
            "unknown category `{}`; expected one of: {}",
            args.category,
            known.join(", ")
        ))));
    }
    if !cfg.req_types.allowed.contains(&args.req_type) {
        return Err(Box::new(Usage(format!(
            "unknown type `{}`; expected one of: {}",
            args.req_type,
            cfg.req_types.allowed.join(", ")
        ))));
    }

    let one_of = |what: &str, value: &Option<String>, allowed: &[String]| match value {
        Some(v) if !allowed.contains(v) => Err(Usage(format!(
            "unknown {what} `{v}`; expected one of: {}",
            allowed.join(", ")
        ))),
        _ => Ok(()),
    };
    one_of("priority", &args.priority, &cfg.priority.levels)?;
    one_of(
        "verification method",
        &args.method,
        &cfg.verification.methods,
    )?;
    one_of("verification level", &args.level, &cfg.verification.levels)?;
    one_of("verification phase", &args.phase, &cfg.verification.phases)?;
    if args.parents.is_empty()
        && cfg
            .validation
            .require_parent_for_categories
            .contains(&args.category)
    {
        return Err(Box::new(Usage(format!(
            "category `{}` requires a parent; pass --parent <ID>",
            args.category
        ))));
    }
    for parent in &args.parents {
        if !set
            .requirements()
            .contains_key(&RequirementId(parent.clone()))
        {
            return Err(Box::new(Usage(format!(
                "no requirement with ID `{parent}`"
            ))));
        }
    }
    for need in &args.satisfies {
        if !set.needs().contains_key(&NeedId(need.clone())) {
            return Err(Box::new(Usage(format!("no need with ID `{need}`"))));
        }
    }

    let mut req = set.scaffold_requirement(ScaffoldInput {
        category: &args.category,
        req_type: &args.req_type,
        title: &args.title,
        statement: &args.statement,
        rationale: args.rationale.as_deref(),
    });
    req.trace.parents = args.parents.into_iter().map(RequirementId).collect();
    req.trace.satisfies = args.satisfies.into_iter().map(NeedId).collect();
    if let Some(priority) = args.priority {
        req.priority = priority;
    }
    let v = &mut req.verification;
    v.method = args.method.unwrap_or_else(|| v.method.clone());
    v.level = args.level.unwrap_or_else(|| v.level.clone());
    v.phase = args.phase.unwrap_or_else(|| v.phase.clone());
    v.success_criteria = args.criteria;
    for name in args.activities {
        let id = set.next_activity_id(&req);
        req.verification
            .activities
            .push(VerificationActivity::new(id, name));
    }

    let path = set
        .category_dir(&args.category)
        .join(format!("{}.toml", req.id));
    let mut view = req.clone();
    view.content_hash = Some(req.compute_content_hash());
    finish(
        ctx,
        "Requirement",
        &req.id.0,
        path,
        &req,
        &view,
        args.dry_run,
    )
}
