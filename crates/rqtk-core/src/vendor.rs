//! Vendored interface snapshots (`.rqtk/vendor/<alias>/`) and their lockfile
//! (`.rqtk/interfaces.lock`).
//!
//! `rqtk interfaces sync` writes a snapshot per alias: the provider's interface requirements
//! under `requirements/`, every file their specs match or reference under `files/`, and the
//! provider's verification status in `status.toml`. The lockfile records the commit each
//! alias resolved to and the hash of every vendored file. Everything here reads only those
//! committed files: nothing touches the network, and nothing vendored is executed.

use crate::diagnostic::Diagnostic;
use crate::model::{Config, Requirement, RequirementId};
use crate::repository::CONFIG_PATH;
use crate::spec::normalize_line_endings;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub const LOCK_PATH: &str = ".rqtk/interfaces.lock";
pub const VENDOR_DIR: &str = ".rqtk/vendor";
pub const LOCK_VERSION: u32 = 1;
/// Name of the status file inside a snapshot.
pub const STATUS_FILE: &str = "status.toml";

/// `.rqtk/interfaces.lock`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lockfile {
    pub version: u32,
    #[serde(default)]
    pub sources: BTreeMap<String, LockedSource>,
}

/// What one alias resolved to at its last sync.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LockedSource {
    pub git: String,
    #[serde(rename = "ref")]
    pub reference: String,
    pub ref_kind: RefKind,
    /// The commit `ref` resolved to.
    pub commit: String,
    /// Content hash of each vendored interface requirement, by ID.
    #[serde(default)]
    pub requirements: BTreeMap<String, String>,
    /// SHA-256 of each vendored file (line endings normalised to LF), by path relative to
    /// the alias's snapshot directory.
    #[serde(default)]
    pub files: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefKind {
    Tag,
    Branch,
    Commit,
}

impl Lockfile {
    pub fn path(repo_root: &Path) -> PathBuf {
        repo_root.join(LOCK_PATH)
    }

    /// The lockfile of the repository at `repo_root`, or `None` when there is none. A
    /// lockfile that exists but cannot be read is an error, never silently empty.
    pub fn read(repo_root: &Path) -> Result<Option<Self>, crate::error::RqtkError> {
        let path = Self::path(repo_root);
        match std::fs::read_to_string(&path) {
            Ok(text) => Self::parse(&text)
                .map(Some)
                .map_err(|e| Self::unreadable(&e)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(Self::unreadable(&e.to_string())),
        }
    }

    pub(crate) fn unreadable(reason: &str) -> crate::error::RqtkError {
        crate::error::RqtkError::Interface(format!("cannot read {LOCK_PATH}: {reason}"))
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|e| e.message().trim().to_owned())
    }

    pub fn to_toml(&self) -> String {
        let body = toml::to_string_pretty(self).expect("a lockfile serialises");
        format!(
            "# Written by `rqtk interfaces sync`; do not edit. Review vendored changes before committing them.\n{body}"
        )
    }
}

/// The provider's verification status of each vendored requirement (`status.toml`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderStatus {
    /// The provider commit the status was computed at.
    pub commit: String,
    /// Closure status by ID.
    #[serde(default)]
    pub requirements: BTreeMap<String, crate::status::ClosureStatus>,
}

/// One vendored interface requirement.
#[derive(Debug, Clone)]
pub struct VendoredRequirement {
    pub alias: String,
    pub requirement: Requirement,
    /// Its content hash, including its vendored spec files.
    pub hash: String,
    /// The provider's verification status at the pinned commit, if recorded.
    pub provider_status: Option<crate::status::ClosureStatus>,
    pub path: PathBuf,
}

impl VendoredRequirement {
    /// `<alias>:<ID>`.
    pub fn qualified_id(&self) -> String {
        format!("{}:{}", self.alias, self.requirement.id)
    }
}

