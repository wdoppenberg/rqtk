use chrono::NaiveDate;
use schemars::JsonSchema;
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};

#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct RequirementId(pub String);

impl Display for RequirementId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProjectConfig {
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
    pub change_control: ChangeControl,
    pub validation: ValidationRules,
    pub export: ExportConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RqtkConfig {
    #[serde(default)]
    pub repository: RepositoryLayout,
    #[serde(flatten)]
    pub project_config: ProjectConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RepositoryLayout {
    #[serde(default = "default_requirements_dir")]
    pub requirements_dir: String,
    #[serde(default)]
    pub required_files: Vec<String>,
    #[serde(default)]
    pub required_dirs: Vec<String>,
}

impl Default for RepositoryLayout {
    fn default() -> Self {
        Self {
            requirements_dir: default_requirements_dir(),
            required_files: Vec::new(),
            required_dirs: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProjectMeta {
    pub name: String,
    pub short_name: Option<String>,
    pub description: Option<String>,
    #[schemars(with = "String")]
    pub version: Version,
    pub mission_phase: Option<String>,
    pub classification: Option<String>,
    pub risk_posture: Option<String>,
    pub created: Option<NaiveDate>,
    pub updated: Option<NaiveDate>,
    pub organization: Option<ProjectOrganization>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProjectOrganization {
    pub program: Option<String>,
    pub center: Option<String>,
    pub responsible_engineer: Option<String>,
    pub cognizant_authority: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
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
pub struct Category {
    pub name: String,
    pub level: u8,
    pub description: Option<String>,
    /// Marks this category as a traceability root (replaces the hard-coded "STAKE" sentinel).
    /// At least one category should set this to `true` for orphan detection to work.
    #[serde(default)]
    pub is_root: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TypePolicy {
    pub allowed: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct VerificationPolicy {
    pub methods: Vec<String>,
    pub levels: Vec<String>,
    pub phases: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LifecyclePolicy {
    pub states: Vec<String>,
    pub default_state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PriorityPolicy {
    pub levels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CriticalityPolicy {
    pub levels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Standard {
    pub id: String,
    pub title: String,
    pub revision: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ChangeControl {
    pub ccb_required_after: String,
    pub require_signoff: bool,
    #[serde(default)]
    pub approvers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExportConfig {
    #[serde(default)]
    pub formats: Vec<String>,
    pub default_output_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RequirementFile {
    pub requirement: RequirementBody,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RequirementBody {
    pub id: RequirementId,
    pub title: String,
    pub category: String,
    #[serde(rename = "type")]
    pub req_type: String,
    /// SHA-256 of the semantic fields (statement, traceability, verification method, parameters).
    /// Maintained by `rqtk rehash` and the pre-commit hook; checked by lint rule RQ021.
    #[serde(default)]
    pub content_hash: Option<String>,
    pub statement: Statement,
    pub status: Status,
    #[serde(default)]
    pub approval: Option<Approval>,
    #[serde(default)]
    pub parameters: Vec<Parameter>,
    pub traceability: Traceability,
    pub verification: Verification,
    #[serde(default)]
    pub validation: Option<ValidationSpec>,
    #[serde(default)]
    pub risk: Option<Risk>,
    #[serde(default)]
    pub allocation: Option<Allocation>,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    #[schemars(with = "std::collections::BTreeMap<String, serde_json::Value>")]
    pub custom: BTreeMap<String, toml::Value>,
}

impl RequirementBody {
    /// Compute the SHA-256 fingerprint of the semantically load-bearing fields.
    /// Excludes `content_hash`, `created`, `title`, `tags`, `allocation`, `risk`,
    /// `approval`, and `custom` — these are administrative and do not affect what
    /// the requirement demands or how it is verified.
    pub fn compute_content_hash(&self) -> String {
        let mut h = Sha256::new();
        h.update(self.id.0.as_bytes());
        h.update(self.statement.text.as_bytes());
        for p in &self.traceability.parents {
            h.update(p.0.as_bytes());
        }
        for d in &self.traceability.depends_on {
            h.update(d.0.as_bytes());
        }
        for d in &self.traceability.derived_from {
            h.update(d.0.as_bytes());
        }
        for r in &self.traceability.refines {
            h.update(r.0.as_bytes());
        }
        h.update(self.verification.method.as_bytes());
        h.update(self.verification.level.as_bytes());
        h.update(self.verification.phase.as_bytes());
        for param in &self.parameters {
            h.update(param.name.as_bytes());
            h.update(param.operator.as_bytes());
            h.update(param.value.to_string().as_bytes());
            if let Some(unit) = &param.unit {
                h.update(unit.as_bytes());
            }
        }
        format!("{:x}", h.finalize())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Statement {
    pub text: String,
    pub rationale: Option<String>,
    #[serde(default)]
    pub assumptions: Vec<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Status {
    pub state: String,
    pub priority: String,
    pub criticality: Option<String>,
    pub maturity: Option<String>,
    #[serde(default)]
    pub tbd: bool,
    #[serde(default)]
    pub tbr: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Approval {
    pub baselined_at: Option<NaiveDate>,
    pub baselined_by: Option<String>,
    #[serde(default)]
    pub approved_by: Vec<String>,
    #[serde(default)]
    pub ecr_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Parameter {
    pub name: String,
    pub operator: String,
    #[schemars(with = "serde_json::Value")]
    pub value: toml::Value,
    pub unit: Option<String>,
    pub tolerance: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct Traceability {
    #[serde(default)]
    pub parents: Vec<RequirementId>,
    #[serde(default)]
    pub derived_from: Vec<RequirementId>,
    #[serde(default)]
    pub satisfies: Vec<String>,
    #[serde(default)]
    pub refines: Vec<RequirementId>,
    #[serde(default)]
    pub conflicts_with: Vec<RequirementId>,
    #[serde(default)]
    pub depends_on: Vec<RequirementId>,
    #[serde(default)]
    pub related: Vec<RequirementId>,
    #[serde(default)]
    pub external: Vec<ExternalTrace>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExternalTrace {
    #[serde(rename = "type")]
    pub trace_type: String,
    #[serde(rename = "ref")]
    pub reference: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Verification {
    pub method: String,
    pub level: String,
    pub phase: String,
    pub owner: Option<String>,
    pub success_criteria: Option<String>,
    #[serde(default)]
    pub activities: Vec<VerificationActivity>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct VerificationActivity {
    pub id: String,
    pub name: String,
    pub procedure: Option<String>,
    pub expected_result: Option<String>,
    pub status: Option<String>,
    pub executed_at: Option<NaiveDate>,
    #[serde(default)]
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ValidationSpec {
    pub method: Option<String>,
    pub stakeholder: Option<String>,
    pub acceptance_criteria: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Risk {
    #[serde(default)]
    pub hazards: Vec<String>,
    #[serde(default)]
    pub mitigations: Vec<String>,
    pub fmea_ref: Option<String>,
    #[serde(default)]
    pub safety_critical: bool,
    #[serde(default)]
    pub security_sensitive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Allocation {
    #[serde(default)]
    pub subsystems: Vec<String>,
    #[serde(default)]
    pub components: Vec<String>,
    #[serde(default)]
    pub software_modules: Vec<String>,
    #[serde(default)]
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

fn default_separator() -> String {
    "-".to_owned()
}

fn default_prefix() -> String {
    "REQ".to_owned()
}

fn default_padding() -> usize {
    4
}

fn default_requirements_dir() -> String {
    "requirements".to_owned()
}
