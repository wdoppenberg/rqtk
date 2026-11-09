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
use crate::repository::{CONFIG_PATH, RequirementSet, load_config};
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
/// Inside the staging directory: the snapshots a sync is replacing, until it has finished.
const BACKUP_DIR: &str = ".previous";

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
    let config = load_config(&repo_root.join(CONFIG_PATH))?;
    let targets = targets(&config, options).map_err(RqtkError::Interface)?;
    let old_lock = Lockfile::read(repo_root)?.unwrap_or_default();

    let staging = repo_root.join(STAGING_DIR);
    let recovered = recover(repo_root, &staging, &old_lock)?;
    remove_dir(&staging)?;
    let result = stage_all(repo_root, &staging, &targets, &old_lock).and_then(|staged| {
        let mut lock = old_lock.clone();
        lock.version = LOCK_VERSION;
        for s in &staged {
            lock.sources.insert(s.alias.clone(), s.locked.clone());
        }
        swap_in(repo_root, &staging, &staged, &lock)?;
        Ok(staged
            .into_iter()
            .map(|s| {
                let mut warnings = s.warnings;
                if recovered.contains(&s.alias) {
                    warnings.insert(0, format!(
                        "an earlier sync of `{}` was interrupted; its previous snapshot was restored before syncing",
                        s.alias
                    ));
                }
                SyncedSource {
                    warnings,
                    requirements: s.locked.requirements.len(),
                    files: s.locked.files.len(),
                    alias: s.alias,
                    git: s.locked.git,
                    reference: s.locked.reference,
                    ref_kind: s.locked.ref_kind,
                    commit: s.locked.commit,
                    changed: s.changed,
                }
            })
            .collect())
    });
    let _ = std::fs::remove_dir_all(&staging);
    result
}

/// The sources `options` selects from `config`, after checking the alias is declared and
/// every URL is one rqtk fetches. The error is a message about the user's input.
pub fn targets(
    config: &Config,
    options: &SyncOptions,
) -> Result<Vec<(String, InterfaceSource)>, String> {
    let targets: Vec<(String, InterfaceSource)> = match &options.alias {
        Some(alias) => {
            let source = config
                .interfaces
                .get(alias)
                .ok_or_else(|| format!("no interface source `{alias}` is declared"))?;
            vec![(alias.clone(), source.clone())]
        }
        None => config
            .interfaces
            .iter()
            .map(|(a, s)| (a.clone(), s.clone()))
            .collect(),
    };
    for (alias, source) in &targets {
        check_url(&source.git, options.allow_file_url).map_err(|e| format!("`{alias}`: {e}"))?;
    }
    Ok(targets)
}

/// One alias, staged and ready to swap in.
struct Staged {
    alias: String,
    locked: LockedSource,
    /// Whether it differs from the snapshot it replaces.
    changed: bool,
    warnings: Vec<String>,
}

fn stage_all(
    repo_root: &Path,
    staging: &Path,
    targets: &[(String, InterfaceSource)],
    old: &Lockfile,
) -> Result<Vec<Staged>, RqtkError> {
    let mut staged = Vec::new();
    for (alias, source) in targets {
        let dir = staging.join(alias);
        let (locked, warnings) = stage(repo_root, alias, source, &dir)?;
        let changed = old.sources.get(alias) != Some(&locked);
        staged.push(Staged {
            alias: alias.clone(),
            locked,
            changed,
            warnings,
        });
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
            crate::vendor::short_commit(&commit.to_string())
        ));
    }
    let work = temp.path().join("work");
    let symlinks = extract(&repo, commit, &work).map_err(err)?;

    // The provider's format must be one this rqtk reads.
    let config_path = work.join(CONFIG_PATH);
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
        let mut referenced: Vec<&str> = interface.specs.iter().map(|s| s.path.as_str()).collect();
        referenced.extend(resolution.problems.iter().filter_map(|p| p.path.as_deref()));
        for path in referenced {
            if let Some(link) = symlinks.iter().find(|l| covers_link(path, l)) {
                return Err(err(format!(
                    "{id}: spec path `{path}` reaches symbolic link `{link}`; refusing to vendor it"
                )));
            }
        }
        if let Some(problem) = resolution.problems.first() {
            return Err(err(format!(
                "{id}: {} ({}); the provider's `rqtk lint` reports this too. Fix it there or pin another ref",
                problem.message, problem.code
            )));
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
            .map(|(id, _)| (id.0.clone(), statuses[id].status))
            .collect(),
    };
    let status_path = dir.join(STATUS_FILE);
    create_parent(&status_path)?;
    write(&status_path, toml::to_string_pretty(&status)?.as_bytes())?;

    // The snapshot must reproduce the provider's own hashes; anything else means a file it
    // needs was not vendored.
    let (vendored, _) = load_snapshot(alias, dir);
    for (id, v) in &vendored {
        if v.hash != set.content_hash(id) {
            return Err(err(format!(
                "{id}: the vendored copy does not reproduce the provider's content hash; refusing to sync an incomplete snapshot"
            )));
        }
    }
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

