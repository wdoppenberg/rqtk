//! Interface definition files: which files an `[interface]` table names, and the digest of
//! their content that goes into the requirement's content hash.
//!
//! OpenAPI entries are parsed (see [`crate::openapi`]): their `$ref`s are followed, their
//! element pointer is checked, and only the element and what it references are hashed, in a
//! canonical form. Every other format is opaque: each file a path or glob matches is hashed
//! whole, with line endings normalised to LF.
//!
//! Files are read through [`SpecFiles`], so the same resolution runs on the working tree, on
//! a vendored snapshot, and on a git tree at another ref.

use crate::model::{ContentHasher, Interface};
use crate::vendor::VENDOR_DIR;
use std::collections::BTreeSet;
use std::path::Path;

/// The spec formats rqtk knows. Only OpenAPI is parsed; the others are hashed as files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecFormat {
    OpenApi,
    Protobuf,
    AsyncApi,
    JsonSchema,
    Other,
}

impl SpecFormat {
    pub const ALL: [SpecFormat; 5] = [
        Self::OpenApi,
        Self::Protobuf,
        Self::AsyncApi,
        Self::JsonSchema,
        Self::Other,
    ];

    /// The name used for the format in `[[interface.specs]]`.
    pub fn name(self) -> &'static str {
        match self {
            Self::OpenApi => "openapi",
            Self::Protobuf => "protobuf",
            Self::AsyncApi => "asyncapi",
            Self::JsonSchema => "jsonschema",
            Self::Other => "other",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.name() == name)
    }
}

/// Lint codes for spec problems.
pub mod codes {
    pub const NO_MATCH: &str = "RQ032";
    pub const UNKNOWN_FORMAT: &str = "RQ033";
    pub const NO_SPECS: &str = "RQ034";
    pub const UNPARSEABLE: &str = "RQ035";
    pub const ESCAPES: &str = "RQ036";
    pub const UNRESOLVED: &str = "RQ037";
}

/// Where an interface's definition files are read from: paths are root-relative and
/// `/`-separated.
pub trait SpecFiles {
    /// The bytes of `path`, or `None` when it is not a file.
    fn read(&self, path: &str) -> std::io::Result<Option<Vec<u8>>>;
    /// Every file below directory `dir` (`""` for the root), sorted.
    fn list(&self, dir: &str) -> Vec<String>;
}

/// Spec files in a directory on disk.
#[derive(Debug, Clone, Copy)]
pub struct DirFiles<'a> {
    root: &'a Path,
    ignore_files: bool,
}

impl<'a> DirFiles<'a> {
    /// A repository's working tree: globs skip what the repository's own `.gitignore` files
    /// exclude (never those of parent directories or the user's global excludes), so they
    /// match what a commit of the repository holds.
    pub fn working_tree(root: &'a Path) -> Self {
        Self {
            root,
            ignore_files: true,
        }
    }

    /// A vendored snapshot: every file counts, whatever any ignore file says.
    pub fn snapshot(root: &'a Path) -> Self {
        Self {
            root,
            ignore_files: false,
        }
    }
}

impl SpecFiles for DirFiles<'_> {
    fn read(&self, path: &str) -> std::io::Result<Option<Vec<u8>>> {
        let full = self.root.join(path);
        if !full.is_file() {
            return Ok(None);
        }
        std::fs::read(full).map(Some)
    }

    fn list(&self, dir: &str) -> Vec<String> {
        let root = self.root.to_path_buf();
        let wanted = dir.to_owned();
        // Walk from the root, so the root's ignore files apply, pruning everything that
        // is neither on the way to `dir` nor inside it.
        let mut files: Vec<String> = ignore::WalkBuilder::new(&root)
            .standard_filters(false)
            .git_ignore(self.ignore_files)
            .ignore(self.ignore_files)
            .require_git(false)
            .parents(false)
            .filter_entry({
                let root = root.clone();
                move |e| {
                    if e.file_name() == ".git" {
                        return false;
                    }
                    let rel = rel_path(&root, e.path());
                    rel.is_empty()
                        || wanted.is_empty()
                        || rel == wanted
                        || rel.starts_with(&format!("{wanted}/"))
                        || wanted.starts_with(&format!("{rel}/"))
                }
            })
            .build()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_some_and(|t| t.is_file()))
            .map(|e| rel_path(&root, e.path()))
            .filter(|rel| dir.is_empty() || rel.starts_with(&format!("{dir}/")))
            .collect();
        files.sort();
        files
    }
}