/// Every vendored snapshot, as committed, and what is wrong with them.
#[derive(Debug, Clone, Default)]
pub struct Vendored {
    pub lock: Option<Lockfile>,
    /// Vendored requirements by alias, then ID.
    pub requirements: BTreeMap<String, BTreeMap<RequirementId, VendoredRequirement>>,
    pub diagnostics: Vec<Diagnostic>,
}

impl Vendored {
    pub fn get(&self, alias: &str, id: &RequirementId) -> Option<&VendoredRequirement> {
        self.requirements.get(alias)?.get(id)
    }

    /// Whether `alias` was synced (has a lockfile entry).
    pub fn is_synced(&self, alias: &str) -> bool {
        self.lock
            .as_ref()
            .is_some_and(|l| l.sources.contains_key(alias))
    }

    pub fn iter(&self) -> impl Iterator<Item = &VendoredRequirement> {
        self.requirements.values().flat_map(BTreeMap::values)
    }
}

/// Load the lockfile and every snapshot below `repo_root`, checking them against each other
/// and against `config`.
pub fn load(repo_root: &Path, config: &Config) -> Vendored {
    let mut out = Vendored::default();
    let lock_path = Lockfile::path(repo_root);
    let vendor_root = repo_root.join(VENDOR_DIR);
    match Lockfile::read(repo_root) {
        Ok(lock) => out.lock = lock,
        Err(e) => out
            .diagnostics
            .push(Diagnostic::new("RQ100", e.to_string()).file(&lock_path)),
    }
    let empty = Lockfile::default();
    let lock = out.lock.clone().unwrap_or(empty);

    // RQ041: config and lockfile must describe the same sources.
    for (alias, source) in &config.interfaces {
        match lock.sources.get(alias) {
            None => out.diagnostics.push(
                Diagnostic::new(
                    "RQ041",
                    format!(
                        "interface source `{alias}` has not been synced; run `rqtk interfaces sync {alias}`"
                    ),
                )
                .file(repo_root.join(CONFIG_PATH)),
            ),
            Some(locked) if locked.git != source.git || locked.reference != source.reference => {
                out.diagnostics.push(
                    Diagnostic::new(
                        "RQ041",
                        format!(
                            "interface source `{alias}` is configured as {} @ {} but the lockfile has {} @ {}; run `rqtk interfaces sync {alias}`",
                            source.git, source.reference, locked.git, locked.reference
                        ),
                    )
                    .file(&lock_path),
                )
            }
            Some(_) => {}
        }
    }
    let mut on_disk: BTreeSet<String> = BTreeSet::new();
    if let Ok(entries) = std::fs::read_dir(&vendor_root) {
        for entry in entries.flatten() {
            on_disk.insert(entry.file_name().to_string_lossy().into_owned());
        }
    }
    let orphans: BTreeSet<&String> = lock.sources.keys().chain(&on_disk).collect();
    for alias in orphans {
        if !config.interfaces.contains_key(alias) {
            out.diagnostics.push(
                Diagnostic::new(
                    "RQ041",
                    format!(
                        "`{alias}` is vendored but not declared under `[interfaces]`; run `rqtk interfaces remove {alias}` or declare it again"
                    ),
                )
                .file(vendor_root.join(alias)),
            );
        }
    }

    for (alias, locked) in &lock.sources {
        if locked.ref_kind == RefKind::Branch {
            out.diagnostics.push(
                Diagnostic::new(
                    "RQ042",
                    format!(
                        "interface source `{alias}` pins branch `{}`, which moves; pin a tag or commit SHA",
                        locked.reference
                    ),
                )
                .file(repo_root.join(CONFIG_PATH)),
            );
        }
        let dir = vendor_root.join(alias);
        out.diagnostics.extend(check_integrity(alias, &dir, locked));
        let (requirements, diagnostics) = load_snapshot(alias, &dir);
        out.diagnostics.extend(diagnostics);
        out.requirements.insert(alias.clone(), requirements);
    }
    out
}