/// Whether spec `pattern` (a path or glob) could reach root-relative symbolic link `link`:
/// it names the link, runs through it as a directory, or is a glob whose literal directory
/// prefix contains it. Symbolic links are never written, so a spec that reaches one would
/// be vendored incomplete.
fn covers_link(pattern: &str, link: &str) -> bool {
    let Some(pattern) = crate::spec::normalize("", pattern) else {
        return false;
    };
    let prefix = if crate::spec::is_glob(&pattern) {
        let literal: Vec<&str> = pattern
            .split('/')
            .take_while(|part| !crate::spec::is_glob(part))
            .collect();
        let literal = literal.join("/");
        if literal.is_empty() || link.starts_with(&format!("{literal}/")) {
            return true;
        }
        literal
    } else {
        pattern
    };
    prefix == link || prefix.starts_with(&format!("{link}/"))
}

/// Undo what an interrupted [`swap_in`] left behind. A snapshot backed up in the staging
/// directory is restored unless the live snapshot already matches the lockfile, which
/// means the swap had finished. Returns the aliases that were restored.
fn recover(
    repo_root: &Path,
    staging: &Path,
    lock: &Lockfile,
) -> Result<BTreeSet<String>, RqtkError> {
    let mut restored = BTreeSet::new();
    let Ok(entries) = std::fs::read_dir(staging.join(BACKUP_DIR)) else {
        return Ok(restored);
    };
    for entry in entries.flatten() {
        let alias = entry.file_name().to_string_lossy().into_owned();
        let live = repo_root.join(VENDOR_DIR).join(&alias);
        let finished = live.is_dir()
            && lock
                .sources
                .get(&alias)
                .is_some_and(|l| l.files == hash_tree(&live));
        if finished {
            continue;
        }
        remove_dir(&live)?;
        create_parent(&live)?;
        rename(&entry.path(), &live)?;
        restored.insert(alias);
    }
    Ok(restored)
}

/// Move the staged snapshots into `.rqtk/vendor/` and write the lockfile, restoring the
/// previous snapshots if anything fails.
fn swap_in(
    repo_root: &Path,
    staging: &Path,
    staged: &[Staged],
    lock: &Lockfile,
) -> Result<(), RqtkError> {
    let vendor = repo_root.join(VENDOR_DIR);
    std::fs::create_dir_all(&vendor).map_err(|source| RqtkError::Io {
        path: vendor.clone(),
        source,
    })?;
    let backups = staging.join(BACKUP_DIR);
    std::fs::create_dir_all(&backups).map_err(|source| RqtkError::Io {
        path: backups.clone(),
        source,
    })?;
    let mut moved: Vec<&str> = Vec::new();
    let outcome = (|| {
        for Staged { alias, .. } in staged {
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
    if let Some(mut lock) = Lockfile::read(repo_root)?
        && lock.sources.remove(alias).is_some()
    {
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
        if name.is_empty()
            || name == "."
            || name == ".."
            || name.contains(['/', '\\'])
            || name.chars().any(char::is_control)
        {
            return Err(format!(
                "provider tree has an invalid file name `{}`",
                name.escape_debug()
            ));
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

#[cfg(test)]
mod tests {
    use super::*;

    fn locked(dir: &Path) -> LockedSource {
        LockedSource {
            git: "https://example.com/p".to_owned(),
            reference: "v1".to_owned(),
            ref_kind: RefKind::Tag,
            commit: "0".repeat(40),
            requirements: BTreeMap::new(),
            files: hash_tree(dir),
        }
    }

    fn put(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    // rqtk: verifies VA-CLI-0019-02
    #[test]
    fn a_failure_while_swapping_in_rolls_back() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path();
        let vendor = root.join(VENDOR_DIR);
        let staging = root.join(STAGING_DIR);
        put(&vendor.join("a/files/x.yaml"), "old a");
        put(&vendor.join("b/files/x.yaml"), "old b");
        put(&staging.join("a/files/x.yaml"), "new a");
        // `b` was never staged, so moving it in fails after `a` has been swapped.
        let staged = |alias: &str, dir: &Path| Staged {
            alias: alias.to_owned(),
            locked: locked(dir),
            changed: true,
            warnings: vec![],
        };
        let staged = [
            staged("a", &staging.join("a")),
            staged("b", &vendor.join("b")),
        ];
        let before = hash_tree(&vendor);
        assert!(swap_in(root, &staging, &staged, &Lockfile::default()).is_err());
        assert_eq!(hash_tree(&vendor), before);
        assert!(!Lockfile::path(root).exists());
    }

    // rqtk: verifies VA-CLI-0019-03
    #[test]
    fn an_interrupted_swap_is_undone_by_the_next_sync() {
        let root = tempfile::tempdir().unwrap();
        let root = root.path();
        let live = root.join(VENDOR_DIR).join("a");
        let backup = root.join(STAGING_DIR).join(BACKUP_DIR).join("a");
        put(&backup.join("files/x.yaml"), "old");
        let mut lock = Lockfile::default();
        lock.sources.insert("a".into(), locked(&backup));
        // Interrupted after the new snapshot went live but before the lockfile was written.
        put(&live.join("files/x.yaml"), "new");
        let restored = recover(root, &root.join(STAGING_DIR), &lock).unwrap();
        assert_eq!(restored, BTreeSet::from(["a".to_owned()]));
        assert_eq!(
            std::fs::read_to_string(live.join("files/x.yaml")).unwrap(),
            "old"
        );

        // Interrupted after the lockfile was written: the live snapshot is current.
        put(&backup.join("files/x.yaml"), "older");
        let restored = recover(root, &root.join(STAGING_DIR), &lock).unwrap();
        assert!(restored.is_empty());
        assert_eq!(
            std::fs::read_to_string(live.join("files/x.yaml")).unwrap(),
            "old"
        );
    }
}
