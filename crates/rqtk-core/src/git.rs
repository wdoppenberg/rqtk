use crate::error::RqtkError;
use crate::model::{RequirementFile, RequirementId};
use chrono::{DateTime, Utc};
use gix::refs::transaction::PreviousValue;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;

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
    pub(crate) repo: gix::Repository,
    pub workdir: PathBuf,
}

impl std::fmt::Debug for GitContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitContext")
            .field("workdir", &self.workdir)
            .finish()
    }
}

impl GitContext {
    pub fn open(path: &Path) -> Result<Self, RqtkError> {
        let repo = gix::discover(path).map_err(|e| RqtkError::Git(e.to_string()))?;
        let workdir = repo
            .workdir()
            .ok_or_else(|| RqtkError::Git("bare repositories are not supported".into()))?
            .to_path_buf();
        Ok(Self { repo, workdir })
    }

    pub fn committer_name(&self) -> Result<String, RqtkError> {
        self.repo
            .committer()
            .ok_or_else(|| RqtkError::Git("no committer identity configured".into()))
            .and_then(|r| r.map_err(|e| RqtkError::Git(e.to_string())))
            .map(|sig| sig.name.to_string())
    }

    /// Create an annotated tag `rqtk/<name>` at HEAD.
    pub fn create_baseline_tag(&self, name: &BaselineName, message: &str) -> Result<(), RqtkError> {
        let head_id = self
            .repo
            .head_id()
            .map_err(|e| RqtkError::Git(e.to_string()))?;
        let sig = self
            .repo
            .committer()
            .ok_or_else(|| RqtkError::Git("no committer identity configured".into()))
            .and_then(|r| r.map_err(|e| RqtkError::Git(e.to_string())))?;
        let tag_name = format!("{BASELINE_TAG_PREFIX}{name}");
        self.repo
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
            .repo
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

            let obj = match self.repo.find_object(direct_oid) {
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

        let head_id = match self.repo.head_id() {
            Ok(id) => id.detach(),
            Err(_) => return Ok(None),
        };

        // Collect all commits and reverse to walk oldest-first.
        let walk = self
            .repo
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
                    self.repo
                        .find_object(*parent_id)
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

        let head_id = match self.repo.head_id() {
            Ok(id) => id.detach(),
            Err(_) => return Ok(Vec::new()),
        };

        let walk = self
            .repo
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
                    let parent_blob_oid: Option<gix::ObjectId> = self
                        .repo
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
    ) -> Result<RequirementDiff, RqtkError> {
        let from_tree_id = self.resolve_baseline_tree_id(from)?;
        let to_tree_id = self.resolve_baseline_tree_id(to)?;
        self.diff_tree_ids(
            from_tree_id,
            to_tree_id,
            from.to_string(),
            to.to_string(),
            req_dir,
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
    ) -> Result<RequirementDiff, RqtkError> {
        let from_tree_id = self.resolve_ref_tree_id(from)?;
        let to_tree_id = self.resolve_ref_tree_id(to)?;
        self.diff_tree_ids(
            from_tree_id,
            to_tree_id,
            from.to_string(),
            to.to_string(),
            req_dir,
        )
    }

    fn diff_tree_ids(
        &self,
        from_tree_id: gix::ObjectId,
        to_tree_id: gix::ObjectId,
        from_label: String,
        to_label: String,
        req_dir: &Path,
    ) -> Result<RequirementDiff, RqtkError> {
        let from_tree = self
            .repo
            .find_object(from_tree_id)
            .map_err(|e| RqtkError::Git(e.to_string()))?
            .into_tree();
        let to_tree = self
            .repo
            .find_object(to_tree_id)
            .map_err(|e| RqtkError::Git(e.to_string()))?
            .into_tree();

        let repo_root = self.workdir.as_path();
        let from_reqs = read_requirements_from_tree(&self.repo, &from_tree, req_dir, repo_root)?;
        let to_reqs = read_requirements_from_tree(&self.repo, &to_tree, req_dir, repo_root)?;

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
            let from_r = &from_reqs[&id].requirement;
            let to_r = &to_reqs[&id].requirement;

            let from_ser = toml::to_string_pretty(&RequirementFile {
                requirement: from_r.clone(),
            })
            .unwrap_or_default();
            let to_ser = toml::to_string_pretty(&RequirementFile {
                requirement: to_r.clone(),
            })
            .unwrap_or_default();

            if from_ser == to_ser {
                continue;
            }

            let from_hash = from_r
                .content_hash
                .clone()
                .unwrap_or_else(|| from_r.compute_content_hash());
            let to_hash = to_r
                .content_hash
                .clone()
                .unwrap_or_else(|| to_r.compute_content_hash());

            let change_kind = if from_hash != to_hash {
                ChangeKind::Semantic
            } else {
                ChangeKind::Cosmetic
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
        if spec == "HEAD" {
            let head_id = self
                .repo
                .head_id()
                .map_err(|e| RqtkError::Git(e.to_string()))?
                .detach();
            return self.commit_oid_to_tree_id(head_id);
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
            if let Ok(mut r) = self.repo.find_reference(candidate.as_str()) {
                let commit_id = r
                    .peel_to_id()
                    .map_err(|e| RqtkError::Git(e.to_string()))?
                    .detach();
                return self.commit_oid_to_tree_id(commit_id);
            }
        }

        Err(RqtkError::Git(format!(
            "could not resolve '{spec}' as a git ref or rqtk baseline"
        )))
    }

    fn commit_oid_to_tree_id(&self, oid: gix::ObjectId) -> Result<gix::ObjectId, RqtkError> {
        let commit = self
            .repo
            .find_object(oid)
            .map_err(|e| RqtkError::Git(e.to_string()))?
            .try_into_commit()
            .map_err(|e| RqtkError::Git(e.to_string()))?;
        commit
            .tree_id()
            .map(|id| id.detach())
            .map_err(|e| RqtkError::Git(e.to_string()))
    }

    /// Return the parsed requirement file as it existed at a given baseline.
    pub fn requirement_at_baseline(
        &self,
        baseline: &BaselineName,
        req_path: &Path,
    ) -> Result<Option<RequirementFile>, RqtkError> {
        let tree_id = self.resolve_baseline_tree_id(baseline)?;
        let tree = self
            .repo
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
                    .repo
                    .find_object(blob_oid)
                    .map_err(|e| RqtkError::Git(e.to_string()))?;
                let content =
                    std::str::from_utf8(&obj.data).map_err(|e| RqtkError::Git(e.to_string()))?;
                let req: RequirementFile =
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
            .repo
            .find_reference(ref_name.as_str())
            .map_err(|_| RqtkError::BaselineNotFound(name.to_string()))?;
        // peel_to_id follows symrefs and tag objects to reach the commit
        let commit_id = ref_
            .peel_to_id()
            .map_err(|e| RqtkError::Git(e.to_string()))?
            .detach();
        let commit = self
            .repo
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

fn read_requirements_from_tree(
    repo: &gix::Repository,
    tree: &gix::Tree<'_>,
    req_dir: &Path,
    repo_root: &Path,
) -> Result<BTreeMap<RequirementId, RequirementFile>, RqtkError> {
    let req_dir_c = req_dir
        .canonicalize()
        .unwrap_or_else(|_| req_dir.to_path_buf());
    let repo_root_c = repo_root
        .canonicalize()
        .unwrap_or_else(|_| repo_root.to_path_buf());
    let rel = req_dir_c
        .strip_prefix(&repo_root_c)
        .unwrap_or(req_dir)
        .to_path_buf();

    let subtree_entry = tree
        .lookup_entry_by_path(&rel)
        .map_err(|e| RqtkError::Git(format!("requirements dir not found in tree: {e}")))?
        .ok_or_else(|| RqtkError::Git("requirements dir not found in tree".into()))?;

    let subtree_oid = subtree_entry.oid().to_owned();
    let subtree = repo
        .find_object(subtree_oid)
        .map_err(|e| RqtkError::Git(e.to_string()))?
        .into_tree();

    let files = subtree
        .traverse()
        .breadthfirst
        .files()
        .map_err(|e| RqtkError::Git(e.to_string()))?;

    let mut requirements = BTreeMap::new();
    for file in files {
        if file.mode.is_tree() {
            continue;
        }
        let name = file.filepath.to_string();
        if !name.ends_with(".toml") {
            continue;
        }
        let obj = repo
            .find_object(file.oid)
            .map_err(|e| RqtkError::Git(e.to_string()))?;
        let Ok(content) = std::str::from_utf8(&obj.data) else {
            continue;
        };
        if let Ok(req_file) = toml::from_str::<RequirementFile>(content) {
            requirements.insert(req_file.requirement.id.clone(), req_file);
        }
    }

    Ok(requirements)
}
