use crate::error::RqtkError;
use crate::model::{Requirement, RequirementId};
use chrono::{DateTime, Utc};
use gix::refs::transaction::PreviousValue;
use serde::de::DeserializeOwned;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::str::FromStr;

/// The git library rqtk uses, for clients that hand repositories and object IDs around.
pub use gix;

pub const BASELINE_TAG_PREFIX: &str = "rqtk/";

/// A validated baseline version name: non-empty, no `/`, no whitespace.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BaselineName(String);

impl BaselineName {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Construct without validation — for names sourced from git refs we wrote ourselves.
    pub(crate) fn from_trusted(s: impl Into<String>) -> Self {
        Self(s.into())
    }
}

impl FromStr for BaselineName {
    type Err = RqtkError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() || s.contains('/') || s.contains(char::is_whitespace) {
            return Err(RqtkError::InvalidBaselineName(s.to_owned()));
        }
        Ok(Self(s.to_owned()))
    }
}

impl std::fmt::Display for BaselineName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for BaselineName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// A full commit SHA with an accessor for the short (8-char) prefix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitHash(String);

impl CommitHash {
    pub fn full(&self) -> &str {
        &self.0
    }

    pub fn short(&self) -> &str {
        &self.0[..self.0.len().min(8)]
    }
}

impl std::fmt::Display for CommitHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.short())
    }
}

impl From<String> for CommitHash {
    fn from(s: String) -> Self {
        Self(s)
    }
}

#[derive(Debug, Clone)]
pub struct CommitInfo {
    pub hash: CommitHash,
    pub author: String,
    pub timestamp: Option<DateTime<Utc>>,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct Baseline {
    pub name: BaselineName,
    pub timestamp: Option<DateTime<Utc>>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ChangeKind {
    /// The content hash (semantic fingerprint) changed — the requirement's meaning shifted.
    Semantic,
    /// Formatting or metadata only; the semantic fingerprint is unchanged.
    Cosmetic,
}

#[derive(Debug, Clone)]
pub struct ModifiedRequirement {
    pub id: RequirementId,
    pub change_kind: ChangeKind,
}

#[derive(Debug, Clone)]
pub struct RequirementDiff {
    pub from: String,
    pub to: String,
    pub added: Vec<RequirementId>,
    pub removed: Vec<RequirementId>,
    pub modified: Vec<ModifiedRequirement>,
}

pub struct GitContext {
    /// `None` outside a git repository: loading and linting work, history doesn't.
    repository: Option<gix::Repository>,
    pub workdir: PathBuf,
    /// Set for a snapshot: the files under `workdir` are this commit's tree, extracted, and
    /// it stands in for HEAD.
    snapshot: Option<gix::ObjectId>,
}

impl std::fmt::Debug for GitContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitContext")
            .field("workdir", &self.workdir)
            .field("snapshot", &self.snapshot)
            .finish()
    }
}

impl GitContext {
    /// Open the repository containing `path`. Outside a git repository the context is
    /// empty: commands that need history report that, everything else works.
    pub fn open(path: &Path) -> Result<Self, RqtkError> {
        let Ok(repo) = gix::discover(path) else {
            return Ok(Self {
                repository: None,
                workdir: path.to_path_buf(),
                snapshot: None,
            });
        };
        let workdir = repo
            .workdir()
            .ok_or_else(|| RqtkError::Git("bare repositories are not supported".into()))?
            .to_path_buf();
        Ok(Self {
            repository: Some(repo),
            workdir,
            snapshot: None,
        })
    }

    /// A snapshot of `repo` at `commit`, whose tree has been extracted to `workdir`: files
    /// are read from there, history and other revisions from `repo`, and `commit` is HEAD.
    pub fn snapshot(repo: gix::Repository, workdir: PathBuf, commit: gix::ObjectId) -> Self {
        Self {
            repository: Some(repo),
            workdir,
            snapshot: Some(commit),
        }
    }

    /// The commit a snapshot was taken at; `None` for a working tree.
    pub fn snapshot_commit(&self) -> Option<CommitHash> {
        self.snapshot.map(|c| CommitHash::from(c.to_string()))
    }

    /// The commit standing for HEAD: the snapshot's, or the repository's HEAD.
    fn head(&self) -> Option<gix::ObjectId> {
        if let Some(commit) = self.snapshot {
            return Some(commit);
        }
        Some(self.repository.as_ref()?.head_id().ok()?.detach())
    }

    /// The commit `spec` names (a tag, baseline name, branch, SHA or anything `git
    /// rev-parse` understands).
    pub fn resolve_commit(&self, spec: &str) -> Result<gix::ObjectId, RqtkError> {
        self.resolve_ref_commit_id(spec)
    }

