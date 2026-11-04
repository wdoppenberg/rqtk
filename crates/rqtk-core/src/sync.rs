//! `rqtk interfaces sync`: fetch provider repositories and vendor their interface
//! requirements.
//!
//! For each alias, the provider is cloned into a temporary directory, the pinned ref is
//! resolved to a commit, and that commit's tree is written out (without symbolic links) so
//! its `.rqtk/` can be loaded and its verification status computed. Only the interface
//! requirements and the files their specs match or reference are vendored. All aliases are
//! staged first and swapped in at the end, so a failed sync leaves `.rqtk/vendor/` and the
//! lockfile as they were.
//!
//! Provider content is data: it is parsed as TOML, OpenAPI or hashed, never executed.

use crate::error::RqtkError;
use crate::model::{Config, InterfaceSource, RequirementId, SCHEMA_VERSION};
use crate::repository::{RequirementSet, load_config};
use crate::vendor::{
    LOCK_VERSION, LockedSource, Lockfile, ProviderStatus, RefKind, STATUS_FILE, VENDOR_DIR,
    check_url, hash_tree, load_snapshot,
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Largest file `sync` vendors.
pub const MAX_FILE_BYTES: u64 = 10 * 1024 * 1024;
/// Largest total size of one alias's vendored files.
pub const MAX_ALIAS_BYTES: u64 = 100 * 1024 * 1024;

const STAGING_DIR: &str = ".rqtk/.sync-staging";

#[derive(Debug, Clone, Default)]
pub struct SyncOptions {
    /// Sync only this alias; all declared aliases when `None`.
    pub alias: Option<String>,
    /// Accept `file://` URLs and local paths.
    pub allow_file_url: bool,
}

/// What one alias was synced to.
#[derive(Debug, Clone, Serialize)]
pub struct SyncedSource {
    pub alias: String,
    pub git: String,
    #[serde(rename = "ref")]
    pub reference: String,
    pub ref_kind: RefKind,
    pub commit: String,
    pub requirements: usize,
    pub files: usize,
    /// Whether the snapshot differs from the one it replaced.
    pub changed: bool,
    /// Things worth a second look that did not stop the sync.
    pub warnings: Vec<String>,
}

/// Sync the declared interface sources of the repository at `repo_root`.
pub fn sync(repo_root: &Path, options: &SyncOptions) -> Result<Vec<SyncedSource>, RqtkError> {
    let config = load_config(&repo_root.join(".rqtk/config.toml"))?;
    let targets = targets(&config, options)?;
    let lock_path = Lockfile::path(repo_root);
    let old_lock = match std::fs::read_to_string(&lock_path) {
        Ok(text) => Lockfile::parse(&text).map_err(|e| {
            RqtkError::Interface(format!("cannot read {}: {e}", lock_path.display()))
        })?,
        Err(_) => Lockfile::default(),
    };

    let staging = repo_root.join(STAGING_DIR);
    remove_dir(&staging)?;
    let result = stage_all(repo_root, &staging, &targets, &old_lock).and_then(|staged| {
        let mut lock = old_lock.clone();
        lock.version = LOCK_VERSION;
        for (alias, locked, _, _) in &staged {
            lock.sources.insert(alias.clone(), locked.clone());
        }
        swap_in(repo_root, &staging, &staged, &lock)?;
        Ok(staged
            .into_iter()
            .map(|(alias, locked, changed, warnings)| SyncedSource {
                warnings,
                requirements: locked.requirements.len(),
                files: locked.files.len(),
                alias,
                git: locked.git,
                reference: locked.reference,
                ref_kind: locked.ref_kind,
                commit: locked.commit,
                changed,
            })
            .collect())
    });
    let _ = std::fs::remove_dir_all(&staging);
    result
}

fn targets(
    config: &Config,
    options: &SyncOptions,
) -> Result<Vec<(String, InterfaceSource)>, RqtkError> {
    let targets: Vec<(String, InterfaceSource)> = match &options.alias {
        Some(alias) => {
            let source = config.interfaces.get(alias).ok_or_else(|| {
                RqtkError::Interface(format!("no interface source `{alias}` is declared"))
            })?;
            vec![(alias.clone(), source.clone())]
        }
        None => config
            .interfaces
            .iter()
            .map(|(a, s)| (a.clone(), s.clone()))
            .collect(),
    };
    for (alias, source) in &targets {
        check_url(&source.git, options.allow_file_url)
            .map_err(|e| RqtkError::Interface(format!("`{alias}`: {e}")))?;
    }
    Ok(targets)
}

type Staged = Vec<(String, LockedSource, bool, Vec<String>)>;

fn stage_all(
    repo_root: &Path,
    staging: &Path,
    targets: &[(String, InterfaceSource)],
    old: &Lockfile,
) -> Result<Staged, RqtkError> {
    let mut staged = Vec::new();
    for (alias, source) in targets {
        let dir = staging.join(alias);
        let (locked, warnings) = stage(repo_root, alias, source, &dir)?;
        let changed = old.sources.get(alias) != Some(&locked);
        staged.push((alias.clone(), locked, changed, warnings));
    }
    Ok(staged)
}

/// Fetch `source` and write its snapshot into `dir`.
fn stage(
    repo_root: &Path,
    alias: &str,
    source: &InterfaceSource,
    dir: &Path,
) -> Result<(LockedSource, Vec<String>), RqtkError> {
    let err = |msg: String| RqtkError::Interface(format!("`{alias}`: {msg}"));
    let temp = tempfile::tempdir()
        .map_err(|e| err(format!("cannot create a temporary directory: {e}")))?;
    let repo = fetch(&source.git, &temp.path().join("repo.git")).map_err(err)?;
    let (commit, ref_kind) = resolve_ref(&repo, &source.reference).map_err(err)?;
    let mut warnings = Vec::new();
    if in_own_history(repo_root, commit) {
        warnings.push(format!(
            "`{alias}` resolves to commit {} that is already in this repository's history: it points at this repository or a fork of it, which is almost always a mistake",
            &commit.to_string()[..12]
        ));
    }
    let work = temp.path().join("work");
    let symlinks = extract(&repo, commit, &work).map_err(err)?;

    // The provider's format must be one this rqtk reads.
    let config_path = work.join(".rqtk/config.toml");
    if !config_path.is_file() {
        return Err(err(format!(
            "{} has no .rqtk/config.toml at {}; it is not an rqtk repository",
            source.git, source.reference
        )));
    }
    let provider_config = match load_config(&config_path) {
        Err(RqtkError::UnsupportedSchemaVersion {
            found, supported, ..
        }) => {
            return Err(err(format!(
                "the provider uses schema_version {found}, but this rqtk supports schema_version {supported}; upgrade rqtk or pin an older ref"
            )));
        }
        other => other?,
    };
    debug_assert_eq!(SCHEMA_VERSION, 1);
    // git does not store empty directories; a provider without requirements has none.
    let requirements_root = work.join(&provider_config.repository.requirements_dir);
    std::fs::create_dir_all(&requirements_root).map_err(|source| RqtkError::Io {
        path: requirements_root.clone(),
        source,
    })?;

    let (set, _) = RequirementSet::load_from_repo_root(&work)?.validate();
    let requirements_dir = set
        .requirements_dir()
        .strip_prefix(&work)
        .unwrap_or(Path::new(""))
        .to_path_buf();
    for link in &symlinks {
        if Path::new(link).starts_with(&requirements_dir) && link.ends_with(".toml") {
            return Err(err(format!(
                "provider requirement file `{link}` is a symbolic link; refusing to vendor it"
            )));
        }
    }

    // Collect what to vendor: interface requirement files and their spec files.
    let mut copies: BTreeMap<String, PathBuf> = BTreeMap::new();
    for (id, req) in set.requirements() {
        let Some(interface) = &req.interface else {
            continue;
        };
        let resolution = set.spec_resolution(id).expect("resolved at load");
        if let Some(problem) = resolution.problems.iter().find(|p| p.escapes()) {
            return Err(err(format!("{id}: {}; refusing to sync", problem.message)));
        }
        let mut referenced: Vec<&str> = interface.specs.iter().map(|s| s.path.as_str()).collect();
        referenced.extend(resolution.problems.iter().filter_map(|p| p.path.as_deref()));
        for path in referenced {
            if let Some(link) = symlinks.iter().find(|l| matches_path(path, l)) {
                return Err(err(format!(
                    "{id}: spec file `{link}` is a symbolic link; refusing to vendor it"
                )));
            }
        }
        let source_file = set
            .path_of(&crate::model::EntityRef::Requirement(id.clone()))
            .expect("loaded");
        copies.insert(format!("requirements/{id}.toml"), source_file.to_path_buf());
        for file in &resolution.files {
            copies.insert(format!("files/{file}"), work.join(file));
        }
    }

    let mut total = 0u64;
    for (dest, src) in &copies {
        let size = std::fs::metadata(src).map(|m| m.len()).unwrap_or(0);
        if size > MAX_FILE_BYTES {
            return Err(err(format!(
                "`{dest}` is {} MiB, over the {} MiB limit for a vendored file",
                size / (1024 * 1024),
                MAX_FILE_BYTES / (1024 * 1024)
            )));
        }
        total += size;
        if total > MAX_ALIAS_BYTES {
            return Err(err(format!(
                "vendored content exceeds the {} MiB limit per interface source",
                MAX_ALIAS_BYTES / (1024 * 1024)
            )));
        }
    }
    for (dest, src) in &copies {
        let target = dir.join(dest);
        create_parent(&target)?;
        std::fs::copy(src, &target).map_err(|source| RqtkError::Io {
            path: target.clone(),
            source,
        })?;
    }

    // The provider's verification status at this commit.
    let links = crate::scan::scan(&work, &provider_config.scan)?;
    let evidence = crate::evidence::Evidence::load(&work)?;
    let statuses = set.verification_status(&links, &evidence);
    let status = ProviderStatus {
        commit: commit.to_string(),
        requirements: set
            .requirements()
            .iter()
            .filter(|(_, r)| r.interface.is_some())
            .map(|(id, _)| {
                let s = serde_json::to_value(statuses[id].status)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default();
                (id.0.clone(), s)
            })
            .collect(),
    };
    let status_path = dir.join(STATUS_FILE);
    create_parent(&status_path)?;
    write(&status_path, toml::to_string_pretty(&status)?.as_bytes())?;

    let (vendored, _) = load_snapshot(alias, dir);
    let requirements: BTreeMap<String, String> = vendored
        .into_iter()
        .map(|(id, v): (RequirementId, _)| (id.0, v.hash))
        .collect();
    let locked = LockedSource {
        git: source.git.clone(),
        reference: source.reference.clone(),
        ref_kind,
        commit: commit.to_string(),
        requirements,
        files: hash_tree(dir),
    };
    Ok((locked, warnings))
}

/// Whether `commit` is in the object database of the repository containing `repo_root`.
fn in_own_history(repo_root: &Path, commit: gix::ObjectId) -> bool {
    gix::discover(repo_root).is_ok_and(|repo| repo.has_object(commit))
}

/// Whether spec `pattern` (a path or glob) names root-relative `path`.
fn matches_path(pattern: &str, path: &str) -> bool {
    let Some(pattern) = crate::spec::normalize("", pattern) else {
        return false;
    };
    pattern == path
        || globset::GlobBuilder::new(&pattern)
            .literal_separator(true)
            .build()
            .is_ok_and(|g| g.compile_matcher().is_match(path))
}

/// Move the staged snapshots into `.rqtk/vendor/` and write the lockfile, restoring the
/// previous snapshots if anything fails.
fn swap_in(
    repo_root: &Path,
    staging: &Path,
    staged: &Staged,
    lock: &Lockfile,
) -> Result<(), RqtkError> {
    let vendor = repo_root.join(VENDOR_DIR);
    std::fs::create_dir_all(&vendor).map_err(|source| RqtkError::Io {
        path: vendor.clone(),
        source,
    })?;
    let backups = staging.join(".previous");
    std::fs::create_dir_all(&backups).map_err(|source| RqtkError::Io {
        path: backups.clone(),
        source,
    })?;
    let mut moved: Vec<&str> = Vec::new();
    let outcome = (|| {
        for (alias, _, _, _) in staged {
            let live = vendor.join(alias);
            if live.exists() {
                rename(&live, &backups.join(alias))?;
            }
            moved.push(alias);
            rename(&staging.join(alias), &live)?;
        }
        let lock_path = Lockfile::path(repo_root);
        let tmp = staging.join("interfaces.lock");
        write(&tmp, lock.to_toml().as_bytes())?;
        rename(&tmp, &lock_path)
    })();
    if outcome.is_err() {
        for alias in moved {
            let live = vendor.join(alias);
            let _ = std::fs::remove_dir_all(&live);
            if backups.join(alias).exists() {
                let _ = std::fs::rename(backups.join(alias), &live);
            }
        }
    }
    outcome
}

/// Remove the alias from the lockfile and delete its snapshot.
pub fn remove(repo_root: &Path, alias: &str) -> Result<bool, RqtkError> {
    let lock_path = Lockfile::path(repo_root);
    let mut removed = false;
    if let Ok(text) = std::fs::read_to_string(&lock_path) {
        let mut lock = Lockfile::parse(&text).map_err(|e| {
            RqtkError::Interface(format!("cannot read {}: {e}", lock_path.display()))
        })?;
        if lock.sources.remove(alias).is_some() {
            removed = true;
            if lock.sources.is_empty() {
                std::fs::remove_file(&lock_path).map_err(|source| RqtkError::Io {
                    path: lock_path.clone(),
                    source,
                })?;
            } else {
                write(&lock_path, lock.to_toml().as_bytes())?;
            }
        }
    }
    let dir = repo_root.join(VENDOR_DIR).join(alias);
    if dir.exists() {
        removed = true;
        remove_dir(&dir)?;
        let vendor = repo_root.join(VENDOR_DIR);
        if std::fs::read_dir(&vendor).is_ok_and(|mut d| d.next().is_none()) {
            let _ = std::fs::remove_dir(&vendor);
        }
    }
    Ok(removed)
}

// ── git ─────────────────────────────────────────────────────────────────────

fn fetch(url: &str, path: &Path) -> Result<gix::Repository, String> {
    let mut prepare =
        gix::prepare_clone_bare(url, path).map_err(|e| format!("cannot clone {url}: {e}"))?;
    let (repo, _) = prepare
        .fetch_only(
            gix::progress::Discard,
            &std::sync::atomic::AtomicBool::new(false),
        )
        .map_err(|e| format!("cannot fetch {url}: {e}"))?;
    Ok(repo)
}

/// The commit `reference` names, and whether it is a tag, branch or commit SHA.
fn resolve_ref(
    repo: &gix::Repository,
    reference: &str,
) -> Result<(gix::ObjectId, RefKind), String> {
    let peel = |mut r: gix::Reference<'_>| -> Result<gix::ObjectId, String> {
        let id = r.peel_to_id().map_err(|e| e.to_string())?.detach();
        commit_of(repo, id)
    };
    let tags = [
        format!("refs/tags/{reference}"),
        format!("refs/tags/{}{reference}", crate::git::BASELINE_TAG_PREFIX),
    ];
    for name in &tags {
        if let Ok(Some(r)) = repo.try_find_reference(name.as_str()) {
            return Ok((peel(r)?, RefKind::Tag));
        }
    }
    for name in [
        format!("refs/heads/{reference}"),
        format!("refs/remotes/origin/{reference}"),
    ] {
        if let Ok(Some(r)) = repo.try_find_reference(name.as_str()) {
            return Ok((peel(r)?, RefKind::Branch));
        }
    }
    let is_sha = reference.len() >= 7 && reference.chars().all(|c| c.is_ascii_hexdigit());
    if is_sha && let Ok(id) = repo.rev_parse_single(reference) {
        return Ok((commit_of(repo, id.detach())?, RefKind::Commit));
    }
    Err(format!(
        "ref `{reference}` is not a tag, branch or commit of the provider"
    ))
}