fn rel_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|rel| {
            rel.components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/")
        })
        .unwrap_or_default()
}

/// The files an interface's spec entries name, their digest, and what is wrong with them.
#[derive(Debug, Clone, Default)]
pub struct SpecResolution {
    /// Root-relative, `/`-separated paths of every file the entries match or reference.
    pub files: BTreeSet<String>,
    /// Digest of the spec content (hex SHA-256), fed into the requirement's content hash.
    pub digest: String,
    pub problems: Vec<SpecProblem>,
}

/// A finding about one interface's spec entries, reported by lint under `code`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecProblem {
    pub code: &'static str,
    /// Index of the `[[interface.specs]]` entry, when the problem belongs to one.
    pub entry: Option<usize>,
    pub message: String,
    /// The root-relative file the problem is about, when it is about one.
    pub path: Option<String>,
}

impl SpecProblem {
    pub(crate) fn new(code: &'static str, entry: usize, message: String) -> Self {
        Self {
            code,
            entry: Some(entry),
            message,
            path: None,
        }
    }

    pub(crate) fn at(mut self, path: &str) -> Self {
        self.path = Some(path.to_owned());
        self
    }

    pub(crate) fn no_match(entry: usize, path: &str) -> Self {
        Self::new(
            codes::NO_MATCH,
            entry,
            format!("spec path `{path}` matches no file"),
        )
        .at(path)
    }
}

/// Resolve the spec entries of `interface` against `files`: the repository's working tree
/// for a local requirement, a vendored snapshot's `files/` directory, or a git tree.
pub fn resolve(files: &dyn SpecFiles, interface: &Interface) -> SpecResolution {
    let mut hasher = ContentHasher::default();
    let mut out = SpecResolution::default();
    if interface.specs.is_empty() {
        out.problems.push(SpecProblem {
            code: codes::NO_SPECS,
            entry: None,
            message: "`[interface]` lists no spec files; add an `[[interface.specs]]` entry"
                .to_owned(),
            path: None,
        });
    }
    for (i, spec) in interface.specs.iter().enumerate() {
        hasher.field("spec", &i.to_string());
        let Some(format) = SpecFormat::parse(&spec.format) else {
            let names: Vec<&str> = SpecFormat::ALL.iter().map(|f| f.name()).collect();
            out.problems.push(SpecProblem::new(
                codes::UNKNOWN_FORMAT,
                i,
                format!(
                    "unknown spec format `{}`; use one of {}",
                    spec.format,
                    names.join(", ")
                ),
            ));
            continue;
        };
        let Some(path) = normalize("", &spec.path) else {
            out.problems.push(SpecProblem::new(
                codes::ESCAPES,
                i,
                format!(
                    "spec path `{}` is absolute or points outside the repository",
                    spec.path
                ),
            ));
            continue;
        };
        if format == SpecFormat::OpenApi {
            let closure = crate::openapi::closure(
                files,
                &path,
                spec.element.as_deref(),
                i,
                &mut out.problems,
            );
            hasher.field("canonical", crate::openapi::CANONICAL_FORM);
            for (key, canonical) in &closure.nodes {
                hasher.field(key, canonical);
            }
            out.files.extend(closure.files);
            continue;
        }
        let matched = if is_glob(&path) {
            glob_files(files, &path)
        } else {
            vec![path.clone()]
        };
        let mut found = false;
        for file in matched {
            match files.read(&file) {
                Ok(Some(bytes)) => {
                    found = true;
                    hasher.field("file", &file);
                    hasher.bytes("content", &normalize_line_endings(&bytes));
                    out.files.insert(file);
                }
                Ok(None) => {}
                Err(e) => out.problems.push(SpecProblem::new(
                    codes::NO_MATCH,
                    i,
                    format!("cannot read spec file `{file}`: {e}"),
                )),
            }
        }
        if !found {
            out.problems.push(SpecProblem::no_match(i, &path));
        }
    }
    out.digest = hasher.finish_hex();
    out
}

