//! Repositories opened from git URLs, fetched into a per-user cache.

use crate::error::{Error, ErrorCode, Result};
use crate::repository::{Origin, Repository};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Options for [`Repository::open_remote_with`].
#[derive(Debug, Clone, Default)]
pub struct RemoteOptions {
    /// Also accept `file://` URLs and local paths. Off by default: only https and SSH.
    pub allow_file_url: bool,
}

/// Where fetched repositories are kept: `RQTK_CACHE_DIR` when set, otherwise
/// `~/.cache/rqtk/repos` on Linux (`$XDG_CACHE_HOME/rqtk/repos` when that is set),
/// `~/Library/Caches/rqtk/repos` on macOS and `%LOCALAPPDATA%\rqtk\cache\repos` on Windows.
pub fn cache_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("RQTK_CACHE_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(dir);
    }
    let home = || {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default()
    };
    if cfg!(windows) {
        let base = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| home().join("AppData").join("Local"));
        return base.join("rqtk").join("cache").join("repos");
    }
    if cfg!(target_os = "macos") {
        return home().join("Library/Caches/rqtk/repos");
    }
    std::env::var_os("XDG_CACHE_HOME")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".cache"))
        .join("rqtk/repos")
}

/// Delete every cached repository. Nothing else ever deletes them.
pub fn clear_cache() -> Result<()> {
    match std::fs::remove_dir_all(cache_dir()) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(Error::new(
            ErrorCode::Io,
            format!("cannot clear {}: {e}", cache_dir().display()),
        )),
        _ => Ok(()),
    }
}

/// The cache directory for `url`: a readable name plus a hash, so different URLs never share.
fn cached_path(url: &str) -> PathBuf {
    let digest = format!("{:x}", Sha256::digest(url.as_bytes()));
    let name: String = url
        .trim_end_matches('/')
        .trim_end_matches(".git")
        .rsplit(['/', ':', '\\'])
        .next()
        .unwrap_or("repo")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(40)
        .collect();
    cache_dir().join(format!("{name}-{}.git", &digest[..16]))
}

pub(crate) fn open_remote(url: &str, reference: &str, allow_file_url: bool) -> Result<Repository> {
    rqtk_core::vendor::check_url(url, allow_file_url)
        .map_err(|e| Error::new(ErrorCode::RemoteRefused, e))?;
    let repo = rqtk_core::sync::fetch_cached(url, &cached_path(url))
        .map_err(|e| Error::new(ErrorCode::RemoteFailed, e))?;
    let (commit, _) = rqtk_core::sync::resolve_ref(&repo, reference)
        .map_err(|e| Error::new(ErrorCode::RefNotFound, e))?;
    Ok(Repository::from_commit(
        repo,
        commit,
        Path::new(""),
        Origin::Remote {
            url: url.to_owned(),
            reference: reference.to_owned(),
        },
    )?
    .with_options(allow_file_url))
}

impl Repository {
    /// Open the repository at an https or SSH git `url` as it is at `reference` (a tag,
    /// baseline name, branch or commit). Authenticates as git would: credential helpers, the
    /// SSH agent, `~/.ssh`. The fetched copy is kept in [`cache_dir`] and fetched again on
    /// the next open.
    pub fn open_remote(url: &str, reference: &str) -> Result<Self> {
        open_remote(url, reference, false)
    }

    /// [`Repository::open_remote`] with options.
    pub fn open_remote_with(url: &str, reference: &str, options: &RemoteOptions) -> Result<Self> {
        open_remote(url, reference, options.allow_file_url)
    }
}