fn commit_of(repo: &gix::Repository, id: gix::ObjectId) -> Result<gix::ObjectId, String> {
    let object = repo.find_object(id).map_err(|e| e.to_string())?;
    Ok(object.peel_to_commit().map_err(|e| e.to_string())?.id)
}

/// Write the tree of `commit` below `dest`. Symbolic links and submodules are not written;
/// the paths of symbolic links are returned.
fn extract(
    repo: &gix::Repository,
    commit: gix::ObjectId,
    dest: &Path,
) -> Result<BTreeSet<String>, String> {
    let tree = repo
        .find_object(commit)
        .map_err(|e| e.to_string())?
        .peel_to_commit()
        .map_err(|e| e.to_string())?
        .tree()
        .map_err(|e| e.to_string())?;
    let mut symlinks = BTreeSet::new();
    write_tree(repo, tree, dest, "", &mut symlinks)?;
    Ok(symlinks)
}

fn write_tree(
    repo: &gix::Repository,
    tree: gix::Tree<'_>,
    dest: &Path,
    prefix: &str,
    symlinks: &mut BTreeSet<String>,
) -> Result<(), String> {
    use gix::objs::tree::EntryKind;
    std::fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    for entry in tree.iter() {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.filename().to_string();
        if name.is_empty() || name == "." || name == ".." || name.contains(['/', '\\', '\0']) {
            return Err(format!("provider tree has an invalid file name `{name}`"));
        }
        let rel = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        let oid = entry.oid().to_owned();
        match entry.mode().kind() {
            EntryKind::Tree => {
                let sub = repo
                    .find_object(oid)
                    .map_err(|e| e.to_string())?
                    .into_tree();
                write_tree(repo, sub, &dest.join(&name), &rel, symlinks)?;
            }
            EntryKind::Blob | EntryKind::BlobExecutable => {
                let blob = repo.find_object(oid).map_err(|e| e.to_string())?;
                std::fs::write(dest.join(&name), &blob.data).map_err(|e| e.to_string())?;
            }
            EntryKind::Link => {
                symlinks.insert(rel);
            }
            EntryKind::Commit => {}
        }
    }
    Ok(())
}

// ── files ───────────────────────────────────────────────────────────────────

fn create_parent(path: &Path) -> Result<(), RqtkError> {
    let parent = path.parent().expect("a file path has a parent");
    std::fs::create_dir_all(parent).map_err(|source| RqtkError::Io {
        path: parent.to_path_buf(),
        source,
    })
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), RqtkError> {
    std::fs::write(path, bytes).map_err(|source| RqtkError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn rename(from: &Path, to: &Path) -> Result<(), RqtkError> {
    std::fs::rename(from, to).map_err(|source| RqtkError::Io {
        path: to.to_path_buf(),
        source,
    })
}

fn remove_dir(path: &Path) -> Result<(), RqtkError> {
    match std::fs::remove_dir_all(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(RqtkError::Io {
            path: path.to_path_buf(),
            source: e,
        }),
        _ => Ok(()),
    }
}
