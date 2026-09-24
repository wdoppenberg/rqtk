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
//! // rqtk: verifies VA-SYS-001-02 case "flipped byte is corrupt"
//! ```
//!
//! The link belongs to the test declared directly below it: only blank lines, comments,
//! other tags and attributes, decorators or annotations may sit in between. A tag with a
//! `case` may instead sit inside a test, on a row of its table; it then belongs to the
//! enclosing test and to that one case.

use crate::error::RqtkError;
use crate::model::ScanConfig;
use ignore::WalkBuilder;
use ignore::overrides::OverrideBuilder;
use regex::Regex;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

/// A `verifies` annotation found in source code.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct SourceLink {
    /// Verification activity ID named by the annotation.
    pub activity: String,
    /// Path relative to the repository root.
    pub path: PathBuf,
    /// 1-based line of the annotation.
    pub line: usize,
    /// Name of the annotated test, or of the test group for a `describe` block. `None` when
    /// no test declaration follows the annotation.
    pub test_name: Option<String>,
    /// Suite the test runner reports with the name, such as `Suite` in GoogleTest's
    /// `TEST(Suite, Name)`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suite: Option<String>,
    /// One case of a table-driven or parameterised test.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub case: Option<String>,
    /// The annotation names a group of tests (`describe`, `context`) rather than one test.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub group: bool,
    /// 1-based line of the test declaration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test_line: Option<usize>,
    /// Hash of the test's source (its attributes, declaration and body), used to tell
    /// whether a test changed between runs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_hash: Option<String>,
}

/// Lines of comments, attributes and other tags allowed between a tag and its test.
const MAX_GAP: usize = 40;
/// Longest test body read for the source hash.
const MAX_BODY: usize = 2000;

static ANNOTATION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"^\s*(?:#\[|@)\s*(?:[\w:.]+(?:::|\.))?verifies\(\s*"([^"]+)"\s*(?:,\s*case\s*=\s*"([^"]*)"\s*)?,?\s*\)"#,
    )
    .unwrap()
});
static COMMENT_TAG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"^\s*(?://+|#|/\*+|\*|--)\s*rqtk:\s*verifies\s+([\w.:-]+)(?:\s+case\s+"([^"]*)")?"#,
    )
    .unwrap()
});

/// Test declarations, tried in order on the first code line below a tag.
static DECLARATIONS: LazyLock<Vec<(Shape, Regex)>> = LazyLock::new(|| {
    let quoted = r#"(?:"((?:[^"\\]|\\.)*)"|'((?:[^'\\]|\\.)*)'|`([^`]*)`)"#;
    let js_modifiers = r"(?:\s*\.\s*(?:only|skip|todo|concurrent|sequential|failing|fails|skipIf\([^)]*\)|runIf\([^)]*\)))*";
    let js_each = r"(?:\s*\.\s*each\s*(?:\(.*\)|`[^`]*`))?";
    let rx = |s: &str| Regex::new(s).unwrap();
    vec![
        // GoogleTest: TEST(Suite, Name), TEST_F, TEST_P, TYPED_TEST
        (
            Shape::Suite,
            rx(r"^(?:TEST|TEST_F|TEST_P|TYPED_TEST|TYPED_TEST_P)\s*\(\s*(\w+)\s*,\s*(\w+)\s*\)"),
        ),
        // Catch2, doctest: TEST_CASE("name"), TEST_CASE_METHOD(Fixture, "name"); MSVC TEST_METHOD(Name)
        (
            Shape::Name,
            rx(&format!(
                r"^(?:TEST_CASE|SCENARIO|TEST_CASE_METHOD|TEST_CASE_FIXTURE)\s*\(\s*(?:\w+\s*,\s*)?{quoted}"
            )),
        ),
        (Shape::Name, rx(r"^TEST_METHOD\s*\(\s*(\w+)")),
        // JS/TS, RSpec, Scala: it("…"), test.each(…)("…"), it "…" do
        (
            Shape::Name,
            rx(&format!(
                r"^(?:it|test|specify|scenario){js_modifiers}{js_each}\s*\(?\s*{quoted}"
            )),
        ),
        (
            Shape::Group,
            rx(&format!(
                r"^(?:describe|context|suite){js_modifiers}{js_each}\s*\(?\s*{quoted}"
            )),
        ),
        // Zig, Elixir: test "name"
        (Shape::Name, rx(&format!(r"^test\s+{quoted}"))),
        // Rust
        (Shape::Name, rx(r"\bfn\s+(\w+)")),
        // Python, Ruby
        (
            Shape::Name,
            rx(r"^(?:async\s+)?def\s+(?:self\.)?(\w+[?!]?)"),
        ),
        // Go (with an optional receiver), Swift
        (Shape::Name, rx(r"\bfunc\s+(?:\([^)]*\)\s*)?(\w+)")),
        // Kotlin, including `backticked names`
        (
            Shape::Name,
            rx(r"\bfun\s+(?:<[^>]*>\s*)?(?:[\w.]+\.)?(?:`([^`]+)`|(\w+))"),
        ),
        // JavaScript
        (Shape::Name, rx(r"\bfunction\s*\*?\s*(\w+)")),
        // Java, C#, C, C++: modifiers, a return type, then name(
        (
            Shape::Method,
            rx(
                r"^(?:(?:public|protected|private|internal|static|final|abstract|async|override|virtual|synchronized|open|suspend|inline|extern|unsafe|sealed|partial|new|default)\s+)*((?:[\w.:<>\[\],?*&]+\s+)+?)(?:[*&]\s*)?(?:\w+::)*(\w+)\s*\(",
            ),
        ),
    ]
});