    /// Write the tree of `commit` below `dest`, without symbolic links or submodules.
    pub fn extract_commit(&self, commit: gix::ObjectId, dest: &Path) -> Result<(), RqtkError> {
        extract(self.repo()?, commit, dest)
            .map(|_| ())
            .map_err(RqtkError::Git)
    }

    /// The ID git would give `bytes` as a blob in this repository; `None` outside git.
    pub(crate) fn blob_id(&self, bytes: &[u8]) -> Option<gix::ObjectId> {
        let repo = self.repository.as_ref()?;
        gix::objs::compute_hash(repo.object_hash(), gix::objs::Kind::Blob, bytes).ok()
    }

    /// The blob of every file below `dir` at `spec`, by path relative to the git work tree.
    pub(crate) fn blobs_at(
        &self,
        spec: &str,
        dir: &Path,
    ) -> Result<BTreeMap<PathBuf, gix::ObjectId>, RqtkError> {
        self.blobs_in_tree(self.resolve_ref_tree_id(spec)?, dir)
    }

    fn blobs_in_tree(
        &self,
        tree_id: gix::ObjectId,
        dir: &Path,
    ) -> Result<BTreeMap<PathBuf, gix::ObjectId>, RqtkError> {
        let repo = self.repo()?;
        let rel = self.relative(dir);
        let tree = repo
            .find_object(tree_id)
            .map_err(|e| RqtkError::Git(e.to_string()))?
            .into_tree();
        let Some(entry) = tree
            .lookup_entry_by_path(&rel)
            .map_err(|e| RqtkError::Git(e.to_string()))?
        else {
            return Ok(BTreeMap::new());
        };
        let subtree = repo
            .find_object(entry.oid().to_owned())
            .map_err(|e| RqtkError::Git(e.to_string()))?
            .into_tree();
        Ok(subtree
            .traverse()
            .breadthfirst
            .files()
            .map_err(|e| RqtkError::Git(e.to_string()))?
            .into_iter()
            .filter(|e| e.mode.is_blob())
            .map(|e| (rel.join(e.filepath.to_string()), e.oid))
            .collect())
    }

    /// The blob `oid` parsed as TOML; `None` when it is not a `T`.
    pub(crate) fn parse_blob<T: DeserializeOwned>(&self, oid: gix::ObjectId) -> Option<T> {
        let object = self.repo().ok()?.find_object(oid).ok()?;
        toml::from_str(std::str::from_utf8(&object.data).ok()?).ok()
    }

    /// The bytes of `rel` (relative to `root`, the directory holding `.rqtk/`) at `spec`, or
    /// `None` when it is not a file there. Looks up one path; reads no other object.
    pub fn read_at(
        &self,
        spec: &str,
        root: &Path,
        rel: &str,
    ) -> Result<Option<Vec<u8>>, RqtkError> {
        let repo = self.repo()?;
        let tree = repo
            .find_object(self.resolve_ref_tree_id(spec)?)
            .map_err(|e| RqtkError::Git(e.to_string()))?
            .into_tree();
        let path = self.relative(root).join(rel);
        let Some(entry) = tree
            .lookup_entry_by_path(&path)
            .map_err(|e| RqtkError::Git(e.to_string()))?
        else {
            return Ok(None);
        };
        if !entry.mode().is_blob() {
            return Ok(None);
        }
        Ok(self.blob(entry.oid().to_owned()))
    }

    /// The raw bytes of blob `oid`.
    pub(crate) fn blob(&self, oid: gix::ObjectId) -> Option<Vec<u8>> {
        Some(self.repo().ok()?.find_object(oid).ok()?.data.to_vec())
    }

    /// `path` relative to the git work tree, comparing canonical paths.
    pub(crate) fn relative(&self, path: &Path) -> PathBuf {
        let canonical = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
        canonical(path)
            .strip_prefix(canonical(&self.workdir))
            .map(Path::to_path_buf)
            .unwrap_or_else(|_| path.to_path_buf())
    }

    /// The underlying repository, for opening a snapshot of it.
    pub fn repository(&self) -> Option<&gix::Repository> {
        self.repository.as_ref()
    }

    /// Whether the requirements live in a git repository.
    pub fn is_repository(&self) -> bool {
        self.repository.is_some()
    }

    fn repo(&self) -> Result<&gix::Repository, RqtkError> {
        self.repository.as_ref().ok_or_else(|| {
            RqtkError::Git(format!(
                "`{}` is not inside a git repository; this command needs git history",
                self.workdir.display()
            ))
        })
    }

