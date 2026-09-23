use chrono::NaiveDate;
use schemars::JsonSchema;
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};

/// The on-disk file format version this build reads and writes.
pub const SCHEMA_VERSION: u32 = 1;

/// Prefix of every content hash; bumped whenever the hashed field set or encoding changes.
pub const CONTENT_HASH_PREFIX: &str = "v1:";

macro_rules! id_newtype {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(
            Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
        )]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl Display for $name {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }
    };
}

id_newtype!(
    /// Identifier of a requirement, e.g. `FOBC-SYS-0001`.
    RequirementId
);
id_newtype!(
    /// Identifier of a stakeholder need, e.g. `NEED-0001`.
    NeedId
);
id_newtype!(
    /// Identifier of a stakeholder, e.g. `STK-001`.
    StakeholderId
);

/// A reference to any tracked item.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(tag = "kind", content = "id", rename_all = "lowercase")]
pub enum EntityRef {
    Requirement(RequirementId),
    Need(NeedId),
    Stakeholder(StakeholderId),
}

impl EntityRef {
    pub fn id(&self) -> &str {
        match self {
            EntityRef::Requirement(id) => &id.0,
            EntityRef::Need(id) => &id.0,
            EntityRef::Stakeholder(id) => &id.0,
        }
    }
}

impl Display for EntityRef {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.id())
    }
}

// ── Project configuration (`.rqtk/config.toml`) ──────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// File format version; must equal the version supported by this build of rqtk.
    pub schema_version: u32,
    #[serde(default)]
    pub repository: RepositoryLayout,
    pub project: ProjectMeta,
    pub identification: IdentificationScheme,
    pub categories: BTreeMap<String, Category>,
    #[serde(rename = "types")]
    pub req_types: TypePolicy,
    pub verification: VerificationPolicy,
    pub lifecycle: LifecyclePolicy,
    pub priority: PriorityPolicy,
    pub criticality: CriticalityPolicy,
    #[serde(default)]
    pub standards: Vec<Standard>,
    pub validation: ValidationRules,
    #[serde(default)]
    pub scan: ScanConfig,
}

