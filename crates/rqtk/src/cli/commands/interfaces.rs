//! `rqtk interfaces add | sync | remove | list`: consume interface requirements published
//! by other repositories.

use std::error::Error;

use rqtk_core::sync::{SyncOptions, SyncedSource};
use rqtk_core::vendor::{self, LOCK_PATH, Lockfile, VENDOR_DIR};
use rqtk_core::{InterfaceSource, load_config};
use serde::Serialize;

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
    ctx.root.join(".rqtk/config.toml")
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

pub fn add(ctx: &Ctx, args: AddArgs) -> Result<Exit, Box<dyn Error>> {
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
        output::json(&AddReport {
            alias: &args.alias,
            git: &args.url,
            reference: &args.reference,
            written: !args.dry_run,
            warning: UNTRUSTED_WARNING,
        })?;
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

pub fn sync(ctx: &Ctx, args: SyncArgs) -> Result<Exit, Box<dyn Error>> {
    warn(ctx);
    let config = load_config(&config_path(ctx))?;
    if let Some(alias) = &args.alias
        && !config.interfaces.contains_key(alias)
    {
        return Err(Usage(format!("no interface source `{alias}` is declared")).into());
    }
    for (alias, source) in &config.interfaces {
        if args.alias.as_ref().is_none_or(|a| a == alias) {
            vendor::check_url(&source.git, args.allow_file_url)
                .map_err(|e| Usage(format!("`{alias}`: {e}")))?;
        }
    }
    let sources = rqtk_core::sync::sync(
        &ctx.root,
        &SyncOptions {
            alias: args.alias.clone(),
            allow_file_url: args.allow_file_url,
        },
    )?;
    if ctx.json() {
        output::json(&SyncReport {
            sources: &sources,
            warning: UNTRUSTED_WARNING,
        })?;
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
        let commit = &s.commit[..s.commit.len().min(12)];
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

pub fn remove(ctx: &Ctx, alias: &str) -> Result<Exit, Box<dyn Error>> {
    let declared = vendor::undeclare_source(&ctx.root, alias)?;
    let vendored = rqtk_core::sync::remove(&ctx.root, alias)?;
    if !declared && !vendored {
        return Err(Usage(format!(
            "no interface source `{alias}` is declared or vendored"
        ))
        .into());
    }
    if ctx.json() {
        output::json(&RemoveReport {
            alias,
            removed: true,
        })?;
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
struct Listed {
    alias: String,
    git: String,
    #[serde(rename = "ref")]
    reference: String,
    commit: Option<String>,
    requirements: usize,
}

pub fn list(ctx: &Ctx) -> Result<Exit, Box<dyn Error>> {
    let config = load_config(&config_path(ctx))?;
    let lock = std::fs::read_to_string(Lockfile::path(&ctx.root))
        .ok()
        .and_then(|t| Lockfile::parse(&t).ok())
        .unwrap_or_default();
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
        output::json(&listed)?;
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
            .map_or("not synced".to_owned(), |c| c[..c.len().min(12)].to_owned());
        let count = l.requirements.to_string();
        output::section(&l.alias, "");
        for (k, v) in [
            ("git", l.git.as_str()),
            ("ref", l.reference.as_str()),
            ("commit", commit.as_str()),
            ("requirements", count.as_str()),
        ] {
            println!("     {k:<14}{v}");
        }
    }
    Ok(Exit::Ok)
}