    /// Repository-relative paths that differ between revision `base` and the working tree,
    /// including uncommitted and untracked (non-ignored) files. Uses the `git` executable.
    pub fn changed_files_since(&self, base: &str) -> Result<Vec<PathBuf>, RqtkError> {
        if let Some(commit) = self.snapshot {
            return self.changed_between(
                self.resolve_ref_tree_id(base)?,
                self.commit_oid_to_tree_id(commit)?,
            );
        }
        self.repo()?;
        let git = |args: &[&str]| -> Result<String, RqtkError> {
            let out = std::process::Command::new("git")
                .args(args)
                .current_dir(&self.workdir)
                .output()
                .map_err(|e| RqtkError::Git(format!("cannot run git: {e}")))?;
            if !out.status.success() {
                return Err(RqtkError::Git(
                    String::from_utf8_lossy(&out.stderr).trim().to_owned(),
                ));
            }
            Ok(String::from_utf8_lossy(&out.stdout).into_owned())
        };
        let diffed = git(&["diff", "--name-only", base, "--"])?;
        let untracked = git(&["ls-files", "--others", "--exclude-standard"])?;
        let mut files: Vec<PathBuf> = diffed
            .lines()
            .chain(untracked.lines())
            .filter(|l| !l.is_empty())
            .map(PathBuf::from)
            .collect();
        files.sort();
        files.dedup();
        Ok(files)
    }

    /// Paths whose blob differs between two trees, sorted.
    fn changed_between(
        &self,
        from: gix::ObjectId,
        to: gix::ObjectId,
    ) -> Result<Vec<PathBuf>, RqtkError> {
        let blobs = |tree: gix::ObjectId| -> Result<BTreeMap<String, gix::ObjectId>, RqtkError> {
            let tree = self
                .repo()?
                .find_object(tree)
                .map_err(|e| RqtkError::Git(e.to_string()))?
                .into_tree();
            Ok(tree
                .traverse()
                .breadthfirst
                .files()
                .map_err(|e| RqtkError::Git(e.to_string()))?
                .into_iter()
                .filter(|e| !e.mode.is_tree())
                .map(|e| (e.filepath.to_string(), e.oid))
                .collect())
        };
        let (from, to) = (blobs(from)?, blobs(to)?);
        let mut changed: Vec<PathBuf> = from
            .iter()
            .filter(|(p, oid)| to.get(*p) != Some(*oid))
            .map(|(p, _)| PathBuf::from(p))
            .chain(
                to.keys()
                    .filter(|p| !from.contains_key(*p))
                    .map(PathBuf::from),
            )
            .collect();
        changed.sort();
        Ok(changed)
    }

    /// Full SHA of the commit HEAD points to, or `None` in a repository without commits. For
    /// a snapshot, the snapshot's commit.
    pub fn head_commit(&self) -> Option<CommitHash> {
        Some(CommitHash::from(self.head()?.to_string()))
    }

    pub fn committer_name(&self) -> Result<String, RqtkError> {
        self.repo()?
            .committer()
            .ok_or_else(|| RqtkError::Git("no committer identity configured".into()))
            .and_then(|r| r.map_err(|e| RqtkError::Git(e.to_string())))
            .map(|sig| sig.name.to_string())
    }

    /// Create an annotated tag `rqtk/<name>` at HEAD.
    pub fn create_baseline_tag(&self, name: &BaselineName, message: &str) -> Result<(), RqtkError> {
        if self.snapshot.is_some() {
            return Err(RqtkError::Git(
                "a snapshot is read-only; baselines are created in a working tree".into(),
            ));
        }
        let head_id = self
            .repo()?
            .head_id()
            .map_err(|e| RqtkError::Git(e.to_string()))?;
        let sig = self
            .repo()?
            .committer()
            .ok_or_else(|| RqtkError::Git("no committer identity configured".into()))
            .and_then(|r| r.map_err(|e| RqtkError::Git(e.to_string())))?;
        let tag_name = format!("{BASELINE_TAG_PREFIX}{name}");
        self.repo()?
            .tag(
                &tag_name,
                head_id,
                gix::object::Kind::Commit,
                Some(sig),
                message,
                PreviousValue::MustNotExist,
            )
            .map_err(|e| RqtkError::Git(e.to_string()))?;
        Ok(())
    }

