/// A lint rule: its stable code and a one-line description of what it checks.
#[derive(Debug, Clone, Copy)]
pub struct Rule {
    pub code: &'static str,
    pub summary: &'static str,
}

/// Every diagnostic code rqtk can emit. Codes are stable: a retired code is never reused.
#[rustfmt::skip]
pub const RULES: &[Rule] = &[
    Rule { code: "RQ001", summary: "requirement ID does not match `identification.id_pattern`" },
    Rule { code: "RQ002", summary: "unknown category" },
    Rule { code: "RQ003", summary: "unknown requirement type" },
    Rule { code: "RQ004", summary: "invalid lifecycle state" },
    Rule { code: "RQ005", summary: "invalid priority" },
    Rule { code: "RQ006", summary: "invalid criticality" },
    Rule { code: "RQ007", summary: "rationale is required but missing" },
    Rule { code: "RQ008", summary: "verification method is required but missing" },
    Rule { code: "RQ009", summary: "invalid verification method" },
    Rule { code: "RQ010", summary: "statement is not exactly one sentence with a shall keyword" },
    Rule { code: "RQ011", summary: "statement uses a forbidden keyword" },
    Rule { code: "RQ012", summary: "category requires at least one parent" },
    Rule { code: "RQ013", summary: "TBD is not allowed by project policy" },
    Rule { code: "RQ014", summary: "TBR is not allowed by project policy" },
    Rule { code: "RQ015", summary: "unknown parent reference" },
    Rule { code: "RQ016", summary: "unknown depends_on reference" },
    Rule { code: "RQ017", summary: "requirement is part of a traceability cycle" },
    Rule { code: "RQ018", summary: "orphan requirement has no path to a root category" },
    Rule { code: "RQ019", summary: "repository is missing a required directory" },
    Rule { code: "RQ020", summary: "repository is missing a required file" },
    Rule { code: "RQ021", summary: "content hash is stale; run `rqtk rehash`" },
    Rule { code: "RQ022", summary: "`trace.satisfies` references an unknown need" },
    Rule { code: "RQ023", summary: "reference to an unknown stakeholder" },
    Rule { code: "RQ024", summary: "invalid verification level" },
    Rule { code: "RQ025", summary: "invalid verification phase" },
    Rule { code: "RQ026", summary: "unknown derived_from / refines / conflicts_with / related reference" },
    Rule { code: "RQ027", summary: "verification activity ID is defined more than once" },
    Rule { code: "RQ100", summary: "file could not be read or parsed (syntax error, unknown or missing field)" },
    Rule { code: "RQ101", summary: "ID is defined in more than one file" },
    Rule { code: "RQ102", summary: "file name does not match the ID it contains" },
];

pub fn rule(code: &str) -> Option<&'static Rule> {
    RULES.iter().find(|r| r.code == code)
}
