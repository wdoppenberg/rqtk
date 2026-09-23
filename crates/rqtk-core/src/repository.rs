use crate::diagnostic::{Diagnostic, line_col, resolve_lines};
use crate::error::RqtkError;
use crate::git::GitContext;
use crate::io::edit_toml_file;
use crate::model::{
    Config, EntityRef, Need, NeedId, Requirement, RequirementId, SCHEMA_VERSION, ScaffoldInput,
    Stakeholder, StakeholderAuthority, StakeholderConcerns, StakeholderId, Traceability,
    Verification,
};
use crate::validation::{has_path_to_root, is_single_shall_sentence_violation};

use petgraph::algo::tarjan_scc;
use petgraph::graph::{DiGraph, NodeIndex};
use regex::Regex;
use serde::de::DeserializeOwned;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

pub struct Loaded;
pub struct Validated;

/// An item whose stored content hash differs from its computed one.
#[derive(Debug, Clone, serde::Serialize)]
pub struct StaleHash {
    pub subject: EntityRef,
    pub path: PathBuf,
    pub hash: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TraceView {
    pub upward: Vec<RequirementId>,
    pub downward: Vec<RequirementId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SatisfactionStatus {
    /// At least one requirement's `trace.satisfies` list names this need.
    Satisfied,
    /// No requirement points to this need yet.
    Unsatisfied,
}

/// gix::Repository is not Clone, so RequirementSet is not Clone either.
#[derive(Debug)]
pub struct RequirementSet<S = Loaded> {
    pub config: Config,
    pub requirements: BTreeMap<RequirementId, Requirement>,
    pub needs: BTreeMap<NeedId, Need>,
    pub stakeholders: BTreeMap<StakeholderId, Stakeholder>,
    pub files_by_id: BTreeMap<RequirementId, PathBuf>,
    pub needs_by_id: BTreeMap<NeedId, PathBuf>,
    pub stakeholders_by_id: BTreeMap<StakeholderId, PathBuf>,
    pub root: PathBuf,
    pub needs_root: PathBuf,
    pub stakeholders_root: PathBuf,
    pub repo_root: PathBuf,
    pub config_path: PathBuf,
    pub git: GitContext,
    /// Findings from loading: files that failed to parse, duplicate IDs, misnamed files.
    /// Those files are left out of the set; `validate` reports these alongside lint findings.
    pub load_diagnostics: Vec<Diagnostic>,
    /// IDs found in files that failed to load. References to them are not reported as
    /// unknown, since the root cause is already reported against the broken file.
    unloaded_ids: HashSet<String>,
    id_regex: Regex,
    _state: PhantomData<S>,
}

/// Read and parse `.rqtk/config.toml`, checking the schema version.
pub fn load_config(config_path: &Path) -> Result<Config, RqtkError> {
    let text = read_file(config_path)?;
    // Check the version before the full parse so an old config gets a clear message
    // instead of a list of unknown fields.
    #[derive(serde::Deserialize)]
    struct VersionProbe {
        schema_version: Option<u32>,
    }
    if let Ok(VersionProbe { schema_version }) = toml::from_str::<VersionProbe>(&text)
        && schema_version != Some(SCHEMA_VERSION)
    {
        return Err(RqtkError::UnsupportedSchemaVersion {
            path: config_path.to_path_buf(),
            found: schema_version.unwrap_or(0),
            supported: SCHEMA_VERSION,
        });
    }
    toml::from_str(&text).map_err(|source| RqtkError::TomlParse {
        path: config_path.to_path_buf(),
        source,
    })
}

impl RequirementSet<Loaded> {
    pub fn load_from_repo_root(repo_root: impl AsRef<Path>) -> Result<Self, RqtkError> {
        let repo_root = repo_root.as_ref().to_path_buf();
        let git = GitContext::open(&repo_root)?;
        let config_path = repo_root.join(".rqtk/config.toml");
        let config = load_config(&config_path)?;

        let id_regex = Regex::new(&config.identification.id_pattern).map_err(|source| {
            RqtkError::InvalidIdPattern {
                pattern: config.identification.id_pattern.clone(),
                source,
            }
        })?;

        let root = repo_root.join(&config.repository.requirements_dir);
        let needs_root = repo_root.join(&config.repository.needs_dir);
        let stakeholders_root = repo_root.join(&config.repository.stakeholders_dir);

        let mut diagnostics = Vec::new();
        let mut seen_ids: HashMap<String, PathBuf> = HashMap::new();
        let mut unloaded_ids = HashSet::new();

        let (requirements, files_by_id) = load_items(
            &root,
            true,
            |r: &Requirement| r.id.clone(),
            EntityRef::Requirement,
            &mut seen_ids,
            &mut unloaded_ids,
            &mut diagnostics,
        )?;
        let (needs, needs_by_id) = load_items(
            &needs_root,
            false,
            |n: &Need| n.id.clone(),
            EntityRef::Need,
            &mut seen_ids,
            &mut unloaded_ids,
            &mut diagnostics,
        )?;
        let (stakeholders, stakeholders_by_id) = load_items(
            &stakeholders_root,
            false,
            |s: &Stakeholder| s.id.clone(),
            EntityRef::Stakeholder,
            &mut seen_ids,
            &mut unloaded_ids,
            &mut diagnostics,
        )?;

        Ok(Self {
            config,
            requirements,
            needs,
            stakeholders,
            files_by_id,
            needs_by_id,
            stakeholders_by_id,
            root,
            needs_root,
            stakeholders_root,
            repo_root,
            config_path,
            git,
            load_diagnostics: diagnostics,
            unloaded_ids,
            id_regex,
            _state: PhantomData,
        })
    }

    /// Write `approval.baselined_at` and `approval.baselined_by` into every requirement file,
    /// preserving the rest of each file. Returns the paths of files that changed.
    pub fn stamp_baseline(
        &mut self,
        by: &str,
        date: chrono::NaiveDate,
    ) -> Result<Vec<PathBuf>, RqtkError> {
        let toml_date: toml_edit::Datetime = date
            .format("%Y-%m-%d")
            .to_string()
            .parse()
            .expect("a formatted NaiveDate is a valid TOML local date");
        let mut changed = Vec::new();
        for (id, req) in &mut self.requirements {
            let approval = req.approval.get_or_insert_with(|| crate::model::Approval {
                baselined_at: None,
                baselined_by: None,
                approved_by: Vec::new(),
                ecr_ids: Vec::new(),
            });
            approval.baselined_at = Some(date);
            approval.baselined_by = Some(by.to_owned());

            let path = &self.files_by_id[id];
            let wrote = edit_toml_file(path, |doc| {
                let table = doc
                    .entry("approval")
                    .or_insert_with(toml_edit::table)
                    .as_table_like_mut()
                    .expect("`approval` is a table in a successfully parsed requirement");
                table.insert("baselined_at", toml_edit::value(toml_date));
                table.insert("baselined_by", toml_edit::value(by));
            })?;
            if wrote {
                changed.push(path.clone());
            }
        }
        Ok(changed)
    }

    pub fn validate(mut self) -> (RequirementSet<Validated>, Vec<Diagnostic>) {
        let mut issues = std::mem::take(&mut self.load_diagnostics);
        issues.extend(self.lint_requirements());
        issues.extend(self.lint_needs());
        if self.config.validation.forbid_orphans {
            issues.extend(self.detect_orphans());
        }
        if self.config.validation.forbid_circular_traces {
            issues.extend(self.detect_cycles());
        }
        issues.extend(self.check_repository_structure());
        resolve_lines(&mut issues);

        let validated = RequirementSet {
            config: self.config,
            requirements: self.requirements,
            needs: self.needs,
            stakeholders: self.stakeholders,
            files_by_id: self.files_by_id,
            needs_by_id: self.needs_by_id,
            stakeholders_by_id: self.stakeholders_by_id,
            root: self.root,
            needs_root: self.needs_root,
            stakeholders_root: self.stakeholders_root,
            repo_root: self.repo_root,
            config_path: self.config_path,
            git: self.git,
            load_diagnostics: Vec::new(),
            unloaded_ids: self.unloaded_ids,
            id_regex: self.id_regex,
            _state: PhantomData,
        };
        (validated, issues)
    }

    fn lint_requirements(&self) -> Vec<Diagnostic> {
        let cfg = &self.config;
        let states: HashSet<&str> = cfg.lifecycle.states.iter().map(String::as_str).collect();
        let priorities: HashSet<&str> = cfg.priority.levels.iter().map(String::as_str).collect();
        let criticalities: HashSet<&str> =
            cfg.criticality.levels.iter().map(String::as_str).collect();
        let types: HashSet<&str> = cfg.req_types.allowed.iter().map(String::as_str).collect();
        let methods: HashSet<&str> = cfg
            .verification
            .methods
            .iter()
            .map(String::as_str)
            .collect();
        let levels: HashSet<&str> = cfg.verification.levels.iter().map(String::as_str).collect();
        let phases: HashSet<&str> = cfg.verification.phases.iter().map(String::as_str).collect();
        let parent_required: HashSet<&str> = cfg
            .validation
            .require_parent_for_levels
            .iter()
            .map(String::as_str)
            .collect();
        let forbidden: Vec<(&str, Regex)> = cfg
            .validation
            .forbidden_keywords
            .iter()
            .filter_map(|kw| {
                Regex::new(&format!(r"(?i)\b{}\b", regex::escape(kw)))
                    .ok()
                    .map(|re| (kw.as_str(), re))
            })
            .collect();

        let mut issues = Vec::new();
        let mut activity_owner: HashMap<&str, &RequirementId> = HashMap::new();

        for (req_id, req) in &self.requirements {
            let path = &self.files_by_id[req_id];
            let diag = |d: Diagnostic| d.subject(EntityRef::Requirement(req_id.clone())).file(path);
            let finding = |code, field, msg: String| diag(Diagnostic::new(code, msg).field(field));

            if !self.id_regex.is_match(&req_id.0) {
                issues.push(finding(
                    "RQ001",
                    "id",
                    format!("ID `{req_id}` does not match configured id_pattern"),
                ));
            }
            if !cfg.categories.contains_key(&req.category) {
                issues.push(finding(
                    "RQ002",
                    "category",
                    format!("unknown category `{}`", req.category),
                ));
            }
            if !types.contains(req.req_type.as_str()) {
                issues.push(finding(
                    "RQ003",
                    "type",
                    format!("unknown requirement type `{}`", req.req_type),
                ));
            }
            if !states.contains(req.state.as_str()) {
                issues.push(finding(
                    "RQ004",
                    "state",
                    format!("invalid lifecycle state `{}`", req.state),
                ));
            }
            if !priorities.contains(req.priority.as_str()) {
                issues.push(finding(
                    "RQ005",
                    "priority",
                    format!("invalid priority `{}`", req.priority),
                ));
            }
            if let Some(criticality) = req.criticality.as_deref()
                && !criticalities.contains(criticality)
            {
                issues.push(finding(
                    "RQ006",
                    "criticality",
                    format!("invalid criticality `{criticality}`"),
                ));
            }
            if cfg.validation.require_rationale && is_blank(req.rationale.as_deref()) {
                issues.push(finding(
                    "RQ007",
                    "rationale",
                    "rationale is required but missing".to_owned(),
                ));
            }
            let method = req.verification.method.trim();
            if method.is_empty() {
                if cfg.validation.require_verification_method {
                    issues.push(finding(
                        "RQ008",
                        "verification.method",
                        "verification.method is required but missing".to_owned(),
                    ));
                }
            } else if !methods.contains(method) {
                issues.push(finding(
                    "RQ009",
                    "verification.method",
                    format!("invalid verification method `{method}`"),
                ));
            }
            if !levels.contains(req.verification.level.as_str()) {
                issues.push(finding(
                    "RQ024",
                    "verification.level",
                    format!("invalid verification level `{}`", req.verification.level),
                ));
            }
            if !phases.contains(req.verification.phase.as_str()) {
                issues.push(finding(
                    "RQ025",
                    "verification.phase",
                    format!("invalid verification phase `{}`", req.verification.phase),
                ));
            }
            if !cfg.validation.shall_keywords.is_empty()
                && is_single_shall_sentence_violation(
                    &req.statement,
                    &cfg.validation.shall_keywords,
                )
            {
                issues.push(finding(
                    "RQ010",
                    "statement",
                    "statement must contain exactly one normative sentence with a shall keyword"
                        .to_owned(),
                ));
            }
            for (keyword, re) in &forbidden {
                if re.is_match(&req.statement) {
                    issues.push(finding(
                        "RQ011",
                        "statement",
                        format!("statement uses forbidden keyword `{keyword}`"),
                    ));
                }
            }
            if parent_required.contains(req.category.as_str()) && req.trace.parents.is_empty() {
                issues.push(finding(
                    "RQ012",
                    "trace.parents",
                    format!("category `{}` requires at least one parent", req.category),
                ));
            }
            if req.tbd && !cfg.validation.allow_tbd {
                issues.push(finding(
                    "RQ013",
                    "tbd",
                    "TBD is not allowed by project policy".to_owned(),
                ));
            }
            if req.tbr && !cfg.validation.allow_tbr {
                issues.push(finding(
                    "RQ014",
                    "tbr",
                    "TBR is not allowed by project policy".to_owned(),
                ));
            }
            if let Some(stored) = &req.content_hash {
                let computed = req.compute_content_hash();
                if stored != &computed {
                    issues.push(finding(
                        "RQ021",
                        "content_hash",
                        stale_hash_message(stored, &computed),
                    ));
                }
            }
            for (link, target) in req.trace.requirement_links() {
                if self.requirements.contains_key(target) || self.unloaded_ids.contains(&target.0) {
                    continue;
                }
                let (code, field) = match link {
                    "parents" => ("RQ015", "trace.parents"),
                    "depends_on" => ("RQ016", "trace.depends_on"),
                    "derived_from" => ("RQ026", "trace.derived_from"),
                    "refines" => ("RQ026", "trace.refines"),
                    "conflicts_with" => ("RQ026", "trace.conflicts_with"),
                    _ => ("RQ026", "trace.related"),
                };
                issues.push(finding(
                    code,
                    field,
                    format!("unknown {link} reference `{target}`"),
                ));
            }
            for need_id in &req.trace.satisfies {
                if !self.needs.contains_key(need_id) && !self.unloaded_ids.contains(&need_id.0) {
                    issues.push(finding(
                        "RQ022",
                        "trace.satisfies",
                        format!("satisfies references unknown need `{need_id}`"),
                    ));
                }
            }
            if let Some(stk_id) = req.validation.as_ref().and_then(|v| v.stakeholder.as_ref())
                && !self.stakeholders.contains_key(stk_id)
                && !self.unloaded_ids.contains(&stk_id.0)
            {
                issues.push(finding(
                    "RQ023",
                    "validation.stakeholder",
                    format!("validation.stakeholder `{stk_id}` is not a known stakeholder"),
                ));
            }
            for activity in &req.verification.activities {
                if let Some(first) = activity_owner.insert(&activity.id, req_id) {
                    issues.push(finding(
                        "RQ027",
                        "verification.activities",
                        format!(
                            "verification activity `{}` is already defined by `{first}`",
                            activity.id
                        ),
                    ));
                }
            }
        }
        issues
    }

    fn lint_needs(&self) -> Vec<Diagnostic> {
        let mut issues = Vec::new();
        for (need_id, need) in &self.needs {
            let path = &self.needs_by_id[need_id];
            let diag = |d: Diagnostic| d.subject(EntityRef::Need(need_id.clone())).file(path);
            for stk_id in &need.stakeholders {
                if !self.stakeholders.contains_key(stk_id) && !self.unloaded_ids.contains(&stk_id.0)
                {
                    issues.push(diag(
                        Diagnostic::new(
                            "RQ023",
                            format!("need references unknown stakeholder `{stk_id}`"),
                        )
                        .field("stakeholders"),
                    ));
                }
            }
            if let Some(stored) = &need.content_hash {
                let computed = need.compute_content_hash();
                if stored != &computed {
                    issues.push(diag(
                        Diagnostic::new("RQ021", stale_hash_message(stored, &computed))
                            .field("content_hash"),
                    ));
                }
            }
        }
        issues
    }
}

fn stale_hash_message(stored: &str, computed: &str) -> String {
    let short = |h: &str| h.chars().take(11).collect::<String>();
    format!(
        "content hash is stale (stored {}, computed {}) — run `rqtk rehash`",
        short(stored),
        short(computed)
    )
}

impl RequirementSet<Validated> {
    pub fn satisfaction_closure(&self) -> BTreeMap<NeedId, SatisfactionStatus> {
        let mut result: BTreeMap<NeedId, SatisfactionStatus> = self
            .needs
            .keys()
            .map(|id| (id.clone(), SatisfactionStatus::Unsatisfied))
            .collect();
        for req in self.requirements.values() {
            for need_id in &req.trace.satisfies {
                if let Some(status) = result.get_mut(need_id) {
                    *status = SatisfactionStatus::Satisfied;
                }
            }
        }
        result
    }

    /// Graphviz DOT rendering of stakeholders, needs, requirements and the links between them.
    pub fn to_dot(&self) -> String {
        const PALETTE: &[&str] = &[
            "#AED6F1", "#A9DFBF", "#FAD7A0", "#F1948A", "#D7BDE2", "#A3E4D7", "#F9E79F", "#FADBD8",
        ];

        let cats: BTreeSet<&str> = self
            .requirements
            .values()
            .map(|r| r.category.as_str())
            .collect();
        let color_map: HashMap<&str, &str> = cats
            .iter()
            .enumerate()
            .map(|(i, &c)| (c, PALETTE[i % PALETTE.len()]))
            .collect();

        let mut out = String::from("digraph {\n");
        out.push_str("    node [style=filled fontname=\"Helvetica\"];\n");
        out.push_str("    edge [fontname=\"Helvetica\" fontsize=10];\n");

        for (id, stk) in &self.stakeholders {
            out.push_str(&format!(
                "    \"{}\" [ label=\"{}\" shape=house fillcolor=\"#E5E7EB\" tooltip=\"{}\" ];\n",
                dot_escape(&id.0),
                dot_escape(&id.0),
                dot_escape(&stk.name)
            ));
        }
        for (id, need) in &self.needs {
            out.push_str(&format!(
                "    \"{}\" [ label=\"{}\" shape=note fillcolor=\"#FEF3C7\" tooltip=\"{}\" ];\n",
                dot_escape(&id.0),
                dot_escape(&id.0),
                dot_escape(&need.title)
            ));
        }
        for (id, req) in &self.requirements {
            let color = color_map
                .get(req.category.as_str())
                .copied()
                .unwrap_or("#FFFFFF");
            let is_root = self
                .config
                .categories
                .get(&req.category)
                .is_some_and(|c| c.is_root);
            let shape = if is_root { "box" } else { "ellipse" };
            let tooltip = dot_escape(&format!(
                "{}\\n{}  |  {}  |  {}",
                req.title, req.category, req.state, req.priority
            ));
            out.push_str(&format!(
                "    \"{}\" [ label=\"{}\" shape={} fillcolor=\"{}\" tooltip=\"{}\" ];\n",
                dot_escape(&id.0),
                dot_escape(&id.0),
                shape,
                color,
                tooltip
            ));
        }

        let mut edge = |from: &str, to: &str, attrs: &str| {
            out.push_str(&format!(
                "    \"{}\" -> \"{}\"{};\n",
                dot_escape(from),
                dot_escape(to),
                attrs
            ));
        };
        for (id, need) in &self.needs {
            for stk in &need.stakeholders {
                edge(&id.0, &stk.0, " [ style=dotted label=\"raised by\" ]");
            }
        }
        for (id, req) in &self.requirements {
            let t = &req.trace;
            for n in &t.satisfies {
                edge(&id.0, &n.0, " [ color=darkgreen label=satisfies ]");
            }
            for p in &t.parents {
                edge(&id.0, &p.0, "");
            }
            for d in &t.depends_on {
                edge(&id.0, &d.0, " [ style=dashed color=blue label=depends ]");
            }
            for s in &t.derived_from {
                edge(&id.0, &s.0, " [ style=dotted color=gray label=derived ]");
            }
            for r in &t.refines {
                edge(&id.0, &r.0, " [ style=dashed color=purple label=refines ]");
            }
            for c in &t.conflicts_with {
                edge(&id.0, &c.0, " [ style=dashed color=red label=conflicts ]");
            }
        }

        out.push_str("}\n");
        out
    }
}

fn dot_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

impl<S> RequirementSet<S> {
    pub fn trace_view(&self, root_id: &RequirementId) -> Result<TraceView, RqtkError> {
        if !self.requirements.contains_key(root_id) {
            return Err(RqtkError::RequirementNotFound(root_id.clone()));
        }

        let mut upward = Vec::new();
        let mut stack = vec![root_id.clone()];
        let mut seen = HashSet::from([root_id.clone()]);
        while let Some(id) = stack.pop() {
            if let Some(req) = self.requirements.get(&id) {
                for p in &req.trace.parents {
                    if seen.insert(p.clone()) {
                        upward.push(p.clone());
                        stack.push(p.clone());
                    }
                }
            }
        }

        let mut children_map: HashMap<&RequirementId, Vec<&RequirementId>> = HashMap::new();
        for (id, req) in &self.requirements {
            for p in &req.trace.parents {
                children_map.entry(p).or_default().push(id);
            }
        }

        let mut downward = Vec::new();
        let mut down_stack = vec![root_id];
        let mut down_seen = HashSet::from([root_id]);
        while let Some(id) = down_stack.pop() {
            for &child in children_map.get(id).into_iter().flatten() {
                if down_seen.insert(child) {
                    downward.push(child.clone());
                    down_stack.push(child);
                }
            }
        }

        Ok(TraceView { upward, downward })
    }

    pub fn category_dir(&self, category: &str) -> PathBuf {
        self.root.join(category)
    }

    pub fn next_requirement_id(&self, category: &str) -> RequirementId {
        let ident = &self.config.identification;
        let prefix = format!(
            "{}{sep}{category}{sep}",
            ident.prefix,
            sep = ident.id_separator
        );
        let max_ord = self
            .requirements
            .keys()
            .filter_map(|id| id.0.strip_prefix(&prefix)?.parse::<usize>().ok())
            .max()
            .unwrap_or(0);
        RequirementId(format!(
            "{prefix}{:0width$}",
            max_ord + 1,
            width = ident.zero_padding
        ))
    }

    pub fn scaffold_requirement(&self, input: ScaffoldInput<'_>) -> Requirement {
        let first = |values: &[String], fallback: &str| {
            values
                .first()
                .cloned()
                .unwrap_or_else(|| fallback.to_owned())
        };
        let cfg = &self.config;
        let mut req = Requirement {
            id: self.next_requirement_id(input.category),
            title: input.title.to_owned(),
            category: input.category.to_owned(),
            req_type: input.req_type.to_owned(),
            state: cfg.lifecycle.default_state.clone(),
            priority: first(&cfg.priority.levels, "Medium"),
            criticality: None,
            maturity: None,
            tbd: false,
            tbr: false,
            keywords: Vec::new(),
            statement: input.statement.to_owned(),
            rationale: input.rationale.map(ToOwned::to_owned),
            assumptions: Vec::new(),
            notes: None,
            content_hash: None,
            trace: Traceability::default(),
            verification: Verification {
                method: first(&cfg.verification.methods, "Test"),
                level: first(&cfg.verification.levels, "System"),
                phase: first(&cfg.verification.phases, "Development"),
                owner: None,
                success_criteria: None,
                activities: Vec::new(),
            },
            approval: None,
            parameters: Vec::new(),
            validation: None,
            risk: None,
            allocation: None,
            custom: BTreeMap::new(),
        };
        req.content_hash = Some(req.compute_content_hash());
        req
    }

    pub fn next_stakeholder_id(&self) -> StakeholderId {
        let max = self
            .stakeholders
            .keys()
            .filter_map(|id| id.0.strip_prefix("STK-")?.parse::<usize>().ok())
            .max()
            .unwrap_or(0);
        StakeholderId(format!("STK-{:03}", max + 1))
    }

    pub fn scaffold_stakeholder(&self, id: StakeholderId, name: &str) -> Stakeholder {
        Stakeholder {
            id,
            name: name.to_owned(),
            role: None,
            organization: None,
            concerns: StakeholderConcerns::default(),
            authority: StakeholderAuthority::default(),
        }
    }

    pub fn next_need_id(&self) -> NeedId {
        let max = self
            .needs
            .keys()
            .filter_map(|id| id.0.strip_prefix("NEED-")?.parse::<usize>().ok())
            .max()
            .unwrap_or(0);
        NeedId(format!("NEED-{:04}", max + 1))
    }

    pub fn scaffold_need(&self, id: NeedId, title: &str, statement: &str) -> Need {
        let mut need = Need {
            id,
            title: title.to_owned(),
            state: self.config.lifecycle.default_state.clone(),
            priority: None,
            stakeholders: Vec::new(),
            keywords: Vec::new(),
            statement: statement.to_owned(),
            rationale: None,
            content_hash: None,
            acceptance: None,
        };
        need.content_hash = Some(need.compute_content_hash());
        need
    }

    /// Items whose stored content hash is missing or stale, with the file to update and the
    /// hash it should hold.
    pub fn stale_hashes(&self) -> Vec<StaleHash> {
        let reqs = self.requirements.iter().map(|(id, r)| {
            (
                EntityRef::Requirement(id.clone()),
                &self.files_by_id[id],
                r.content_hash.as_deref(),
                r.compute_content_hash(),
            )
        });
        let needs = self.needs.iter().map(|(id, n)| {
            (
                EntityRef::Need(id.clone()),
                &self.needs_by_id[id],
                n.content_hash.as_deref(),
                n.compute_content_hash(),
            )
        });
        reqs.chain(needs)
            .filter(|(_, _, stored, computed)| *stored != Some(computed.as_str()))
            .map(|(subject, path, _, computed)| StaleHash {
                subject,
                path: path.clone(),
                hash: computed,
            })
            .collect()
    }

    /// Write every stale or missing content hash, preserving formatting. Returns what changed.
    pub fn rehash(&mut self) -> Result<Vec<StaleHash>, RqtkError> {
        let stale = self.stale_hashes();
        for item in &stale {
            write_content_hash(&item.path, &item.hash)?;
            match &item.subject {
                EntityRef::Requirement(id) => {
                    if let Some(r) = self.requirements.get_mut(id) {
                        r.content_hash = Some(item.hash.clone());
                    }
                }
                EntityRef::Need(id) => {
                    if let Some(n) = self.needs.get_mut(id) {
                        n.content_hash = Some(item.hash.clone());
                    }
                }
                EntityRef::Stakeholder(_) => {}
            }
        }
        Ok(stale)
    }

    /// Edges that express derivation or dependency; a cycle among these is a modelling error.
    /// `conflicts_with` and `related` are symmetric by nature and are excluded.
    fn build_trace_graph(
        &self,
    ) -> (
        DiGraph<&RequirementId, ()>,
        HashMap<&RequirementId, NodeIndex>,
    ) {
        let mut graph = DiGraph::new();
        let nodes: HashMap<&RequirementId, NodeIndex> = self
            .requirements
            .keys()
            .map(|id| (id, graph.add_node(id)))
            .collect();
        for (id, req) in &self.requirements {
            let from = nodes[id];
            let t = &req.trace;
            for target in t
                .parents
                .iter()
                .chain(&t.depends_on)
                .chain(&t.derived_from)
                .chain(&t.refines)
            {
                if let Some(&to) = nodes.get(target) {
                    graph.add_edge(from, to, ());
                }
            }
        }
        (graph, nodes)
    }

    fn detect_cycles(&self) -> Vec<Diagnostic> {
        let (graph, _) = self.build_trace_graph();
        let mut issues = Vec::new();
        for component in tarjan_scc(&graph) {
            let is_cycle = component.len() > 1 || graph.contains_edge(component[0], component[0]);
            if !is_cycle {
                continue;
            }
            let mut members: Vec<&RequirementId> = component.iter().map(|&n| graph[n]).collect();
            members.sort();
            let listing = members
                .iter()
                .map(|id| id.0.as_str())
                .collect::<Vec<_>>()
                .join(" ↔ ");
            for id in members {
                issues.push(
                    Diagnostic::new("RQ017", format!("traceability cycle: {listing}"))
                        .subject(EntityRef::Requirement(id.clone()))
                        .file(&self.files_by_id[id])
                        .field("trace"),
                );
            }
        }
        issues
    }

    fn detect_orphans(&self) -> Vec<Diagnostic> {
        let is_root = |req: &Requirement| {
            self.config
                .categories
                .get(&req.category)
                .is_some_and(|c| c.is_root)
        };
        let root_ids: HashSet<RequirementId> = self
            .requirements
            .iter()
            .filter(|(_, req)| is_root(req))
            .map(|(id, _)| id.clone())
            .collect();

        let mut memo = HashMap::new();
        let mut issues = Vec::new();
        for (id, req) in &self.requirements {
            if is_root(req) {
                continue;
            }
            if !has_path_to_root(
                id,
                &self.requirements,
                &root_ids,
                &mut memo,
                &mut HashSet::new(),
            ) {
                issues.push(
                    Diagnostic::new(
                        "RQ018",
                        format!("orphan requirement `{id}` has no path to a root category"),
                    )
                    .subject(EntityRef::Requirement(id.clone()))
                    .file(&self.files_by_id[id])
                    .field("trace.parents"),
                );
            }
        }
        issues
    }

    fn check_repository_structure(&self) -> Vec<Diagnostic> {
        let layout = &self.config.repository;
        let mut issues = Vec::new();
        for rel_dir in &layout.required_dirs {
            let path = self.repo_root.join(rel_dir);
            if !path.is_dir() {
                issues.push(
                    Diagnostic::new(
                        "RQ019",
                        format!("repository is missing required directory `{rel_dir}`"),
                    )
                    .file(path),
                );
            }
        }
        for rel_file in &layout.required_files {
            let path = self.repo_root.join(rel_file);
            if !path.is_file() {
                issues.push(
                    Diagnostic::new(
                        "RQ020",
                        format!("repository is missing required file `{rel_file}`"),
                    )
                    .file(path),
                );
            }
        }
        issues
    }
}

fn write_content_hash(path: &Path, hash: &str) -> Result<(), RqtkError> {
    edit_toml_file(path, |doc| {
        doc.insert("content_hash", toml_edit::value(hash));
    })?;
    Ok(())
}

type Items<K, T> = (BTreeMap<K, T>, BTreeMap<K, PathBuf>);

/// Parse every `.toml` file under `dir` as a `T`. Files that fail to parse, reuse an ID
/// already seen (across all kinds), or whose name does not match their ID produce a
/// diagnostic; the first two are skipped.
fn load_items<K: Ord + Clone + AsRef<str>, T: DeserializeOwned>(
    dir: &Path,
    required: bool,
    id_of: impl Fn(&T) -> K,
    entity: impl Fn(K) -> EntityRef,
    seen_ids: &mut HashMap<String, PathBuf>,
    unloaded_ids: &mut HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<Items<K, T>, RqtkError> {
    let mut items = BTreeMap::new();
    let mut paths = BTreeMap::new();
    if !required && !dir.is_dir() {
        return Ok((items, paths));
    }
    for path in collect_toml_files(dir)? {
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) => {
                diagnostics
                    .push(Diagnostic::new("RQ100", format!("cannot read file: {e}")).file(&path));
                continue;
            }
        };
        let item: T = match toml::from_str(&text) {
            Ok(item) => item,
            Err(e) => {
                if let Some(id) = toml::from_str::<toml::Table>(&text)
                    .ok()
                    .and_then(|t| t.get("id")?.as_str().map(str::to_owned))
                {
                    unloaded_ids.insert(id);
                }
                let message = e.message().trim().to_owned();
                let diag = Diagnostic::new("RQ100", message);
                diagnostics.push(match e.span() {
                    Some(span) => {
                        let (line, col) = line_col(&text, span.start);
                        diag.at(&path, line, col)
                    }
                    None => diag.file(&path),
                });
                continue;
            }
        };
        let id = id_of(&item);
        if let Some(first) = seen_ids.get(id.as_ref()) {
            diagnostics.push(
                Diagnostic::new(
                    "RQ101",
                    format!(
                        "ID `{}` is already defined in {}",
                        id.as_ref(),
                        first.display()
                    ),
                )
                .subject(entity(id.clone()))
                .file(&path)
                .field("id"),
            );
            continue;
        }
        if path.file_stem().and_then(|s| s.to_str()) != Some(id.as_ref()) {
            diagnostics.push(
                Diagnostic::new(
                    "RQ102",
                    format!(
                        "file name does not match ID; expected `{}.toml`",
                        id.as_ref()
                    ),
                )
                .subject(entity(id.clone()))
                .file(&path)
                .field("id"),
            );
        }
        seen_ids.insert(id.as_ref().to_owned(), path.clone());
        paths.insert(id.clone(), path);
        items.insert(id, item);
    }
    Ok((items, paths))
}

/// All `.toml` files below `dir`, recursively, in sorted order.
fn collect_toml_files(dir: &Path) -> Result<Vec<PathBuf>, RqtkError> {
    let mut result = Vec::new();
    let entries = fs::read_dir(dir).map_err(|source| RqtkError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    for entry in entries {
        let path = entry
            .map_err(|source| RqtkError::Io {
                path: dir.to_path_buf(),
                source,
            })?
            .path();
        if path.is_dir() {
            result.extend(collect_toml_files(&path)?);
        } else if path.extension().and_then(|e| e.to_str()) == Some("toml") {
            result.push(path);
        }
    }
    result.sort();
    Ok(result)
}

fn read_file(path: &Path) -> Result<String, RqtkError> {
    fs::read_to_string(path).map_err(|source| RqtkError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn is_blank(s: Option<&str>) -> bool {
    s.is_none_or(|s| s.trim().is_empty())
}