    /// List all `rqtk/*` baseline tags in chronological order.
    pub fn baselines(&self) -> Result<Vec<Baseline>, RqtkError> {
        let prefix = format!("refs/tags/{BASELINE_TAG_PREFIX}");
        let mut baselines = Vec::new();

        let refs = self
            .repo()?
            .references()
            .map_err(|e| RqtkError::Git(e.to_string()))?;

        let tag_refs = refs
            .prefixed(prefix.as_str())
            .map_err(|e| RqtkError::Git(e.to_string()))?;

        for ref_ in tag_refs {
            let ref_ = ref_.map_err(|e| RqtkError::Git(e.to_string()))?;
            let full_name = ref_.name().as_bstr().to_string();
            let Some(version) = full_name.strip_prefix(&prefix) else {
                continue;
            };

            let direct_oid = match ref_.try_id() {
                Some(id) => id.detach(),
                None => continue,
            };

            let obj = match self.repo()?.find_object(direct_oid) {
                Ok(o) => o,
                Err(_) => continue,
            };

            let (ts, msg) = if obj.kind == gix::object::Kind::Tag {
                let tag = match obj.try_into_tag() {
                    Ok(t) => t,
                    Err(_) => continue,
                };
                let secs = tag.tagger().ok().flatten().map(|s| s.seconds());
                let decoded = tag.decode().map_err(|e| RqtkError::Git(e.to_string()))?;
                let m = decoded.message.to_string();
                (secs, m)
            } else if obj.kind == gix::object::Kind::Commit {
                let commit = match obj.try_into_commit() {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                let decoded = commit.decode().map_err(|e| RqtkError::Git(e.to_string()))?;
                let secs = decoded.committer().ok().map(|s| s.seconds());
                let m = decoded.message.to_string();
                (secs, m)
            } else {
                continue;
            };

            let timestamp = ts.and_then(|s| DateTime::from_timestamp(s, 0));
            baselines.push(Baseline {
                name: BaselineName::from_trusted(version),
                timestamp,
                message: msg,
            });
        }

        baselines.sort_by_key(|b| b.timestamp);
        Ok(baselines)
    }

    /// Return the timestamp of the commit that first introduced the requirement file.
    ///
    /// Walks all commits oldest-first; the first commit where the file appears but was
    /// absent from every parent is the creation commit.
    /// Returns `None` if the file has no git history yet (untracked).
    pub fn requirement_creation_date(
        &self,
        req_path: &Path,
    ) -> Result<Option<DateTime<Utc>>, RqtkError> {
        let repo = self.repo()?;
        let req_path_c = req_path
            .canonicalize()
            .unwrap_or_else(|_| req_path.to_path_buf());
        let workdir_c = self
            .workdir
            .canonicalize()
            .unwrap_or_else(|_| self.workdir.clone());
        let rel = req_path_c
            .strip_prefix(&workdir_c)
            .unwrap_or(req_path)
            .to_path_buf();

        let Some(head_id) = self.head() else {
            return Ok(None);
        };

        // Collect all commits and reverse to walk oldest-first.
        let walk = repo
            .rev_walk(std::iter::once(head_id))
            .sorting(gix::revision::walk::Sorting::ByCommitTime(
                Default::default(),
            ))
            .all()
            .map_err(|e| RqtkError::Git(e.to_string()))?;

        let all_infos: Vec<_> = walk
            .map(|r| r.map_err(|e| RqtkError::Git(e.to_string())))
            .collect::<Result<Vec<_>, _>>()?;

        for info in all_infos.into_iter().rev() {
            let commit = info.object().map_err(|e| RqtkError::Git(e.to_string()))?;
            let tree = commit.tree().map_err(|e| RqtkError::Git(e.to_string()))?;

            if tree
                .lookup_entry_by_path(&rel)
                .map_err(|e| RqtkError::Git(e.to_string()))?
                .is_none()
            {
                continue;
            }

            let parent_ids: Vec<gix::ObjectId> = info.parent_ids.iter().copied().collect();
            let introduced = if parent_ids.is_empty() {
                true
            } else {
                parent_ids.iter().all(|parent_id| {
                    repo.find_object(*parent_id)
                        .ok()
                        .and_then(|o| o.try_into_commit().ok())
                        .and_then(|c| c.tree().ok())
                        .map(|pt| pt.lookup_entry_by_path(&rel).ok().flatten().is_none())
                        .unwrap_or(false)
                })
            };

            if introduced {
                let decoded = commit.decode().map_err(|e| RqtkError::Git(e.to_string()))?;
                return Ok(decoded
                    .committer()
                    .ok()
                    .map(|s| s.seconds())
                    .and_then(|s| DateTime::from_timestamp(s, 0)));
            }
        }

        Ok(None)
    }

    /// Return the git log for a single requirement file.
    ///
    /// Note: rename tracking (`git log --follow`) is not supported; only the path
    /// as given is considered.
    pub fn requirement_history(&self, req_path: &Path) -> Result<Vec<CommitInfo>, RqtkError> {
        let repo = self.repo()?;
        let req_path_c = req_path
            .canonicalize()
            .unwrap_or_else(|_| req_path.to_path_buf());
        let workdir_c = self
            .workdir
            .canonicalize()
            .unwrap_or_else(|_| self.workdir.clone());
        let rel = req_path_c
            .strip_prefix(&workdir_c)
            .unwrap_or(req_path)
            .to_path_buf();

        let Some(head_id) = self.head() else {
            return Ok(Vec::new());
        };

        let walk = repo
            .rev_walk(std::iter::once(head_id))
            .sorting(gix::revision::walk::Sorting::ByCommitTime(
                Default::default(),
            ))
            .all()
            .map_err(|e| RqtkError::Git(e.to_string()))?;

        let mut commits = Vec::new();
        for info in walk {
            let info = info.map_err(|e| RqtkError::Git(e.to_string()))?;
            let commit = info.object().map_err(|e| RqtkError::Git(e.to_string()))?;
            let tree = commit.tree().map_err(|e| RqtkError::Git(e.to_string()))?;

            let current_entry = tree
                .lookup_entry_by_path(&rel)
                .map_err(|e| RqtkError::Git(e.to_string()))?;
            let Some(current_entry) = current_entry else {
                continue;
            };
            let current_blob_oid = current_entry.oid().to_owned();

            let parent_ids: Vec<gix::ObjectId> = info.parent_ids.iter().copied().collect();
            let changed = if parent_ids.is_empty() {
                true
            } else {
                parent_ids.iter().any(|parent_id| {
                    let parent_blob_oid: Option<gix::ObjectId> = repo
                        .find_object(*parent_id)
                        .ok()
                        .and_then(|o| o.try_into_commit().ok())
                        .and_then(|c| c.tree().ok())
                        .and_then(|pt| pt.lookup_entry_by_path(&rel).ok().flatten())
                        .map(|e| e.oid().to_owned());
                    parent_blob_oid != Some(current_blob_oid)
                })
            };

            if !changed {
                continue;
            }

            let decoded = commit.decode().map_err(|e| RqtkError::Git(e.to_string()))?;
            let (author_name, secs) = decoded
                .author()
                .map(|a| (a.name.to_string(), Some(a.seconds())))
                .unwrap_or_else(|_| (String::new(), None));
            let timestamp = secs.and_then(|s| DateTime::from_timestamp(s, 0));

            commits.push(CommitInfo {
                hash: CommitHash::from(info.id.to_string()),
                author: author_name,
                timestamp,
                message: decoded.message.to_string(),
            });
        }

        Ok(commits)
    }

    /// Compute the semantic diff between two baseline tags.
    pub fn diff_baselines(
        &self,
        from: &BaselineName,
        to: &BaselineName,
        req_dir: &Path,
        spec_root: &Path,
    ) -> Result<RequirementDiff, RqtkError> {
        let from_tree_id = self.resolve_baseline_tree_id(from)?;
        let to_tree_id = self.resolve_baseline_tree_id(to)?;
        self.diff_tree_ids(
            (from_tree_id, from.to_string()),
            (to_tree_id, to.to_string()),
            &DiffScope {
                req_dir,
                spec_root,
                known: &BTreeMap::new(),
            },
        )
    }

    /// Compute the semantic diff between two arbitrary git refs (branch, SHA, HEAD, or baseline name).
    ///
    /// Accepts the same forms as `git rev-parse`: `HEAD`, `origin/main`, a full SHA, a branch
    /// name, or a bare baseline version like `1.0.0` (resolved as `refs/tags/rqtk/1.0.0`).
    pub fn diff_refs(
        &self,
        from: &str,
        to: &str,
        req_dir: &Path,
        spec_root: &Path,
        known: &BTreeMap<gix::ObjectId, bool>,
    ) -> Result<RequirementDiff, RqtkError> {
        let from_tree_id = self.resolve_ref_tree_id(from)?;
        let to_tree_id = self.resolve_ref_tree_id(to)?;
        self.diff_tree_ids(
            (from_tree_id, from.to_string()),
            (to_tree_id, to.to_string()),
            &DiffScope {
                req_dir,
                spec_root,
                known,
            },
        )
    }

    fn diff_tree_ids(
        &self,
        (from_tree_id, from_label): (gix::ObjectId, String),
        (to_tree_id, to_label): (gix::ObjectId, String),
        scope: &DiffScope<'_>,
    ) -> Result<RequirementDiff, RqtkError> {
        let DiffScope {
            req_dir,
            spec_root,
            known,
        } = *scope;
        // A file whose blob is the same on both sides is unchanged, unless it is an interface
        // requirement (its spec files may differ): only the rest is parsed.
        let from_blobs = self.blobs_in_tree(from_tree_id, req_dir)?;
        let to_blobs = self.blobs_in_tree(to_tree_id, req_dir)?;
        let unchanged: BTreeSet<&PathBuf> = to_blobs
            .iter()
            .filter(|(path, oid)| {
                from_blobs.get(*path) == Some(*oid)
                    && match known.get(*oid) {
                        // A blob the caller has loaded: it knows whether it is an interface.
                        Some(interface) => !interface,
                        None => self
                            .blob(**oid)
                            .is_some_and(|bytes| !contains(&bytes, b"interface")),
                    }
            })
            .map(|(path, _)| path)
            .collect();
        let parse =
            |blobs: &BTreeMap<PathBuf, gix::ObjectId>| -> BTreeMap<RequirementId, Requirement> {
                blobs
                    .iter()
                    .filter(|(path, _)| {
                        path.extension().is_some_and(|e| e == "toml") && !unchanged.contains(path)
                    })
                    .filter_map(|(_, oid)| self.parse_blob::<Requirement>(*oid))
                    .map(|r| (r.id.clone(), r))
                    .collect()
            };
        let from_reqs = parse(&from_blobs);
        let to_reqs = parse(&to_blobs);
        // Spec files are only read when an interface requirement is compared.
        let needs_specs = from_reqs
            .values()
            .chain(to_reqs.values())
            .any(|r| r.interface.is_some());
        let (from_files, to_files) = if needs_specs {
            (
                Some(self.tree_files(from_tree_id, spec_root)?),
                Some(self.tree_files(to_tree_id, spec_root)?),
            )
        } else {
            (None, None)
        };
        let hash = |r: &Requirement, files: &Option<TreeFiles<'_>>| match files {
            Some(files) => hash_in(r, files),
            None => r.compute_content_hash(None),
        };

        let from_ids: std::collections::HashSet<_> = from_reqs.keys().cloned().collect();
        let to_ids: std::collections::HashSet<_> = to_reqs.keys().cloned().collect();

        let mut added: Vec<_> = to_ids.difference(&from_ids).cloned().collect();
        added.sort();
        let mut removed: Vec<_> = from_ids.difference(&to_ids).cloned().collect();
        removed.sort();

        let mut modified = Vec::new();
        let mut common: Vec<_> = from_ids.intersection(&to_ids).cloned().collect();
        common.sort();

        for id in common {
            let from_r = &from_reqs[&id];
            let to_r = &to_reqs[&id];

            // Always recompute: a stored hash may be stale. An interface requirement can
            // change through its spec files alone.
            let change_kind = if hash(from_r, &from_files) != hash(to_r, &to_files) {
                ChangeKind::Semantic
            } else if toml::to_string(from_r).ok() != toml::to_string(to_r).ok() {
                ChangeKind::Cosmetic
            } else {
                continue;
            };

            modified.push(ModifiedRequirement { id, change_kind });
        }

        Ok(RequirementDiff {
            from: from_label,
            to: to_label,
            added,
            removed,
            modified,
        })
    }

