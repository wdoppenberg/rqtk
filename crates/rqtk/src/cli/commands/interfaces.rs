//! `rqtk interfaces add | sync | remove | list`: consume interface requirements published
//! by other repositories.

use rqtk_core::sync::{SyncOptions, SyncedSource};
use rqtk_core::vendor::{self, LOCK_PATH, Lockfile, VENDOR_DIR};
use rqtk_core::{InterfaceSource, load_config};
use serde::Serialize;

use crate::cli::error::CliError;
use crate::cli::output::{self, Ctx, Exit, Usage};

/// Printed by every `add` and `sync`: what is vendored comes from someone else.
pub const UNTRUSTED_WARNING: &str = "Vendored interface content comes from the provider's repository. \
Review the changes to .rqtk/vendor/ and .rqtk/interfaces.lock before committing them: \
an untrusted provider controls the requirements and spec files you pull in.";

fn warn(ctx: &Ctx) {
    if ctx.json() {
        eprintln!("warning: {UNTRUSTED_WARNING}");
    } else {
        output::warning(UNTRUSTED_WARNING);
    }
}

fn config_path(ctx: &Ctx) -> std::path::PathBuf {
    ctx.root.join(rqtk_core::repository::CONFIG_PATH)
}

pub struct AddArgs {
    pub alias: String,
    pub url: String,
    pub reference: String,
    pub allow_file_url: bool,
    pub dry_run: bool,
}

#[derive(Serialize)]
struct AddReport<'a> {
    alias: &'a str,
    git: &'a str,
    #[serde(rename = "ref")]
    reference: &'a str,
    written: bool,
    warning: &'static str,
}

pub fn add(ctx: &Ctx, args: AddArgs) -> Result<Exit, CliError> {
    warn(ctx);
    if !vendor::valid_alias(&args.alias) {
        return Err(Usage(format!(
            "invalid alias `{}`: use letters, digits, `-` and `_`",
            args.alias
        ))
        .into());
    }
    vendor::check_url(&args.url, args.allow_file_url).map_err(Usage)?;
    let config = load_config(&config_path(ctx))?;
    if config.interfaces.contains_key(&args.alias) {
        return Err(Usage(format!(
            "interface source `{}` is already declared; remove it first or edit .rqtk/config.toml",
            args.alias
        ))
        .into());
    }
    let source = InterfaceSource {
        git: args.url.clone(),
        reference: args.reference.clone(),
    };
    if !args.dry_run {
        vendor::declare_source(&ctx.root, &args.alias, &source)?;
    }
    if ctx.json() {
        output::document(
            "interfaces-add",
            &AddReport {
                alias: &args.alias,
                git: &args.url,
                reference: &args.reference,
                written: !args.dry_run,
                warning: UNTRUSTED_WARNING,
            },
        )?;
    } else {
        let label = if args.dry_run {
            "Dry run: interface source not written"
        } else {
            "Interface source added"
        };
        output::success(
            label,
            &[
                ("alias", &args.alias),
                ("git", &args.url),
                ("ref", &args.reference),
                ("next", &format!("rqtk interfaces sync {}", args.alias)),
            ],
        );
    }
    Ok(Exit::Ok)
}

pub struct SyncArgs {
    pub alias: Option<String>,
    pub allow_file_url: bool,
}

#[derive(Serialize)]
struct SyncReport<'a> {
    sources: &'a [SyncedSource],
    warning: &'static str,
}