/// Where `rqtk scan` looks for `verifies` annotations in source code.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScanConfig {
    /// Directories to scan, relative to the repository root. `.gitignore` is honoured.
    #[serde(default = "default_scan_paths")]
    pub paths: Vec<String>,
    /// Glob patterns (gitignore syntax) to skip, e.g. `"tests/fixtures/**"`.
    #[serde(default)]
    pub exclude: Vec<String>,
    /// File extensions to read.
    #[serde(default = "default_scan_extensions")]
    pub extensions: Vec<String>,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            paths: default_scan_paths(),
            exclude: Vec::new(),
            extensions: default_scan_extensions(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RepositoryLayout {
    #[serde(default = "default_requirements_dir")]
    pub requirements_dir: String,
    #[serde(default = "default_stakeholders_dir")]
    pub stakeholders_dir: String,
    #[serde(default = "default_needs_dir")]
    pub needs_dir: String,
    #[serde(default)]
    pub required_files: Vec<String>,
    #[serde(default)]
    pub required_dirs: Vec<String>,
}

impl Default for RepositoryLayout {
    fn default() -> Self {
        Self {
            requirements_dir: default_requirements_dir(),
            stakeholders_dir: default_stakeholders_dir(),
            needs_dir: default_needs_dir(),
            required_files: Vec::new(),
            required_dirs: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectMeta {
    pub name: String,
    pub short_name: Option<String>,
    pub description: Option<String>,
    #[schemars(with = "String")]
    pub version: Version,
    pub mission_phase: Option<String>,
    pub classification: Option<String>,
    pub risk_posture: Option<String>,
    #[serde(default, with = "opt_date", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub created: Option<NaiveDate>,
    #[serde(default, with = "opt_date", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub updated: Option<NaiveDate>,
    pub organization: Option<ProjectOrganization>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectOrganization {
    pub program: Option<String>,
    pub center: Option<String>,
    pub responsible_engineer: Option<String>,
    pub cognizant_authority: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IdentificationScheme {
    pub id_pattern: String,
    #[serde(default = "default_separator")]
    pub id_separator: String,
    #[serde(default = "default_prefix")]
    pub prefix: String,
    #[serde(default = "default_padding")]
    pub zero_padding: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Category {
    pub name: String,
    pub level: u8,
    pub description: Option<String>,
    /// Marks this category as a traceability root. At least one category should set this
    /// to `true` for orphan detection to work.
    #[serde(default)]
    pub is_root: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TypePolicy {
    pub allowed: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VerificationPolicy {
    pub methods: Vec<String>,
    pub levels: Vec<String>,
    pub phases: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LifecyclePolicy {
    pub states: Vec<String>,
    pub default_state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PriorityPolicy {
    pub levels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CriticalityPolicy {
    pub levels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Standard {
    pub id: String,
    pub title: String,
    pub revision: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ValidationRules {
    pub require_rationale: bool,
    pub require_verification_method: bool,
    #[serde(default)]
    pub require_parent_for_levels: Vec<String>,
    pub forbid_orphans: bool,
    pub forbid_circular_traces: bool,
    pub allow_tbd: bool,
    pub allow_tbr: bool,
    #[serde(default)]
    pub shall_keywords: Vec<String>,
    #[serde(default)]
    pub forbidden_keywords: Vec<String>,
}

// ── Requirement ──────────────────────────────────────────────────────────────

/// One requirement file. Scalars sit at the top level; related groups are tables.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Requirement {
    pub id: RequirementId,
    pub title: String,
    pub category: String,
    #[serde(rename = "type")]
    pub req_type: String,
    pub state: String,
    pub priority: String,
    pub criticality: Option<String>,
    pub maturity: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub tbd: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub tbr: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
    /// The normative statement, e.g. "The system shall …".
    pub statement: String,
    pub rationale: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assumptions: Vec<String>,
    pub notes: Option<String>,
    /// Fingerprint of the semantic fields, see [`Requirement::compute_content_hash`].
    /// Maintained by `rqtk rehash`; checked by lint rule RQ021.
    pub content_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Traceability::is_empty")]
    pub trace: Traceability,
    pub verification: Verification,
    pub approval: Option<Approval>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<Parameter>,
    pub validation: Option<ValidationSpec>,
    pub risk: Option<Risk>,
    pub allocation: Option<Allocation>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    #[schemars(with = "std::collections::BTreeMap<String, serde_json::Value>")]
    pub custom: BTreeMap<String, toml::Value>,
}

impl Requirement {
    /// Fingerprint of the fields that change what the requirement demands or how it is
    /// verified: id, statement, parameters, structural links and verification method/level/phase.
    /// Administrative fields (title, status, approval, allocation, risk, custom, …) are excluded.
    pub fn compute_content_hash(&self) -> String {
        let mut h = ContentHasher::default();
        h.field("id", &self.id.0);
        h.field("statement", &self.statement);
        for p in &self.trace.parents {
            h.field("parent", &p.0);
        }
        for d in &self.trace.depends_on {
            h.field("depends_on", &d.0);
        }
        for d in &self.trace.derived_from {
            h.field("derived_from", &d.0);
        }
        for r in &self.trace.refines {
            h.field("refines", &r.0);
        }
        for n in &self.trace.satisfies {
            h.field("satisfies", &n.0);
        }
        h.field("verification.method", &self.verification.method);
        h.field("verification.level", &self.verification.level);
        h.field("verification.phase", &self.verification.phase);
        for param in &self.parameters {
            h.field("parameter.name", &param.name);
            h.field("parameter.operator", &param.operator);
            h.field("parameter.value", &param.value.to_string());
            h.field("parameter.unit", param.unit.as_deref().unwrap_or(""));
            h.field(
                "parameter.tolerance",
                &param.tolerance.map(|t| t.to_string()).unwrap_or_default(),
            );
        }
        h.finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Approval {
    #[serde(default, with = "opt_date", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub baselined_at: Option<NaiveDate>,
    pub baselined_by: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub approved_by: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ecr_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Parameter {
    pub name: String,
    pub operator: String,
    #[schemars(with = "serde_json::Value")]
    pub value: toml::Value,
    pub unit: Option<String>,
    pub tolerance: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
pub struct Traceability {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parents: Vec<RequirementId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub derived_from: Vec<RequirementId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub satisfies: Vec<NeedId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refines: Vec<RequirementId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conflicts_with: Vec<RequirementId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<RequirementId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related: Vec<RequirementId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub external: Vec<ExternalTrace>,
}

impl Traceability {
    pub fn is_empty(&self) -> bool {
        self.parents.is_empty()
            && self.derived_from.is_empty()
            && self.satisfies.is_empty()
            && self.refines.is_empty()
            && self.conflicts_with.is_empty()
            && self.depends_on.is_empty()
            && self.related.is_empty()
            && self.external.is_empty()
    }

    /// Every requirement-to-requirement link, labelled with its field name.
    pub fn requirement_links(&self) -> impl Iterator<Item = (&'static str, &RequirementId)> {
        [
            ("parents", &self.parents),
            ("derived_from", &self.derived_from),
            ("refines", &self.refines),
            ("conflicts_with", &self.conflicts_with),
            ("depends_on", &self.depends_on),
            ("related", &self.related),
        ]
        .into_iter()
        .flat_map(|(name, ids)| ids.iter().map(move |id| (name, id)))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExternalTrace {
    #[serde(rename = "type")]
    pub trace_type: String,
    #[serde(rename = "ref")]
    pub reference: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Verification {
    pub method: String,
    pub level: String,
    pub phase: String,
    pub owner: Option<String>,
    pub success_criteria: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub activities: Vec<VerificationActivity>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VerificationActivity {
    pub id: String,
    pub name: String,
    pub procedure: Option<String>,
    pub expected_result: Option<String>,
    pub status: Option<String>,
    #[serde(default, with = "opt_date", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub executed_at: Option<NaiveDate>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ValidationSpec {
    pub method: Option<String>,
    pub stakeholder: Option<StakeholderId>,
    pub acceptance_criteria: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Risk {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hazards: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mitigations: Vec<String>,
    pub fmea_ref: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub safety_critical: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub security_sensitive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Allocation {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subsystems: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub software_modules: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_files: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ScaffoldInput<'a> {
    pub category: &'a str,
    pub req_type: &'a str,
    pub title: &'a str,
    pub statement: &'a str,
    pub rationale: Option<&'a str>,
}

// ── Stakeholder ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Stakeholder {
    pub id: StakeholderId,
    pub name: String,
    pub role: Option<String>,
    pub organization: Option<String>,
    #[serde(default, skip_serializing_if = "StakeholderConcerns::is_empty")]
    pub concerns: StakeholderConcerns,
    #[serde(default, skip_serializing_if = "StakeholderAuthority::is_empty")]
    pub authority: StakeholderAuthority,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StakeholderConcerns {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub primary: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub secondary: Vec<String>,
}

impl StakeholderConcerns {
    pub fn is_empty(&self) -> bool {
        self.primary.is_empty() && self.secondary.is_empty()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StakeholderAuthority {
    pub approval_scope: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub sign_off_required: bool,
}

impl StakeholderAuthority {
    pub fn is_empty(&self) -> bool {
        self.approval_scope.is_none() && !self.sign_off_required
    }
}

// ── Stakeholder need ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Need {
    pub id: NeedId,
    pub title: String,
    pub state: String,
    pub priority: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stakeholders: Vec<StakeholderId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
    pub statement: String,
    pub rationale: Option<String>,
    pub content_hash: Option<String>,
    pub acceptance: Option<Acceptance>,
}

impl Need {
    pub fn compute_content_hash(&self) -> String {
        let mut h = ContentHasher::default();
        h.field("id", &self.id.0);
        h.field("statement", &self.statement);
        for stk in &self.stakeholders {
            h.field("stakeholder", &stk.0);
        }
        h.finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Acceptance {
    pub criteria: Option<String>,
    pub validated_by: Option<String>,
    #[serde(default, with = "opt_date", skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<String>")]
    pub validated_at: Option<NaiveDate>,
}

// ── helpers ──────────────────────────────────────────────────────────────────

/// Dates are written as `YYYY-MM-DD` strings and read from either a string or a native
/// TOML local date (`executed_at = 2024-11-15`).
mod opt_date {
    use chrono::NaiveDate;
    use serde::{Deserialize, Deserializer, Serializer, de::Error};

    pub fn serialize<S: Serializer>(date: &Option<NaiveDate>, s: S) -> Result<S::Ok, S::Error> {
        match date {
            Some(d) => s.serialize_str(&d.format("%Y-%m-%d").to_string()),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<NaiveDate>, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Toml(toml::value::Datetime),
            Text(String),
        }
        let Some(raw) = Option::<Raw>::deserialize(d)? else {
            return Ok(None);
        };
        let text = match raw {
            Raw::Toml(dt) if dt.time.is_none() && dt.offset.is_none() => dt.to_string(),
            Raw::Toml(dt) => {
                return Err(D::Error::custom(format!("expected a date, found `{dt}`")));
            }
            Raw::Text(text) => text,
        };
        NaiveDate::parse_from_str(&text, "%Y-%m-%d")
            .map(Some)
            .map_err(|e| D::Error::custom(format!("invalid date `{text}`: {e}")))
    }
}

/// SHA-256 over tagged, length-prefixed fields, so that no two distinct field sets
/// (e.g. `parents = ["AB"]` vs `parents = ["A", "B"]`) share an encoding.
#[derive(Default)]
struct ContentHasher(Sha256);

impl ContentHasher {
    fn field(&mut self, tag: &str, value: &str) {
        self.0.update(tag.as_bytes());
        self.0.update([0u8]);
        self.0.update((value.len() as u64).to_le_bytes());
        self.0.update(value.as_bytes());
    }

    fn finish(self) -> String {
        format!("{CONTENT_HASH_PREFIX}{:x}", self.0.finalize())
    }
}

fn is_false(b: &bool) -> bool {
    !*b
}

fn default_separator() -> String {
    "-".to_owned()
}

fn default_prefix() -> String {
    "REQ".to_owned()
}

fn default_padding() -> usize {
    4
}

fn default_scan_paths() -> Vec<String> {
    vec![".".to_owned()]
}

fn default_scan_extensions() -> Vec<String> {
    [
        "rs", "py", "go", "ts", "tsx", "js", "jsx", "java", "kt", "c", "cc", "cpp", "h", "hpp",
        "cs", "swift", "rb",
    ]
    .map(str::to_owned)
    .to_vec()
}

fn default_requirements_dir() -> String {
    ".rqtk/requirements".to_owned()
}

fn default_stakeholders_dir() -> String {
    ".rqtk/stakeholders".to_owned()
}

fn default_needs_dir() -> String {
    ".rqtk/needs".to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn requirement(extra: &str) -> Requirement {
        toml::from_str(&format!(
            r#"id = "R-1"
title = "t"
category = "SYS"
type = "Functional"
state = "Draft"
priority = "High"
statement = "The system shall work."
{extra}
[verification]
method = "Test"
level = "System"
phase = "Development"
"#
        ))
        .unwrap()
    }

    #[test]
    fn content_hash_is_versioned() {
        assert!(requirement("").compute_content_hash().starts_with("v1:"));
    }

    #[test]
    fn content_hash_frames_list_items() {
        let joined = requirement("[trace]\nparents = [\"AB\"]\n");
        let split = requirement("[trace]\nparents = [\"A\", \"B\"]\n");
        assert_ne!(joined.compute_content_hash(), split.compute_content_hash());
    }

    #[test]
    fn content_hash_distinguishes_link_kinds() {
        let parent = requirement("[trace]\nparents = [\"X\"]\n");
        let refines = requirement("[trace]\nrefines = [\"X\"]\n");
        assert_ne!(
            parent.compute_content_hash(),
            refines.compute_content_hash()
        );
    }

    #[test]
    fn content_hash_ignores_administrative_fields() {
        let a = requirement("");
        let b = requirement("notes = \"reworded\"\nkeywords = [\"x\"]\n");
        assert_eq!(a.compute_content_hash(), b.compute_content_hash());
    }

    #[test]
    fn written_requirement_round_trips() {
        let req = requirement("[trace]\nparents = [\"X\"]\n");
        let text = toml::to_string_pretty(&req).unwrap();
        let back: Requirement = toml::from_str(&text).unwrap();
        assert_eq!(back.trace.parents, req.trace.parents);
        assert!(!text.contains("[requirement"), "{text}");
    }
}
