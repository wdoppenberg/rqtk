use crate::diagnostic::Severity;
use serde::Serialize;

/// A lint rule: its stable code, severity, what it checks and how to fix a finding.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Rule {
    pub code: &'static str,
    pub severity: Severity,
    pub summary: &'static str,
    pub fix: &'static str,
}

use Severity::{Error as E, Warning as W};

/// Every diagnostic code rqtk can emit. Codes are stable: a retired code is never reused.
#[rustfmt::skip]
pub const RULES: &[Rule] = &[
    Rule { code: "RQ001", severity: E, summary: "requirement ID does not match the ID pattern", fix: "Rename the ID (and file) to `<prefix>-<CATEGORY>-<NUMBER>`, as `rqtk add` does, or set `identification.id_pattern` in .rqtk/config.toml to accept it." },
    Rule { code: "RQ002", severity: E, summary: "unknown category", fix: "Use a key from `[categories]` in .rqtk/config.toml, or add the category there." },
    Rule { code: "RQ003", severity: E, summary: "unknown requirement type", fix: "Use a value from `types.allowed`, or add the type there." },
    Rule { code: "RQ004", severity: E, summary: "invalid lifecycle state", fix: "Use a value from `lifecycle.states`." },
    Rule { code: "RQ005", severity: E, summary: "invalid priority", fix: "Use a value from `priority.levels`." },
    Rule { code: "RQ006", severity: E, summary: "invalid criticality", fix: "Use a value from `criticality.levels`, or remove `criticality`." },
    Rule { code: "RQ007", severity: E, summary: "rationale is required but missing", fix: "Add `rationale = \"…\"` explaining why the requirement exists." },
    Rule { code: "RQ008", severity: E, summary: "verification method is required but missing", fix: "Set `verification.method` to a value from `verification.methods`." },
    Rule { code: "RQ009", severity: E, summary: "invalid verification method", fix: "Use a value from `verification.methods`." },
    Rule { code: "RQ010", severity: E, summary: "statement is not exactly one sentence with a shall keyword", fix: "Rewrite `statement` as one sentence using a keyword from `validation.shall_keywords`; split compound requirements." },
    Rule { code: "RQ011", severity: E, summary: "statement uses a forbidden keyword", fix: "Replace the vague word with a measurable criterion." },
    Rule { code: "RQ012", severity: E, summary: "category requires at least one parent", fix: "Add the requirement it decomposes to `trace.parents`." },
    Rule { code: "RQ013", severity: E, summary: "TBD is not allowed by project policy", fix: "Resolve the open item and set `tbd = false`." },
    Rule { code: "RQ014", severity: E, summary: "TBR is not allowed by project policy", fix: "Resolve the open item and set `tbr = false`." },
    Rule { code: "RQ015", severity: E, summary: "unknown parent reference", fix: "Correct the ID in `trace.parents` or create the parent requirement." },
    Rule { code: "RQ016", severity: E, summary: "unknown depends_on reference", fix: "Correct the ID in `trace.depends_on`." },
    Rule { code: "RQ017", severity: E, summary: "requirement is part of a traceability cycle", fix: "Remove one of the parents/depends_on/derived_from/refines links named in the message." },
    Rule { code: "RQ018", severity: W, summary: "orphan requirement has no path to a root category", fix: "Add a `trace.parents` chain that reaches a category with `is_root = true`." },
    Rule { code: "RQ019", severity: E, summary: "repository is missing a required directory", fix: "Create the directory, or remove it from `repository.required_dirs`." },
    Rule { code: "RQ020", severity: E, summary: "repository is missing a required file", fix: "Create the file, or remove it from `repository.required_files`." },
    Rule { code: "RQ021", severity: W, summary: "content hash is stale", fix: "Run `rqtk rehash`." },
    Rule { code: "RQ022", severity: E, summary: "`trace.satisfies` references an unknown need", fix: "Correct the need ID or create it with `rqtk add-need`." },
    Rule { code: "RQ023", severity: E, summary: "reference to an unknown stakeholder", fix: "Correct the stakeholder ID or create it with `rqtk add-stakeholder`." },
    Rule { code: "RQ024", severity: E, summary: "invalid verification level", fix: "Use a value from `verification.levels`." },
    Rule { code: "RQ025", severity: E, summary: "invalid verification phase", fix: "Use a value from `verification.phases`." },
    Rule { code: "RQ026", severity: E, summary: "unknown derived_from / refines / conflicts_with / related reference", fix: "Correct the ID in the named `trace` field." },
    Rule { code: "RQ027", severity: E, summary: "verification activity ID is defined more than once", fix: "Give each `[[verification.activities]]` entry a unique `id`." },
    Rule { code: "RQ028", severity: E, summary: "`verifies` annotation names an unknown verification activity", fix: "Correct the activity ID in the annotation, or add the activity to a requirement." },
    Rule { code: "RQ029", severity: W, summary: "hand-written status on an activity whose status comes from tests", fix: "Remove `status` (and `executed_at`) from the activity; run tests and `rqtk verify` instead." },
    Rule { code: "RQ030", severity: E, summary: "`verifies` annotation is not followed by a test rqtk recognises", fix: "Place the annotation directly above the test, with only comments and attributes in between. For one case of a table-driven test, tag its row: `// rqtk: verifies VA-… case \"name\"`. `rqtk scan` shows what each annotation is attached to." },
    Rule { code: "RQ031", severity: W, summary: "an activity's evidence file does not exist", fix: "Correct the path (relative to the repository root), add the file, or link to it with a URL." },
    Rule { code: "RQ032", severity: E, summary: "interface spec path or glob matches no file", fix: "Correct `path` in the `[[interface.specs]]` entry (relative to the repository root), or add the file." },
    Rule { code: "RQ033", severity: E, summary: "unknown interface spec format", fix: "Set `format` to one of openapi, protobuf, asyncapi, jsonschema or other." },
    Rule { code: "RQ034", severity: E, summary: "`[interface]` table lists no spec files", fix: "Add an `[[interface.specs]]` entry with `format` and `path`, or remove the `[interface]` table." },
    Rule { code: "RQ035", severity: E, summary: "OpenAPI document cannot be parsed", fix: "Fix the YAML or JSON at the reported file. rqtk reads OpenAPI 3.0 and 3.1, rejects YAML alias expansion beyond a fixed budget and nesting deeper than 128 levels." },
    Rule { code: "RQ036", severity: E, summary: "spec path or OpenAPI `$ref` points to a URL or outside the repository", fix: "Use repository-relative paths and references; rqtk never fetches URLs. Copy the referenced definition into the repository instead." },
    Rule { code: "RQ037", severity: E, summary: "OpenAPI element pointer or `$ref` does not resolve", fix: "Point `element` (or the `$ref`) at an existing node, written as a JSON pointer such as `#/paths/~1users~1{id}/get` (`~1` encodes `/`)." },
    Rule { code: "RQ038", severity: E, summary: "`consumes` link names an undeclared interface source", fix: "Write the link as `<alias>:<ID>` and declare the alias with `rqtk interfaces add <alias> <url> --ref <ref>`." },
    Rule { code: "RQ039", severity: E, summary: "`consumes` link names a requirement absent from the vendored snapshot", fix: "The provider no longer exposes it at the pinned ref: update the link, pin another ref, or drop the dependency." },
    Rule { code: "RQ040", severity: E, summary: "vendored file does not match the interface lockfile", fix: "Vendored content must not be edited. Restore it with `rqtk interfaces sync <alias>` and review the diff." },
    Rule { code: "RQ041", severity: E, summary: "interface sources in the config and the lockfile disagree", fix: "Run `rqtk interfaces sync` after changing `[interfaces]`, or `rqtk interfaces remove <alias>` for a source you no longer use." },
    Rule { code: "RQ042", severity: W, summary: "interface source pins a git branch", fix: "Pin a tag (ideally an `rqtk baseline`) or a commit SHA, so the same config always means the same contract." },
    Rule { code: "RQ043", severity: E, summary: "vendored requirement cannot be read", fix: "The snapshot was written by a different rqtk or edited; run `rqtk interfaces sync <alias>`." },
    Rule { code: "RQ100", severity: E, summary: "file could not be read or parsed (syntax error, unknown or missing field)", fix: "Fix the TOML at the reported line; `rqtk schema <kind>` lists the allowed fields." },
    Rule { code: "RQ101", severity: E, summary: "ID is defined in more than one file", fix: "Give one of the items a new ID and rename its file to match." },
    Rule { code: "RQ102", severity: E, summary: "file name does not match the ID it contains", fix: "Rename the file to `<ID>.toml`." },
];

pub fn rule(code: &str) -> Option<&'static Rule> {
    RULES.iter().find(|r| r.code == code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn codes_are_unique_and_sorted() {
        let codes: Vec<&str> = RULES.iter().map(|r| r.code).collect();
        assert_eq!(codes.iter().collect::<HashSet<_>>().len(), codes.len());
        let mut sorted = codes.clone();
        sorted.sort();
        assert_eq!(codes, sorted);
    }
}