/// RQ040: the files of a snapshot must be exactly those the lockfile lists, with their hashes.
fn check_integrity(alias: &str, dir: &Path, locked: &LockedSource) -> Vec<Diagnostic> {
    let mut issues = Vec::new();
    let actual = hash_tree(dir);
    for (path, hash) in &locked.files {
        match actual.get(path) {
            None => issues.push(
                Diagnostic::new(
                    "RQ040",
                    format!("vendored file `{path}` of `{alias}` is missing"),
                )
                .file(dir.join(path)),
            ),
            Some(found) if found != hash => issues.push(
                Diagnostic::new(
                    "RQ040",
                    format!(
                        "vendored file `{path}` of `{alias}` differs from the lockfile; vendored content must not be edited"
                    ),
                )
                .file(dir.join(path)),
            ),
            Some(_) => {}
        }
    }
    for path in actual.keys() {
        if !locked.files.contains_key(path) {
            issues.push(
                Diagnostic::new(
                    "RQ040",
                    format!("vendored file `{path}` of `{alias}` is not in the lockfile"),
                )
                .file(dir.join(path)),
            );
        }
    }
    issues
}

/// The vendored requirements of one snapshot, checked only against rqtk's built-in schema.
pub(crate) fn load_snapshot(
    alias: &str,
    dir: &Path,
) -> (
    BTreeMap<RequirementId, VendoredRequirement>,
    Vec<Diagnostic>,
) {
    let mut out = BTreeMap::new();
    let mut issues = Vec::new();
    let status: ProviderStatus = std::fs::read_to_string(dir.join(STATUS_FILE))
        .ok()
        .and_then(|t| toml::from_str(&t).ok())
        .unwrap_or_default();
    let files_root = dir.join("files");
    let Ok(entries) = std::fs::read_dir(dir.join("requirements")) else {
        return (out, issues);
    };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }
        let parsed = std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|t| {
                toml::from_str::<Requirement>(&t).map_err(|e| e.message().trim().to_owned())
            });
        let requirement = match parsed {
            Ok(r) => r,
            Err(e) => {
                issues.push(
                    Diagnostic::new(
                        "RQ043",
                        format!("vendored requirement of `{alias}` cannot be read: {e}"),
                    )
                    .file(&path),
                );
                continue;
            }
        };
        let digest = requirement
            .interface
            .as_ref()
            .map(|i| crate::spec::resolve(&crate::spec::DirFiles::snapshot(&files_root), i).digest);
        let hash = requirement.compute_content_hash(digest.as_deref());
        let provider_status = status.requirements.get(&requirement.id.0).copied();
        out.insert(
            requirement.id.clone(),
            VendoredRequirement {
                alias: alias.to_owned(),
                requirement,
                hash,
                provider_status,
                path,
            },
        );
    }
    (out, issues)
}

/// Add `[interfaces.<alias>]` with `git` and `ref` to `.rqtk/config.toml`, keeping the
/// rest of the file as it is.
pub fn declare_source(
    repo_root: &Path,
    alias: &str,
    source: &crate::model::InterfaceSource,
) -> Result<(), crate::error::RqtkError> {
    crate::io::edit_toml_file(&repo_root.join(CONFIG_PATH), |doc| {
        let interfaces = doc
            .entry("interfaces")
            .or_insert_with(|| {
                let mut t = toml_edit::Table::new();
                t.set_implicit(true);
                toml_edit::Item::Table(t)
            })
            .as_table_mut()
            .expect("`interfaces` is a table in a parsed config");
        let mut table = toml_edit::Table::new();
        table.insert("git", toml_edit::value(&source.git));
        table.insert("ref", toml_edit::value(&source.reference));
        interfaces.insert(alias, toml_edit::Item::Table(table));
    })?;
    Ok(())
}

/// Remove `[interfaces.<alias>]` from `.rqtk/config.toml`. Returns whether it was there.
pub fn undeclare_source(repo_root: &Path, alias: &str) -> Result<bool, crate::error::RqtkError> {
    let mut removed = false;
    crate::io::edit_toml_file(&repo_root.join(CONFIG_PATH), |doc| {
        if let Some(interfaces) = doc.get_mut("interfaces").and_then(|i| i.as_table_mut()) {
            removed = interfaces.remove(alias).is_some();
            if interfaces.is_empty() {
                doc.remove("interfaces");
            }
        }
    })?;
    Ok(removed)
}

