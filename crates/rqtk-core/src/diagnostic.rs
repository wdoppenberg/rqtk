use crate::model::EntityRef;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

/// A position in a source file. `line` and `column` are 1-based.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Location {
    pub path: PathBuf,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

/// A single finding from loading or linting a requirement set.
#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub severity: Severity,
    /// Stable rule code, e.g. `RQ007`. See [`crate::rules::RULES`].
    pub code: &'static str,
    pub message: String,
    /// The item the finding is about, if any.
    pub subject: Option<EntityRef>,
    pub location: Option<Location>,
    /// Dotted key path inside the file (e.g. `trace.parents`) used to resolve `location.line`.
    #[serde(skip)]
    pub field: Option<&'static str>,
}

impl Diagnostic {
    /// A finding for rule `code`; its severity comes from [`crate::rules::RULES`].
    ///
    /// # Panics
    /// If `code` is not in the rule table.
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        let rule = crate::rules::rule(code).unwrap_or_else(|| panic!("unknown rule code {code}"));
        Self {
            severity: rule.severity,
            code,
            message: message.into(),
            subject: None,
            location: None,
            field: None,
        }
    }

    pub fn subject(mut self, subject: EntityRef) -> Self {
        self.subject = Some(subject);
        self
    }

    pub fn file(mut self, path: impl Into<PathBuf>) -> Self {
        self.location = Some(Location {
            path: path.into(),
            line: None,
            column: None,
        });
        self
    }

    pub fn at(mut self, path: impl Into<PathBuf>, line: usize, column: usize) -> Self {
        self.location = Some(Location {
            path: path.into(),
            line: Some(line),
            column: Some(column),
        });
        self
    }

    /// Point at a dotted key inside the file; the line is resolved by [`resolve_lines`].
    pub fn field(mut self, field: &'static str) -> Self {
        self.field = Some(field);
        self
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}

/// Fill in `line`/`column` for diagnostics that name a `field` but have no position yet.
/// Each file is parsed at most once; files that fail to parse are left unresolved.
pub fn resolve_lines(diagnostics: &mut [Diagnostic]) {
    let mut docs: BTreeMap<PathBuf, Option<(String, toml_edit::Document<String>)>> =
        BTreeMap::new();
    for diag in diagnostics.iter_mut() {
        let (Some(field), Some(loc)) = (diag.field, diag.location.as_mut()) else {
            continue;
        };
        if loc.line.is_some() {
            continue;
        }
        let entry = docs
            .entry(loc.path.clone())
            .or_insert_with(|| parse_with_spans(&loc.path));
        let Some((text, doc)) = entry else { continue };
        if let Some(offset) = key_offset(doc, field) {
            let (line, column) = line_col(text, offset);
            loc.line = Some(line);
            loc.column = Some(column);
        }
    }
}

fn parse_with_spans(path: &Path) -> Option<(String, toml_edit::Document<String>)> {
    let text = std::fs::read_to_string(path).ok()?;
    let doc = toml_edit::Document::parse(text.clone()).ok()?;
    Some((text, doc))
}

/// Byte offset of the deepest key of `field` that exists in the document.
fn key_offset(doc: &toml_edit::Document<String>, field: &str) -> Option<usize> {
    let mut table: &dyn toml_edit::TableLike = doc.as_table();
    let mut best = None;
    for part in field.split('.') {
        let Some((key, item)) = table.get_key_value(part) else {
            break;
        };
        best = key.span().map(|s| s.start).or(best);
        match item.as_table_like() {
            Some(next) => table = next,
            None => break,
        }
    }
    best
}

/// Convert a byte offset into a 1-based (line, column) pair.
pub fn line_col(text: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(text.len());
    let before = &text[..offset];
    let line = before.matches('\n').count() + 1;
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    let column = text[line_start..offset].chars().count() + 1;
    (line, column)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_col_is_one_based_and_counts_chars() {
        let text = "a = 1\nbé = 2\n";
        assert_eq!(line_col(text, 0), (1, 1));
        assert_eq!(line_col(text, text.find('=').unwrap()), (1, 3));
        let second_eq = text.rfind('=').unwrap();
        assert_eq!(line_col(text, second_eq), (2, 4));
    }

    #[test]
    fn key_offset_finds_nested_keys() {
        let text = "id = \"x\"\n\n[trace]\nparents = []\n".to_owned();
        let doc = toml_edit::Document::parse(text.clone()).unwrap();
        let offset = key_offset(&doc, "trace.parents").unwrap();
        assert_eq!(line_col(&text, offset), (4, 1));
        // A missing leaf falls back to its nearest existing parent key.
        let offset = key_offset(&doc, "trace.depends_on").unwrap();
        assert_eq!(line_col(&text, offset), (3, 2));
        assert!(key_offset(&doc, "verification.method").is_none());
    }
}