    /// Resolve an arbitrary ref spec to a git tree object ID.
    ///
    /// Accepts (in order of precedence):
    /// - `HEAD`
    /// - full `refs/...` paths
    /// - `origin/branch` style → `refs/remotes/origin/branch`
    /// - bare names → tried as rqtk baseline tag, local branch, then tag
    fn resolve_ref_tree_id(&self, spec: &str) -> Result<gix::ObjectId, RqtkError> {
        let commit = self.resolve_ref_commit_id(spec)?;
        self.commit_oid_to_tree_id(commit)
    }

    /// [`Self::resolve_ref_tree_id`], stopping at the commit.
    fn resolve_ref_commit_id(&self, spec: &str) -> Result<gix::ObjectId, RqtkError> {
        self.repo()?;
        if spec == "HEAD" {
            return self
                .head()
                .ok_or_else(|| RqtkError::Git("HEAD does not point to a commit".into()));
        }

        let candidates: Vec<String> = if spec.starts_with("refs/") {
            vec![spec.to_owned()]
        } else if spec.contains('/') {
            vec![format!("refs/remotes/{spec}")]
        } else {
            vec![
                format!("refs/tags/{BASELINE_TAG_PREFIX}{spec}"),
                format!("refs/heads/{spec}"),
                format!("refs/tags/{spec}"),
            ]
        };

        for candidate in &candidates {
            if let Ok(mut r) = self.repo()?.find_reference(candidate.as_str()) {
                let commit_id = r
                    .peel_to_id()
                    .map_err(|e| RqtkError::Git(e.to_string()))?
                    .detach();
                return Ok(commit_id);
            }
        }

        // Anything else git understands: SHAs, `HEAD~2`, `main^`, …
        if let Ok(id) = self.repo()?.rev_parse_single(spec) {
            let git_err = |e: &dyn std::fmt::Display| RqtkError::Git(e.to_string());
            let commit_id = id
                .object()
                .map_err(|e| git_err(&e))?
                .peel_to_commit()
                .map_err(|e| git_err(&e))?
                .id;
            return Ok(commit_id);
        }

        Err(RqtkError::Git(format!(
            "could not resolve '{spec}' as a git ref or rqtk baseline"
        )))
    }