/// Whether `alias` can name an interface source: letters, digits, `-` and `_`.
pub fn valid_alias(alias: &str) -> bool {
    !alias.is_empty()
        && alias
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Accept only `https://` and SSH (`ssh://…` or `user@host:path`) git URLs, plus `file://`
/// URLs and local paths when `allow_file` is set. Every other transport is refused: git's
/// `ext::` runs commands, and `git://` and `http://` are unauthenticated.
pub fn check_url(url: &str, allow_file: bool) -> Result<(), String> {
    let lower = url.to_ascii_lowercase();
    let refuse = |why: &str| {
        Err(format!(
            "refusing git URL `{url}`: {why}; use an https:// or ssh:// URL"
        ))
    };
    if lower.starts_with("https://") || lower.starts_with("ssh://") {
        return Ok(());
    }
    if lower.starts_with("file://") || (!url.contains(':') && !url.is_empty()) {
        return if allow_file {
            Ok(())
        } else {
            refuse("local repositories need --allow-file-url")
        };
    }
    if url.contains("::") {
        return refuse("the `<transport>::` form can run arbitrary commands");
    }
    if let Some((scheme, _)) = url.split_once("://") {
        return refuse(&format!("the `{scheme}` transport is not allowed"));
    }
    // scp-like `[user@]host:path`, but not a Windows drive (`C:\…`).
    let (host, _) = url.split_once(':').expect("contains ':'");
    let looks_like_drive = host.len() == 1 && host.chars().all(|c| c.is_ascii_alphabetic());
    if !host.is_empty() && !host.contains('/') && !looks_like_drive {
        return Ok(());
    }
    if looks_like_drive && allow_file {
        return Ok(());
    }
    refuse("unrecognised URL")
}

/// The first 12 characters of a provider commit SHA, as rqtk prints it.
pub fn short_commit(sha: &str) -> &str {
    &sha[..sha.len().min(12)]
}

/// SHA-256 of `bytes` with line endings normalised to LF, as recorded in the lockfile.
pub fn file_hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(normalize_line_endings(bytes)))
}

/// Hash of every regular file below `dir`, by `/`-separated relative path. Symbolic links
/// are not followed; they are listed with an empty hash so they never match the lockfile.
pub fn hash_tree(dir: &Path) -> BTreeMap<String, String> {
    fn walk(dir: &Path, rel: &str, out: &mut BTreeMap<String, String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let rel = if rel.is_empty() {
                name
            } else {
                format!("{rel}/{name}")
            };
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                walk(&entry.path(), &rel, out);
            } else if kind.is_file() {
                let hash = std::fs::read(entry.path())
                    .map(|b| file_hash(&b))
                    .unwrap_or_default();
                out.insert(rel, hash);
            } else {
                out.insert(rel, String::new());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, "", &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // rqtk: verifies VA-CLI-0018-01
    #[test]
    fn only_https_and_ssh_transports_are_accepted() {
        for ok in [
            "https://github.com/acme/api",
            "ssh://git@github.com/acme/api.git",
            "git@github.com:acme/api.git",
        ] {
            assert!(check_url(ok, false).is_ok(), "{ok}");
        }
        for bad in [
            "ext::sh -c touch% /tmp/pwned",
            "git://example.com/repo",
            "http://example.com/repo",
            "fd::17",
            "file:///srv/repo",
            "/srv/repo",
        ] {
            assert!(check_url(bad, false).is_err(), "{bad}");
        }
    }

    // rqtk: verifies VA-CLI-0018-02
    #[test]
    fn file_urls_need_an_explicit_flag() {
        assert!(check_url("file:///srv/repo", false).is_err());
        assert!(check_url("file:///srv/repo", true).is_ok());
        assert!(check_url("../provider", true).is_ok());
        assert!(check_url("ext::sh -c x", true).is_err());
    }
}