/// `bytes` with every CRLF replaced by LF, so a checkout with `core.autocrlf` hashes the
/// same as one without.
pub fn normalize_line_endings(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut iter = bytes.iter().peekable();
    while let Some(&b) = iter.next() {
        if b == b'\r' && iter.peek() == Some(&&b'\n') {
            continue;
        }
        out.push(b);
    }
    out
}

/// Join `rel` onto the root-relative directory `base` and normalise `.` and `..`
/// lexically. `None` when `rel` is absolute, names a drive or URL scheme, or climbs above
/// the root.
pub(crate) fn normalize(base: &str, rel: &str) -> Option<String> {
    if rel.starts_with('/') || rel.starts_with('\\') || rel.contains(':') {
        return None;
    }
    let mut parts: Vec<&str> = base.split('/').filter(|p| !p.is_empty()).collect();
    for part in rel.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            p => parts.push(p),
        }
    }
    if parts.is_empty() {
        return None;
    }
    Some(parts.join("/"))
}

pub(crate) fn is_glob(path: &str) -> bool {
    path.contains(['*', '?', '[', '{'])
}

/// Files matching `pattern`, sorted. Only the directory named by the pattern's literal
/// prefix is listed, and vendored snapshots are never matched.
fn glob_files(files: &dyn SpecFiles, pattern: &str) -> Vec<String> {
    let Some(matcher) = glob_matcher(pattern) else {
        return Vec::new();
    };
    let prefix: Vec<&str> = pattern
        .split('/')
        .take_while(|part| !is_glob(part))
        .collect();
    let vendor = format!("{VENDOR_DIR}/");
    files
        .list(&prefix.join("/"))
        .into_iter()
        .filter(|f| !f.starts_with(&vendor) && matcher.is_match(f))
        .collect()
}