#[derive(Clone, Copy)]
enum Shape {
    /// The first non-empty capture is the name.
    Name,
    /// A group of tests (`describe`); the first non-empty capture is its title.
    Group,
    /// Captures 1 and 2 are the suite and the name.
    Suite,
    /// Capture 1 is the return type, capture 2 the name.
    Method,
}

/// Words that can't be a return type or a method name, so a line starting with them is a
/// statement rather than a method declaration.
const NOT_A_METHOD: &[&str] = &[
    "return",
    "if",
    "else",
    "while",
    "for",
    "foreach",
    "switch",
    "new",
    "throw",
    "case",
    "await",
    "yield",
    "catch",
    "using",
    "delete",
    "sizeof",
    "typeof",
    "lock",
    "when",
    "do",
    "try",
    "assert",
    "print",
    "println",
    "echo",
    "import",
    "package",
    "class",
    "struct",
    "enum",
    "interface",
    "namespace",
    "var",
    "let",
    "const",
    "val",
];

struct Declaration {
    name: String,
    suite: Option<String>,
    group: bool,
}

fn declaration(code: &str) -> Option<Declaration> {
    let code = code.trim();
    for (shape, rx) in DECLARATIONS.iter() {
        let Some(c) = rx.captures(code) else { continue };
        let first = || {
            c.iter()
                .skip(1)
                .flatten()
                .map(|m| m.as_str().to_owned())
                .next()
        };
        let found = match shape {
            Shape::Name => first().map(|name| Declaration {
                name,
                suite: None,
                group: false,
            }),
            Shape::Group => first().map(|name| Declaration {
                name,
                suite: None,
                group: true,
            }),
            Shape::Suite => Some(Declaration {
                name: c[2].to_owned(),
                suite: Some(c[1].to_owned()),
                group: false,
            }),
            Shape::Method => {
                let first_word = c[1].split_whitespace().next().unwrap_or_default();
                let name = &c[2];
                (!NOT_A_METHOD.contains(&first_word) && !NOT_A_METHOD.contains(&name)).then(|| {
                    Declaration {
                        name: name.to_owned(),
                        suite: None,
                        group: false,
                    }
                })
            }
        };
        if found.is_some() {
            return found;
        }
    }
    None
}

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
        let case = caps.get(2).map(|m| m.as_str().to_owned());
        let mut link = SourceLink {
            activity: caps[1].to_owned(),
            path: path.to_path_buf(),
            line: idx + 1,
            case,
            ..SourceLink::default()
        };
        let below = declaration_below(&lines, idx);
        let found = match below {
            Some(found) => Some((found, idx + 1)),
            // A case tag on a table row belongs to the enclosing test.
            None if link.case.is_some() => enclosing_declaration(&lines, idx).map(|f| {
                let at = f.0;
                (f, at)
            }),
            None => None,
        };
        if let Some(((decl_idx, decl), hash_from)) = found {
            link.test_name = Some(decl.name);
            link.suite = decl.suite;
            link.group = decl.group;
            link.test_line = Some(decl_idx + 1);
            link.source_hash = Some(source_hash(&lines, hash_from, decl_idx));
        }
        links.push(link);
    }
    links
}

/// The test declared directly below the tag on line `tag`, skipping blank lines, comments,
/// other tags, and attributes (which may span several lines).
fn declaration_below(lines: &[&str], tag: usize) -> Option<(usize, Declaration)> {
    let mut depth = 0i32;
    for (idx, line) in lines.iter().enumerate().skip(tag + 1).take(MAX_GAP) {
        let code = line.trim();
        if depth > 0 {
            depth += bracket_delta(code);
            continue;
        }
        if code.is_empty() || is_comment(code) {
            continue;
        }
        if is_attribute(code) {
            depth = bracket_delta(code).max(0);
            continue;
        }
        return declaration(code).map(|d| (idx, d));
    }
    None
}