pub fn sync(ctx: &Ctx, args: SyncArgs) -> Result<Exit, CliError> {
    warn(ctx);
    let config = load_config(&config_path(ctx))?;
    let options = SyncOptions {
        alias: args.alias.clone(),
        allow_file_url: args.allow_file_url,
    };
    // Bad input is a usage error, reported before anything is fetched.
    rqtk_core::sync::targets(&config, &options).map_err(Usage)?;
    let sources = rqtk_core::sync::sync(&ctx.root, &options)?;
    for warning in sources.iter().flat_map(|s| &s.warnings) {
        if ctx.json() {
            eprintln!("warning: {}", output::safe(warning));
        } else {
            output::warning(warning);
        }
    }
    if ctx.json() {
        output::document(
            "interfaces-sync",
            &SyncReport {
                sources: &sources,
                warning: UNTRUSTED_WARNING,
            },
        )?;
        return Ok(Exit::Ok);
    }
    if sources.is_empty() {
        output::success("No interface sources declared", &[]);
        return Ok(Exit::Ok);
    }
    for s in &sources {
        let label = if s.changed {
            format!("Synced {}", s.alias)
        } else {
            format!("{} is up to date", s.alias)
        };
        let commit = vendor::short_commit(&s.commit);
        let requirements = s.requirements.to_string();
        output::success(
            &label,
            &[
                (
                    "ref",
                    &format!("{} ({:?})", s.reference, s.ref_kind).to_lowercase(),
                ),
                ("commit", commit),
                ("requirements", &requirements),
            ],
        );
    }
    if sources.iter().any(|s| s.changed) {
        println!(
            "\n  Review with `git diff -- {VENDOR_DIR} {LOCK_PATH}`; `rqtk coverage` shows the requirements this puts under review."
        );
    }
    Ok(Exit::Ok)
}

#[derive(Serialize)]
struct RemoveReport<'a> {
    alias: &'a str,
    removed: bool,
}

pub fn remove(ctx: &Ctx, alias: &str) -> Result<Exit, CliError> {
    // Read the lockfile before touching anything, so a corrupt one stops the removal
    // before the config is edited.
    Lockfile::read(&ctx.root)?;
    let declared = vendor::undeclare_source(&ctx.root, alias)?;
    let vendored = rqtk_core::sync::remove(&ctx.root, alias)?;
    if !declared && !vendored {
        return Err(Usage(format!(
            "no interface source `{alias}` is declared or vendored"
        ))
        .into());
    }
    if ctx.json() {
        output::document(
            "interfaces-remove",
            &RemoveReport {
                alias,
                removed: true,
            },
        )?;
    } else {
        output::success(
            "Interface source removed",
            &[
                ("alias", alias),
                (
                    "next",
                    "rqtk lint  (lists consumes links that still name it)",
                ),
            ],
        );
    }
    Ok(Exit::Ok)
}

#[derive(Serialize)]
pub struct ListReport<'a> {
    sources: &'a [Listed],
}

#[derive(Serialize)]
struct Listed {
    alias: String,
    git: String,
    #[serde(rename = "ref")]
    reference: String,
    commit: Option<String>,
    requirements: usize,
}

pub fn list(ctx: &Ctx) -> Result<Exit, CliError> {
    let config = load_config(&config_path(ctx))?;
    let lock = Lockfile::read(&ctx.root)?.unwrap_or_default();
    let listed: Vec<Listed> = config
        .interfaces
        .iter()
        .map(|(alias, source)| {
            let locked = lock.sources.get(alias);
            Listed {
                alias: alias.clone(),
                git: source.git.clone(),
                reference: source.reference.clone(),
                commit: locked.map(|l| l.commit.clone()),
                requirements: locked.map_or(0, |l| l.requirements.len()),
            }
        })
        .collect();
    if ctx.json() {
        output::document("interfaces-list", &ListReport { sources: &listed })?;
        return Ok(Exit::Ok);
    }
    if listed.is_empty() {
        println!(
            "  No interface sources. Add one with `rqtk interfaces add <alias> <url> --ref <ref>`."
        );
        return Ok(Exit::Ok);
    }
    for l in &listed {
        let commit = l
            .commit
            .as_deref()
            .map_or("not synced", vendor::short_commit);
        let count = l.requirements.to_string();
        output::section(&l.alias, "");
        for (k, v) in [
            ("git", l.git.as_str()),
            ("ref", l.reference.as_str()),
            ("commit", commit),
            ("requirements", count.as_str()),
        ] {
            println!("     {k:<14}{}", output::safe(v));
        }
    }
    Ok(Exit::Ok)
}
