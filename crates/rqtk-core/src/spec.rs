//! Interface definition files: which files an `[interface]` table names, and the digest of
//! their content that goes into the requirement's content hash.
//!
//! OpenAPI entries are parsed (see [`crate::openapi`]): their `$ref`s are followed, their
//! element pointer is checked, and only the element and what it references are hashed, in a
//! canonical form. Every other format is opaque: each file a path or glob matches is hashed
//! whole, with line endings normalised to LF.

use crate::model::{ContentHasher, Interface, SPEC_FORMATS};
use std::collections::BTreeSet;
use std::path::Path;

/// Where vendored snapshots live; never matched by a glob of the repository's own specs.
const VENDOR_DIR: &str = ".rqtk/vendor";

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

    /// Whether the problem means a provider could make rqtk read outside its content
    /// (RQ036): `rqtk interfaces sync` refuses to vendor such an interface.
    pub fn escapes(&self) -> bool {
        self.code == "RQ036"
    }
}

/// Resolve the spec entries of `interface` against `root`: the repository root for a local
/// requirement, or a vendored snapshot's `files/` directory.
pub fn resolve(root: &Path, interface: &Interface) -> SpecResolution {
    let mut hasher = ContentHasher::default();
    let mut out = SpecResolution::default();
    if interface.specs.is_empty() {
        out.problems.push(SpecProblem {
            code: "RQ034",
            entry: None,
            message: "`[interface]` lists no spec files; add an `[[interface.specs]]` entry"
                .to_owned(),
            path: None,
        });
    }
    for (i, spec) in interface.specs.iter().enumerate() {
        hasher.field("spec", &i.to_string());
        if !SPEC_FORMATS.contains(&spec.format.as_str()) {
            out.problems.push(SpecProblem::new(
                "RQ033",
                i,
                format!(
                    "unknown spec format `{}`; use one of {}",
                    spec.format,
                    SPEC_FORMATS.join(", ")
                ),
            ));
            continue;
        }
        let Some(path) = normalize("", &spec.path) else {
            out.problems.push(SpecProblem::new(
                "RQ036",
                i,
                format!(
                    "spec path `{}` is absolute or points outside the repository",
                    spec.path
                ),
            ));
            continue;
        };
        if spec.format == "openapi" {
            let closure =
                crate::openapi::closure(root, &path, spec.element.as_deref(), i, &mut out.problems);
            for (key, canonical) in &closure.nodes {
                hasher.field(key, canonical);
            }
            out.files.extend(closure.files);
            continue;
        }
        let matched = if is_glob(&path) {
            glob_files(root, &path)
        } else if root.join(&path).is_file() {
            vec![path.clone()]
        } else {
            Vec::new()
        };
        if matched.is_empty() {
            out.problems.push(
                SpecProblem::new(
                    "RQ032",
                    i,
                    format!("spec path `{}` matches no file", spec.path),
                )
                .at(&path),
            );
        }
        for file in matched {
            match std::fs::read(root.join(&file)) {
                Ok(bytes) => {
                    hasher.field("file", &file);
                    hasher.bytes("content", &normalize_line_endings(&bytes));
                    out.files.insert(file);
                }
                Err(e) => out.problems.push(SpecProblem::new(
                    "RQ032",
                    i,
                    format!("cannot read spec file `{file}`: {e}"),
                )),
            }
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

fn is_glob(path: &str) -> bool {
    path.contains(['*', '?', '[', '{'])
}

/// Files below `root` matching `pattern`, sorted. The walk starts at the pattern's literal
/// prefix, honours `.gitignore`, does not follow symbolic links, and skips `.git` and the
/// vendored snapshots.
fn glob_files(root: &Path, pattern: &str) -> Vec<String> {
    let Ok(glob) = globset::GlobBuilder::new(pattern)
        .literal_separator(true)
        .build()
    else {
        return Vec::new();
    };
    let matcher = glob.compile_matcher();
    let prefix: Vec<&str> = pattern
        .split('/')
        .take_while(|part| !is_glob(part))
        .collect();
    let start = root.join(prefix.join("/"));
    if !start.is_dir() {
        return Vec::new();
    }
    let vendor = root.join(VENDOR_DIR);
    let mut files: Vec<String> = ignore::WalkBuilder::new(&start)
        .hidden(false)
        .require_git(false)
        .git_global(false)
        .filter_entry(move |e| e.file_name() != ".git" && e.path() != vendor)
        .build()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_some_and(|t| t.is_file()))
        .filter_map(|e| {
            let rel = e.path().strip_prefix(root).ok()?;
            let rel = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            matcher.is_match(&rel).then_some(rel)
        })
        .collect();
    files.sort();
    files
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
            resolve(lf.path(), &spec).digest,
            resolve(crlf.path(), &spec).digest
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
        let first = resolve(dir.path(), &spec);
        assert!(first.problems.is_empty(), "{:?}", first.problems);
        assert_eq!(
            first.files.iter().map(String::as_str).collect::<Vec<_>>(),
            ["proto/a.proto", "proto/v1/b.proto"]
        );
        std::fs::write(dir.path().join("proto/v1/b.proto"), "changed").unwrap();
        assert_ne!(resolve(dir.path(), &spec).digest, first.digest);
        std::fs::write(dir.path().join("proto/v1/notes.md"), "changed").unwrap();
        std::fs::write(dir.path().join("proto/v1/b.proto"), "b").unwrap();
        assert_eq!(resolve(dir.path(), &spec).digest, first.digest);
    }

    // rqtk: verifies VA-CORE-0009-01
    #[test]
    fn missing_path_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let r = resolve(dir.path(), &interface(&[("protobuf", "nope/*.proto")]));
        assert_eq!(r.problems.len(), 1);
        assert_eq!(r.problems[0].code, "RQ032");
    }

    // rqtk: verifies VA-CORE-0009-02
    #[test]
    fn unknown_format_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.graphql"), "type Q").unwrap();
        let r = resolve(dir.path(), &interface(&[("graphql", "a.graphql")]));
        assert_eq!(r.problems.len(), 1);
        assert_eq!(r.problems[0].code, "RQ033");
    }

    // rqtk: verifies VA-CORE-0019-01
    #[test]
    fn empty_interface_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let r = resolve(dir.path(), &Interface::default());
        assert_eq!(r.problems.len(), 1);
        assert_eq!(r.problems[0].code, "RQ034");
        assert_eq!(r.problems[0].entry, None);
    }
}