    fn commit_oid_to_tree_id(&self, oid: gix::ObjectId) -> Result<gix::ObjectId, RqtkError> {
        let commit = self
            .repo()?
            .find_object(oid)
            .map_err(|e| RqtkError::Git(e.to_string()))?
            .try_into_commit()
            .map_err(|e| RqtkError::Git(e.to_string()))?;
        commit
            .tree_id()
            .map(|id| id.detach())
            .map_err(|e| RqtkError::Git(e.to_string()))
    }

    /// The text of repository-relative file `rel` at `spec`, or `None` if it does not exist
    /// there.
    pub fn file_at_ref(&self, spec: &str, rel: &Path) -> Result<Option<String>, RqtkError> {
        let tree = self
            .repo()?
            .find_object(self.resolve_ref_tree_id(spec)?)
            .map_err(|e| RqtkError::Git(e.to_string()))?
            .into_tree();
        let Some(entry) = tree
            .lookup_entry_by_path(rel)
            .map_err(|e| RqtkError::Git(e.to_string()))?
        else {
            return Ok(None);
        };
        let obj = self
            .repo()?
            .find_object(entry.oid().to_owned())
            .map_err(|e| RqtkError::Git(e.to_string()))?;
        Ok(Some(String::from_utf8_lossy(&obj.data).into_owned()))
    }