/// The nearest test declared above line `tag` at a smaller indentation.
fn enclosing_declaration(lines: &[&str], tag: usize) -> Option<(usize, Declaration)> {
    let indent = indentation(lines[tag]);
    lines[..tag]
        .iter()
        .enumerate()
        .rev()
        .filter(|(_, l)| !l.trim().is_empty() && indentation(l) < indent)
        .find_map(|(idx, l)| declaration(l).map(|d| (idx, d)))
}

fn is_comment(code: &str) -> bool {
    code.starts_with("//")
        || code.starts_with("/*")
        || code.starts_with('*')
        || code.starts_with("--")
        || (code.starts_with('#') && !code.starts_with("#[") && !code.starts_with("#!["))
}

/// Rust `#[…]`, Python/Java/Kotlin/TypeScript `@…`, and C# `[Attribute]`.
fn is_attribute(code: &str) -> bool {
    code.starts_with("#[")
        || code.starts_with('@')
        || (code.starts_with('[') && code[1..].starts_with(|c: char| c.is_ascii_uppercase()))
}

/// Opening minus closing brackets on a line, outside double-quoted and backtick strings.
fn bracket_delta(code: &str) -> i32 {
    let mut delta = 0;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for c in code.chars() {
        match quote {
            Some(q) => {
                if escaped {
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == q {
                    quote = None;
                }
            }
            None => match c {
                '"' | '`' => quote = Some(c),
                '(' | '[' | '{' => delta += 1,
                ')' | ']' | '}' => delta -= 1,
                _ => {}
            },
        }
    }
    delta
}

fn indentation(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// Hash of lines `from` up to the end of the test declared on line `decl`: to the closing
/// brace for brace-delimited bodies, else over the lines indented deeper than the
/// declaration (Python, Ruby). Whitespace and blank lines don't count.
fn source_hash(lines: &[&str], from: usize, decl: usize) -> String {
    let end = body_end(lines, decl);
    let mut hasher = Sha256::new();
    for line in &lines[from.min(decl)..=end] {
        let code = line.trim();
        if !code.is_empty() && !COMMENT_TAG.is_match(line) {
            hasher.update(code.as_bytes());
            hasher.update(b"\n");
        }
    }
    let digest = hasher.finalize();
    digest[..8].iter().map(|b| format!("{b:02x}")).collect()
}

fn body_end(lines: &[&str], decl: usize) -> usize {
    let last = (decl + MAX_BODY).min(lines.len() - 1);
    // Brace-delimited: from the first `{` on the declaration line or below it.
    let mut depth = 0i32;
    let mut opened = false;
    for (idx, line) in lines.iter().enumerate().take(last + 1).skip(decl) {
        if !opened && idx > decl && !line.contains('{') && line.trim().ends_with(':') {
            break;
        }
        depth += brace_delta(line);
        opened |= line.contains('{');
        if opened && depth <= 0 {
            return idx;
        }
        if !opened && idx > decl + 2 {
            break;
        }
    }
    // Indentation-delimited.
    let base = indentation(lines[decl]);
    let mut end = decl;
    for (idx, line) in lines.iter().enumerate().take(last + 1).skip(decl + 1) {
        let code = line.trim();
        if code.is_empty() {
            continue;
        }
        if indentation(line) > base {
            end = idx;
        } else {
            if code == "end" || code.starts_with('}') || code.starts_with(')') {
                end = idx;
            }
            break;
        }
    }
    end
}

fn brace_delta(line: &str) -> i32 {
    let mut delta = 0;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for c in line.chars() {
        match quote {
            Some(q) => {
                if escaped {
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == q {
                    quote = None;
                }
            }
            None => match c {
                '"' | '`' => quote = Some(c),
                '{' => delta += 1,
                '}' => delta -= 1,
                _ => {}
            },
        }
    }
    delta
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
    fn jvm_dotnet_and_c_family_methods() {
        let java =
            "  // rqtk: verifies A\n  @Test\n  @DisplayName(\"x\")\n  public void plain() {}\n";
        assert_eq!(names(java), vec![link("A", "plain")]);
        let kotlin = "// rqtk: verifies A\n@Test\nfun `rounds down`() {}\n";
        assert_eq!(names(kotlin), vec![link("A", "rounds down")]);
        let csharp = "// rqtk: verifies A\n[Fact]\npublic async Task ClampsNegative() {}\n";
        assert_eq!(names(csharp), vec![link("A", "ClampsNegative")]);
        let c = "/* rqtk: verifies A */\nstatic void test_decoder(void) {}\n";
        assert_eq!(names(c), vec![link("A", "test_decoder")]);
    }

    // rqtk: verifies VA-CORE-006-01
    #[test]
    fn googletest_catch2_and_suites() {
        let links = scan_text(
            Path::new("t.cpp"),
            "// rqtk: verifies A\nTEST_P(Params, IsPositive) {}\n",
        );
        assert_eq!(links[0].test_name.as_deref(), Some("IsPositive"));
        assert_eq!(links[0].suite.as_deref(), Some("Params"));
        let catch = "// rqtk: verifies A\nTEST_CASE(\"decodes a frame\", \"[can]\") {}\n";
        assert_eq!(names(catch), vec![link("A", "decodes a frame")]);
    }

    // rqtk: verifies VA-CORE-006-01
    #[test]
    fn js_modifiers_each_and_groups() {
        let each =
            "// rqtk: verifies A\nit.each([[1, 2], [3, 4]])(\"adds %i and %i\", (a, b) => {});\n";
        assert_eq!(names(each), vec![link("A", "adds %i and %i")]);
        let skip = "// rqtk: verifies A\ntest.skip('later', () => {});\n";
        assert_eq!(names(skip), vec![link("A", "later")]);
        let links = scan_text(
            Path::new("x.ts"),
            "// rqtk: verifies A\ndescribe(\"grouped\", () => {\n  it(\"one\", () => {});\n});\n",
        );
        assert_eq!(links[0].test_name.as_deref(), Some("grouped"));
        assert!(links[0].group);
    }

    // rqtk: verifies VA-CORE-006-01
    #[test]
    fn binds_only_to_the_declaration_directly_below() {
        // An unrecognised line between the tag and the next test stops the search, so the
        // tag can't attach to a later, unrelated test.
        let src = "// rqtk: verifies A\nconst helper = () => 1;\n\nit(\"unrelated\", () => {});\n";
        assert_eq!(names(src), vec![("A".to_owned(), None)]);
        // Statements aren't method declarations.
        assert_eq!(
            names("// rqtk: verifies A\nreturn check(x);\n"),
            vec![("A".to_owned(), None)]
        );
        // Multi-line attributes and decorators are skipped as a whole.
        let py = "# rqtk: verifies A\n@pytest.mark.parametrize(\n    \"n\",\n    [1, 2],\n)\ndef test_n(n):\n    pass\n";
        assert_eq!(names(py), vec![link("A", "test_n")]);
    }

    // rqtk: verifies VA-CORE-006-01
    #[test]
    fn case_tags_on_table_rows_bind_to_the_enclosing_test() {
        let go = concat!(
            "func TestIntact(t *testing.T) {\n",
            "\tcases := []struct{ name string }{\n",
            "\t\t// rqtk: verifies A case \"flipped byte is corrupt\"\n",
            "\t\t{\"flipped byte is corrupt\"},\n",
            "\t}\n",
            "}\n",
        );
        let links = scan_text(Path::new("x_test.go"), go);
        assert_eq!(links[0].test_name.as_deref(), Some("TestIntact"));
        assert_eq!(links[0].case.as_deref(), Some("flipped byte is corrupt"));
        let py = "@verifies(\"A\", case=\"neg\")\ndef test_sign(sign):\n    pass\n";
        assert_eq!(
            scan_text(Path::new("t.py"), py)[0].case.as_deref(),
            Some("neg")
        );
    }

    // rqtk: verifies VA-CORE-006-01
    #[test]
    fn source_hash_covers_the_body_only() {
        let hash = |src: &str| {
            scan_text(Path::new("x.rs"), src)[0]
                .source_hash
                .clone()
                .unwrap()
        };
        let base =
            "// rqtk: verifies A\n#[test]\nfn t() {\n    assert!(true);\n}\n\nfn other() {}\n";
        let reformatted = "// rqtk: verifies A\n#[test]\nfn t() {\n\n        assert!(true);\n}\n\nfn other() { 1 }\n";
        let changed = "// rqtk: verifies A\n#[test]\nfn t() {\n    assert!(false);\n}\n";
        assert_eq!(hash(base), hash(reformatted));
        assert_ne!(hash(base), hash(changed));
        let py = |body: &str| {
            hash_py(&format!(
                "# rqtk: verifies A\ndef test_x():\n    {body}\n\ndef later():\n    pass\n"
            ))
        };
        assert_ne!(py("assert 1"), py("assert 2"));
    }

    fn hash_py(src: &str) -> String {
        scan_text(Path::new("x.py"), src)[0]
            .source_hash
            .clone()
            .unwrap()
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