pub(crate) fn glob_matcher(pattern: &str) -> Option<globset::GlobMatcher> {
    globset::GlobBuilder::new(pattern)
        .literal_separator(true)
        .build()
        .ok()
        .map(|g| g.compile_matcher())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::SpecEntry;

    fn interface(specs: &[(&str, &str)]) -> Interface {
        Interface {
            specs: specs
                .iter()
                .map(|(format, path)| SpecEntry {
                    format: (*format).to_owned(),
                    path: (*path).to_owned(),
                    element: None,
                })
                .collect(),
        }
    }

    #[test]
    fn normalize_stays_inside_the_root() {
        assert_eq!(normalize("", "a/./b/../c").as_deref(), Some("a/c"));
        assert_eq!(
            normalize("api", "../schemas/x.yaml").as_deref(),
            Some("schemas/x.yaml")
        );
        assert_eq!(normalize("", "../x"), None);
        assert_eq!(normalize("api", "../../x"), None);
        assert_eq!(normalize("", "/etc/passwd"), None);
        assert_eq!(normalize("", "C:\\x"), None);
        assert_eq!(normalize("", "https://example.com/x"), None);
    }

    // rqtk: verifies VA-CORE-0008-02
    #[test]
    fn crlf_and_lf_checkouts_hash_identically() {
        let lf = tempfile::tempdir().unwrap();
        let crlf = tempfile::tempdir().unwrap();
        std::fs::write(
            lf.path().join("a.proto"),
            "syntax = \"proto3\";\nmessage A {}\n",
        )
        .unwrap();
        std::fs::write(
            crlf.path().join("a.proto"),
            "syntax = \"proto3\";\r\nmessage A {}\r\n",
        )
        .unwrap();
        let spec = interface(&[("protobuf", "a.proto")]);
        assert_eq!(
            resolve(&DirFiles::working_tree(lf.path()), &spec).digest,
            resolve(&DirFiles::working_tree(crlf.path()), &spec).digest
        );
    }

    // rqtk: verifies VA-CORE-0022-01
    #[test]
    fn glob_matches_are_hashed() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("proto/v1")).unwrap();
        std::fs::write(dir.path().join("proto/a.proto"), "a").unwrap();
        std::fs::write(dir.path().join("proto/v1/b.proto"), "b").unwrap();
        std::fs::write(dir.path().join("proto/v1/notes.md"), "n").unwrap();
        let spec = interface(&[("protobuf", "proto/**/*.proto")]);
        let first = resolve(&DirFiles::working_tree(dir.path()), &spec);
        assert!(first.problems.is_empty(), "{:?}", first.problems);
        assert_eq!(
            first.files.iter().map(String::as_str).collect::<Vec<_>>(),
            ["proto/a.proto", "proto/v1/b.proto"]
        );
        std::fs::write(dir.path().join("proto/v1/b.proto"), "changed").unwrap();
        assert_ne!(
            resolve(&DirFiles::working_tree(dir.path()), &spec).digest,
            first.digest
        );
        std::fs::write(dir.path().join("proto/v1/notes.md"), "changed").unwrap();
        std::fs::write(dir.path().join("proto/v1/b.proto"), "b").unwrap();
        assert_eq!(
            resolve(&DirFiles::working_tree(dir.path()), &spec).digest,
            first.digest
        );
    }

    // rqtk: verifies VA-CORE-0022-02
    #[test]
    fn only_the_repositorys_own_ignore_files_apply() {
        let outer = tempfile::tempdir().unwrap();
        // An ignore file above the repository, and one in a vendored snapshot's directory,
        // must not hide spec files.
        std::fs::write(outer.path().join(".gitignore"), "*.proto\n").unwrap();
        let root = outer.path().join("repo");
        std::fs::create_dir_all(root.join("proto")).unwrap();
        std::fs::write(root.join("proto/a.proto"), "a").unwrap();
        std::fs::write(root.join("proto/generated.proto"), "g").unwrap();
        let spec = interface(&[("protobuf", "proto/*.proto")]);
        let files = |r: &SpecResolution| r.files.iter().cloned().collect::<Vec<_>>();

        let local = resolve(&DirFiles::working_tree(&root), &spec);
        assert_eq!(files(&local), ["proto/a.proto", "proto/generated.proto"]);

        std::fs::write(root.join(".gitignore"), "generated.proto\n").unwrap();
        let local = resolve(&DirFiles::working_tree(&root), &spec);
        assert_eq!(files(&local), ["proto/a.proto"]);

        let vendored = resolve(&DirFiles::snapshot(&root), &spec);
        assert_eq!(files(&vendored), ["proto/a.proto", "proto/generated.proto"]);
    }

    // rqtk: verifies VA-CORE-0009-01
    #[test]
    fn missing_path_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let r = resolve(
            &DirFiles::working_tree(dir.path()),
            &interface(&[("protobuf", "nope/*.proto")]),
        );
        assert_eq!(r.problems.len(), 1);
        assert_eq!(r.problems[0].code, "RQ032");
    }

    // rqtk: verifies VA-CORE-0025-03
    #[test]
    fn escaping_spec_paths_are_reported() {
        let outer = tempfile::tempdir().unwrap();
        let root = outer.path().join("repo");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(outer.path().join("secret.proto"), "s").unwrap();
        for path in ["../secret.proto", "/etc/hosts", "a/../../secret.proto"] {
            let r = resolve(
                &DirFiles::working_tree(&root),
                &interface(&[("protobuf", path)]),
            );
            assert_eq!(r.problems.len(), 1, "{path}: {:?}", r.problems);
            assert_eq!(r.problems[0].code, "RQ036", "{path}");
            assert!(r.files.is_empty(), "{path}");
        }
    }

    // rqtk: verifies VA-CORE-0009-02
    #[test]
    fn unknown_format_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.graphql"), "type Q").unwrap();
        let r = resolve(
            &DirFiles::working_tree(dir.path()),
            &interface(&[("graphql", "a.graphql")]),
        );
        assert_eq!(r.problems.len(), 1);
        assert_eq!(r.problems[0].code, "RQ033");
    }

    // rqtk: verifies VA-CORE-0019-01
    #[test]
    fn empty_interface_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let r = resolve(&DirFiles::working_tree(dir.path()), &Interface::default());
        assert_eq!(r.problems.len(), 1);
        assert_eq!(r.problems[0].code, "RQ034");
        assert_eq!(r.problems[0].entry, None);
    }
}