    /// The files below `spec_root` (the directory holding `.rqtk/`) as they are at `spec`,
    /// for resolving interface specs at that ref.
    pub fn spec_files_at(&self, spec: &str, spec_root: &Path) -> Result<TreeFiles<'_>, RqtkError> {
        self.tree_files(self.resolve_ref_tree_id(spec)?, spec_root)
    }

    fn tree_files(
        &self,
        tree_id: gix::ObjectId,
        spec_root: &Path,
    ) -> Result<TreeFiles<'_>, RqtkError> {
        let repo = self.repo()?;
        let canonical = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
        let prefix = canonical(spec_root)
            .strip_prefix(canonical(&self.workdir))
            .map(|p| {
                p.components()
                    .map(|c| c.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .unwrap_or_default();
        let tree = repo
            .find_object(tree_id)
            .map_err(|e| RqtkError::Git(e.to_string()))?
            .into_tree();
        let mut blobs = BTreeMap::new();
        for entry in tree
            .traverse()
            .breadthfirst
            .files()
            .map_err(|e| RqtkError::Git(e.to_string()))?
        {
            if !entry.mode.is_blob() {
                continue;
            }
            let path = entry.filepath.to_string();
            let rel = if prefix.is_empty() {
                Some(path.as_str())
            } else {
                path.strip_prefix(&format!("{prefix}/"))
            };
            if let Some(rel) = rel {
                blobs.insert(rel.to_owned(), entry.oid);
            }
        }
        Ok(TreeFiles { repo, blobs })
    }

    /// Parse every `.toml` file below `dir` as a `T`, as it exists at `spec` (a branch, tag,
    /// `HEAD`, or bare baseline name). A directory absent at that ref yields nothing.
    pub fn items_at_ref<T: DeserializeOwned>(
        &self,
        spec: &str,
        dir: &Path,
    ) -> Result<Vec<T>, RqtkError> {
        let tree = self
            .repo()?
            .find_object(self.resolve_ref_tree_id(spec)?)
            .map_err(|e| RqtkError::Git(e.to_string()))?
            .into_tree();
        read_items_from_tree(self.repo()?, &tree, dir, &self.workdir)
    }

    /// Return the parsed requirement file as it existed at a given baseline.
    pub fn requirement_at_baseline(
        &self,
        baseline: &BaselineName,
        req_path: &Path,
    ) -> Result<Option<Requirement>, RqtkError> {
        let tree_id = self.resolve_baseline_tree_id(baseline)?;
        let tree = self
            .repo()?
            .find_object(tree_id)
            .map_err(|e| RqtkError::Git(e.to_string()))?
            .into_tree();

        let req_path_c = req_path
            .canonicalize()
            .unwrap_or_else(|_| req_path.to_path_buf());
        let workdir_c = self
            .workdir
            .canonicalize()
            .unwrap_or_else(|_| self.workdir.clone());
        let rel = req_path_c
            .strip_prefix(&workdir_c)
            .unwrap_or(req_path)
            .to_path_buf();

        match tree
            .lookup_entry_by_path(&rel)
            .map_err(|e| RqtkError::Git(e.to_string()))?
        {
            Some(entry) => {
                let blob_oid = entry.oid().to_owned();
                let obj = self
                    .repo()?
                    .find_object(blob_oid)
                    .map_err(|e| RqtkError::Git(e.to_string()))?;
                let content =
                    std::str::from_utf8(&obj.data).map_err(|e| RqtkError::Git(e.to_string()))?;
                let req: Requirement =
                    toml::from_str(content).map_err(|source| RqtkError::TomlParse {
                        path: req_path.to_path_buf(),
                        source,
                    })?;
                Ok(Some(req))
            }
            None => Ok(None),
        }
    }

    fn resolve_baseline_tree_id(&self, name: &BaselineName) -> Result<gix::ObjectId, RqtkError> {
        let ref_name = format!("refs/tags/{BASELINE_TAG_PREFIX}{name}");
        let mut ref_ = self
            .repo()?
            .find_reference(ref_name.as_str())
            .map_err(|_| RqtkError::BaselineNotFound(name.to_string()))?;
        // peel_to_id follows symrefs and tag objects to reach the commit
        let commit_id = ref_
            .peel_to_id()
            .map_err(|e| RqtkError::Git(e.to_string()))?
            .detach();
        let commit = self
            .repo()?
            .find_object(commit_id)
            .map_err(|e| RqtkError::Git(e.to_string()))?
            .try_into_commit()
            .map_err(|e| RqtkError::Git(e.to_string()))?;
        commit
            .tree_id()
            .map(|id| id.detach())
            .map_err(|e| RqtkError::Git(e.to_string()))
    }
}

/// What a requirement diff compares within the two trees.
#[derive(Clone, Copy)]
struct DiffScope<'a> {
    /// The requirements directory.
    req_dir: &'a Path,
    /// The directory holding `.rqtk/`, which interface spec paths are relative to.
    spec_root: &'a Path,
    /// Whether each blob the caller has loaded is an interface requirement.
    known: &'a BTreeMap<gix::ObjectId, bool>,
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// Files as they are in a git tree, read for resolving interface specs at a ref.
pub struct TreeFiles<'r> {
    repo: &'r gix::Repository,
    /// Blob of each file, by path relative to the spec root.
    blobs: BTreeMap<String, gix::ObjectId>,
}

