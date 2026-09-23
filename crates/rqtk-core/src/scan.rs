//! Discovery of `verifies` links between source code and verification activities.
//!
//! A link is declared either by an attribute or decorator,
//!
//! ```text
//! #[verifies("VA-SYS-001-01")]        // Rust (rqtk-macros)
//! @verifies("VA-SYS-001-01")          # Python (rqtk package)
//! ```
//!
//! or, in any language, by a comment tag:
//!
//! ```text
//! // rqtk: verifies VA-SYS-001-01
//! ```
//!
//! The link is attributed to the next function declared within a few lines, whose name is
//! later matched against test results.

use crate::error::RqtkError;
use crate::model::ScanConfig;
use ignore::WalkBuilder;
use ignore::overrides::OverrideBuilder;
use regex::Regex;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

/// A `verifies` annotation found in source code.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct SourceLink {
    /// Verification activity ID named by the annotation.
    pub activity: String,
    /// Path relative to the repository root.
    pub path: PathBuf,
    /// 1-based line of the annotation.
    pub line: usize,
    /// Name of the annotated test function, if one was found.
    pub test_name: Option<String>,
}

/// Lines searched below an annotation for the function it annotates.
const LOOKAHEAD: usize = 12;

static ANNOTATION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^\s*(?:#\[|@)\s*(?:[\w:.]+(?:::|\.))?verifies\(\s*"([^"]+)"\s*\)"#).unwrap()
});
static COMMENT_TAG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*(?://+|#|/\*+|\*|--)\s*rqtk:\s*verifies\s+([\w.:-]+)").unwrap()
});
static FUNCTION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?:\bfn|\bdef|\bfunc|\bfunction)\s+([A-Za-z_]\w*)|\b(?:it|test)\(\s*["'`]([^"'`]+)["'`]"#,
    )
    .unwrap()
});

/// Scan the configured source paths under `repo_root` for `verifies` links.
/// Honours `.gitignore`; results are sorted by path and line.
pub fn scan(repo_root: &Path, config: &ScanConfig) -> Result<Vec<SourceLink>, RqtkError> {
    // Exclude globs are matched against paths relative to the override root, so walk
    // from the same normalised absolute root (no `.` segments).
    let repo_root = &repo_root.canonicalize().map_err(|source| RqtkError::Io {
        path: repo_root.to_path_buf(),
        source,
    })?;
    let mut overrides = OverrideBuilder::new(repo_root);
    for pattern in &config.exclude {
        overrides
            .add(&format!("!{pattern}"))
            .map_err(|e| RqtkError::InvalidScanPattern(pattern.clone(), e.to_string()))?;
    }
    let overrides = overrides
        .build()
        .map_err(|e| RqtkError::InvalidScanPattern(config.exclude.join(", "), e.to_string()))?;

    let mut links = Vec::new();
    for root in &config.paths {
        let start: PathBuf = repo_root
            .join(root)
            .components()
            .filter(|c| !matches!(c, std::path::Component::CurDir))
            .collect();
        let walk = WalkBuilder::new(start).overrides(overrides.clone()).build();
        for entry in walk.flatten() {
            let path = entry.path();
            let wanted = path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|ext| config.extensions.iter().any(|x| x == ext));
            if !wanted || !entry.file_type().is_some_and(|t| t.is_file()) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(path) else {
                continue; // not UTF-8 text
            };
            let rel = path.strip_prefix(repo_root).unwrap_or(path);
            links.extend(scan_text(rel, &text));
        }
    }
    links.sort();
    links.dedup();
    Ok(links)
}

/// Find links in one file's text. `path` is recorded as given.
pub fn scan_text(path: &Path, text: &str) -> Vec<SourceLink> {
    let lines: Vec<&str> = text.lines().collect();
    let mut links = Vec::new();
    for (idx, line) in lines.iter().enumerate() {
        let Some(caps) = ANNOTATION
            .captures(line)
            .or_else(|| COMMENT_TAG.captures(line))
        else {
            continue;
        };
        let test_name = lines.iter().skip(idx + 1).take(LOOKAHEAD).find_map(|l| {
            let c = FUNCTION.captures(l)?;
            c.get(1).or_else(|| c.get(2)).map(|m| m.as_str().to_owned())
        });
        links.push(SourceLink {
            activity: caps[1].to_owned(),
            path: path.to_path_buf(),
            line: idx + 1,
            test_name,
        });
    }
    links
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(text: &str) -> Vec<(String, Option<String>)> {
        scan_text(Path::new("x"), text)
            .into_iter()
            .map(|l| (l.activity, l.test_name))
            .collect()
    }

    fn link(activity: &str, test: &str) -> (String, Option<String>) {
        (activity.to_owned(), Some(test.to_owned()))
    }

    // rqtk: verifies VA-CORE-006-01
    #[test]
    fn rust_attributes_including_stacked_and_qualified() {
        let src = "#[verifies(\"A\")]\n#[rqtk_macros::verifies(\"B\")]\n#[test]\nfn both() {}\n";
        assert_eq!(names(src), vec![link("A", "both"), link("B", "both")]);
    }

    // rqtk: verifies VA-CORE-006-01
    #[test]
    fn python_decorators() {
        let src = "@rqtk.verifies(\"A\")\ndef test_a():\n    pass\n";
        assert_eq!(names(src), vec![link("A", "test_a")]);
    }

    // rqtk: verifies VA-CORE-006-01
    #[test]
    fn comment_tags_in_other_languages() {
        let go = "// rqtk: verifies VA-1\nfunc TestBoot(t *testing.T) {}\n";
        assert_eq!(names(go), vec![link("VA-1", "TestBoot")]);
        let js = "  // rqtk: verifies VA-2\n  it(\"boots quickly\", () => {})\n";
        assert_eq!(names(js), vec![link("VA-2", "boots quickly")]);
    }

    // rqtk: verifies VA-CORE-006-01
    #[test]
    fn ignores_commented_out_annotations_calls_and_docs() {
        let src = concat!(
            "// #[verifies(\"A\")]\n",
            "/// #[verifies(\"B\")]\n",
            "    x = rqtk.verifies(\"C\")\n",
            "let s = \"#[verifies(\\\"D\\\")]\";\n",
        );
        assert!(names(src).is_empty(), "{:?}", names(src));
    }

    // rqtk: verifies VA-CORE-006-01
    #[test]
    fn records_one_based_line_and_missing_function() {
        let links = scan_text(
            Path::new("f.rs"),
            "\n#[verifies(\"A\")]\nconst X: u8 = 1;\n",
        );
        assert_eq!(links[0].line, 2);
        assert_eq!(links[0].test_name, None);
    }
}