impl crate::spec::SpecFiles for TreeFiles<'_> {
    fn read(&self, path: &str) -> std::io::Result<Option<Vec<u8>>> {
        let Some(oid) = self.blobs.get(path) else {
            return Ok(None);
        };
        let object = self.repo.find_object(*oid).map_err(std::io::Error::other)?;
        Ok(Some(object.data.to_vec()))
    }

    fn list(&self, dir: &str) -> Vec<String> {
        let prefix = format!("{dir}/");
        self.blobs
            .keys()
            .filter(|p| dir.is_empty() || p.starts_with(&prefix))
            .cloned()
            .collect()
    }
}

/// The content hash of `req` with its spec files read from `files`.
pub(crate) fn hash_in(req: &Requirement, files: &dyn crate::spec::SpecFiles) -> String {
    let digest = req
        .interface
        .as_ref()
        .map(|i| crate::spec::resolve(files, i).digest);
    req.compute_content_hash(digest.as_deref())
}

/// Parse every `.toml` file below `dir` in `tree` as a `T`. A directory missing from the
/// tree yields nothing; files that do not parse are skipped.
fn read_items_from_tree<T: DeserializeOwned>(
    repo: &gix::Repository,
    tree: &gix::Tree<'_>,
    dir: &Path,
    repo_root: &Path,
) -> Result<Vec<T>, RqtkError> {
    let dir_c = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    let repo_root_c = repo_root
        .canonicalize()
        .unwrap_or_else(|_| repo_root.to_path_buf());
    let rel = dir_c
        .strip_prefix(&repo_root_c)
        .unwrap_or(dir)
        .to_path_buf();

    let Some(subtree_entry) = tree
        .lookup_entry_by_path(&rel)
        .map_err(|e| RqtkError::Git(e.to_string()))?
    else {
        return Ok(Vec::new());
    };
    let subtree = repo
        .find_object(subtree_entry.oid().to_owned())
        .map_err(|e| RqtkError::Git(e.to_string()))?
        .into_tree();
    let files = subtree
        .traverse()
        .breadthfirst
        .files()
        .map_err(|e| RqtkError::Git(e.to_string()))?;

    let mut items = Vec::new();
    for file in files {
        if file.mode.is_tree() || !file.filepath.to_string().ends_with(".toml") {
            continue;
        }
        let obj = repo
            .find_object(file.oid)
            .map_err(|e| RqtkError::Git(e.to_string()))?;
        let Ok(content) = std::str::from_utf8(&obj.data) else {
            continue;
        };
        if let Ok(item) = toml::from_str::<T>(content) {
            items.push(item);
        }
    }
    Ok(items)
}

/// Write the tree of `commit` below `dest`. Symbolic links and submodules are not written;
/// the paths of symbolic links are returned.
pub(crate) fn extract(
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
