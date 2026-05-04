use crate::error::RqtkError;
use crate::model::{
    RequirementBody, RequirementFile, RequirementId, ScaffoldInput, Statement, Status, Tags,
    Traceability, Verification,
};
use crate::model::ProjectConfig;
use crate::validation::{
    has_path_to_stake, is_single_shall_sentence_violation, issue_error, issue_warning, LintIssue,
};
use chrono::Utc;
use petgraph::algo::is_cyclic_directed;
use petgraph::dot::{Config, Dot};
use petgraph::graph::{DiGraph, NodeIndex};
use regex::Regex;
use semver::Version;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct RequirementSet {
    pub config: ProjectConfig,
    pub requirements: BTreeMap<RequirementId, RequirementFile>,
    pub files_by_id: BTreeMap<RequirementId, PathBuf>,
    pub root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct TraceView {
    pub upward: Vec<RequirementId>,
    pub downward: Vec<RequirementId>,
}

#[derive(Debug, Clone, Copy)]
enum TraceEdge {
    Parent,
    Dependency,
}

impl RequirementSet {
    pub fn load_from_repo_root(repo_root: impl AsRef<Path>) -> Result<Self, RqtkError> {
        let root = repo_root.as_ref().join("requirements");
        Self::load_from_requirements_dir(root)
    }

    pub fn load_from_requirements_dir(requirements_dir: impl AsRef<Path>) -> Result<Self, RqtkError> {
        let root = requirements_dir.as_ref().to_path_buf();
        let config_path = root.join("requirements.toml");
        let config_str = read_file(&config_path)?;
        let config: ProjectConfig = toml::from_str(&config_str).map_err(|source| RqtkError::TomlParse {
            path: config_path.clone(),
            source,
        })?;

        let mut requirements = BTreeMap::new();
        let mut files_by_id = BTreeMap::new();

        let entries = fs::read_dir(&root).map_err(|source| RqtkError::Io {
            path: root.clone(),
            source,
        })?;

        for entry in entries {
            let entry = entry.map_err(|source| RqtkError::Io {
                path: root.clone(),
                source,
            })?;
            let path = entry.path();
            if !is_requirement_file(&path) {
                continue;
            }
            if path.file_name().and_then(|n| n.to_str()) == Some("requirements.toml") {
                continue;
            }
            let text = read_file(&path)?;
            let req_file: RequirementFile = toml::from_str(&text).map_err(|source| RqtkError::TomlParse {
                path: path.clone(),
                source,
            })?;
            let req_id = req_file.requirement.id.clone();
            if requirements.contains_key(&req_id) {
                return Err(RqtkError::DuplicateRequirement(req_id));
            }
            files_by_id.insert(req_id.clone(), path);
            requirements.insert(req_id, req_file);
        }

        Ok(Self {
            config,
            requirements,
            files_by_id,
            root,
        })
    }

    pub fn validate(&self) -> Result<Vec<LintIssue>, RqtkError> {
        let id_regex = Regex::new(&self.config.identification.id_pattern).map_err(|source| {
            RqtkError::InvalidIdPattern {
                pattern: self.config.identification.id_pattern.clone(),
                source,
            }
        })?;
        let known_ids: HashSet<_> = self.requirements.keys().cloned().collect();
        let known_categories: HashSet<_> = self.config.categories.keys().cloned().collect();
        let allowed_types: HashSet<_> = self.config.req_types.allowed.iter().map(|s| s.as_str()).collect();
        let allowed_states: HashSet<_> = self.config.lifecycle.states.iter().map(|s| s.as_str()).collect();
        let allowed_priorities: HashSet<_> = self.config.priority.levels.iter().map(|s| s.as_str()).collect();
        let allowed_criticalities: HashSet<_> =
            self.config.criticality.levels.iter().map(|s| s.as_str()).collect();
        let allowed_methods: HashSet<_> =
            self.config.verification.methods.iter().map(|s| s.as_str()).collect();
        let parent_required: HashSet<_> = self
            .config
            .validation
            .require_parent_for_levels
            .iter()
            .map(|s| s.as_str())
            .collect();

        let mut issues = Vec::new();

        for (req_id, req_file) in &self.requirements {
            let req = &req_file.requirement;
            let path = self.files_by_id.get(req_id).cloned();

            if !id_regex.is_match(&req_id.0) {
                issues.push(issue_error(
                    "RQ001",
                    format!("ID `{}` does not match configured id_pattern", req_id),
                    Some(req_id.clone()),
                    path.clone(),
                ));
            }
            if !known_categories.contains(&req.category) {
                issues.push(issue_error(
                    "RQ002",
                    format!("unknown category `{}`", req.category),
                    Some(req_id.clone()),
                    path.clone(),
                ));
            }
            if !allowed_types.contains(req.req_type.as_str()) {
                issues.push(issue_error(
                    "RQ003",
                    format!("unknown requirement type `{}`", req.req_type),
                    Some(req_id.clone()),
                    path.clone(),
                ));
            }
            if !allowed_states.contains(req.status.state.as_str()) {
                issues.push(issue_error(
                    "RQ004",
                    format!("invalid lifecycle state `{}`", req.status.state),
                    Some(req_id.clone()),
                    path.clone(),
                ));
            }
            if !allowed_priorities.contains(req.status.priority.as_str()) {
                issues.push(issue_error(
                    "RQ005",
                    format!("invalid priority `{}`", req.status.priority),
                    Some(req_id.clone()),
                    path.clone(),
                ));
            }
            if let Some(criticality) = req.status.criticality.as_deref() {
                if !allowed_criticalities.contains(criticality) {
                    issues.push(issue_error(
                        "RQ006",
                        format!("invalid criticality `{criticality}`"),
                        Some(req_id.clone()),
                        path.clone(),
                    ));
                }
            }
            if self.config.validation.require_rationale
                && req.statement.rationale.as_deref().unwrap_or("").trim().is_empty()
            {
                issues.push(issue_error(
                    "RQ007",
                    "rationale is required but missing".to_owned(),
                    Some(req_id.clone()),
                    path.clone(),
                ));
            }
            if self.config.validation.require_verification_method && req.verification.method.trim().is_empty() {
                issues.push(issue_error(
                    "RQ008",
                    "verification.method is required but missing".to_owned(),
                    Some(req_id.clone()),
                    path.clone(),
                ));
            }
            if !allowed_methods.contains(req.verification.method.as_str()) {
                issues.push(issue_error(
                    "RQ009",
                    format!("invalid verification method `{}`", req.verification.method),
                    Some(req_id.clone()),
                    path.clone(),
                ));
            }
            if is_single_shall_sentence_violation(&req.statement.text, &self.config.validation.shall_keywords) {
                issues.push(issue_error(
                    "RQ010",
                    "statement must contain exactly one normative sentence with a shall keyword".to_owned(),
                    Some(req_id.clone()),
                    path.clone(),
                ));
            }
            if req.status.priority == "Mandatory" {
                let lower = req.statement.text.to_lowercase();
                for forbidden in &self.config.validation.forbidden_keywords {
                    if lower.contains(&forbidden.to_lowercase()) {
                        issues.push(issue_error(
                            "RQ011",
                            format!("mandatory statement uses forbidden keyword `{forbidden}`"),
                            Some(req_id.clone()),
                            path.clone(),
                        ));
                    }
                }
            }
            if parent_required.contains(req.category.as_str()) && req.traceability.parents.is_empty() {
                issues.push(issue_error(
                    "RQ012",
                    format!("category `{}` requires at least one parent", req.category),
                    Some(req_id.clone()),
                    path.clone(),
                ));
            }
            if req.status.tbd && !self.config.validation.allow_tbd {
                issues.push(issue_error(
                    "RQ013",
                    "TBD is not allowed by project policy".to_owned(),
                    Some(req_id.clone()),
                    path.clone(),
                ));
            }
            if req.status.tbr && !self.config.validation.allow_tbr {
                issues.push(issue_error(
                    "RQ014",
                    "TBR is not allowed by project policy".to_owned(),
                    Some(req_id.clone()),
                    path.clone(),
                ));
            }
            for parent in &req.traceability.parents {
                if !known_ids.contains(parent) {
                    issues.push(issue_error(
                        "RQ015",
                        format!("unknown parent reference `{parent}`"),
                        Some(req_id.clone()),
                        path.clone(),
                    ));
                }
            }
            for dep in &req.traceability.depends_on {
                if !known_ids.contains(dep) {
                    issues.push(issue_error(
                        "RQ016",
                        format!("unknown dependency `{dep}`"),
                        Some(req_id.clone()),
                        path.clone(),
                    ));
                }
            }
        }

        if self.config.validation.forbid_orphans {
            issues.extend(self.detect_orphans());
        }
        if self.config.validation.forbid_circular_traces && self.has_trace_cycles() {
            issues.push(issue_error(
                "RQ017",
                "traceability graph contains cycles".to_owned(),
                None,
                None,
            ));
        }
        Ok(issues)
    }

    pub fn coverage_gaps(&self) -> Vec<RequirementId> {
        let mut gaps = Vec::new();
        for (id, req) in &self.requirements {
            let ver = &req.requirement.verification;
            if ver.activities.is_empty() || ver.success_criteria.as_deref().unwrap_or("").trim().is_empty() {
                gaps.push(id.clone());
            }
        }
        gaps
    }

    pub fn trace_view(&self, root_id: &RequirementId) -> Result<TraceView, RqtkError> {
        if !self.requirements.contains_key(root_id) {
            return Err(RqtkError::RequirementNotFound(root_id.clone()));
        }

        let mut upward = Vec::new();
        let mut stack = vec![root_id.clone()];
        let mut seen = HashSet::new();
        while let Some(id) = stack.pop() {
            if !seen.insert(id.clone()) {
                continue;
            }
            upward.push(id.clone());
            if let Some(req) = self.requirements.get(&id) {
                for p in &req.requirement.traceability.parents {
                    stack.push(p.clone());
                }
            }
        }

        let mut children_map: HashMap<RequirementId, Vec<RequirementId>> = HashMap::new();
        for (id, req) in &self.requirements {
            for p in &req.requirement.traceability.parents {
                children_map.entry(p.clone()).or_default().push(id.clone());
            }
        }

        let mut downward = Vec::new();
        let mut down_stack = vec![root_id.clone()];
        let mut down_seen = HashSet::new();
        while let Some(id) = down_stack.pop() {
            if !down_seen.insert(id.clone()) {
                continue;
            }
            downward.push(id.clone());
            if let Some(children) = children_map.get(&id) {
                for child in children {
                    down_stack.push(child.clone());
                }
            }
        }

        Ok(TraceView { upward, downward })
    }

    pub fn to_dot(&self) -> String {
        let graph = self.build_trace_graph();
        format!("{:?}", Dot::with_config(&graph, &[Config::EdgeNoLabel]))
    }

    pub fn next_requirement_id(&self, category: &str) -> RequirementId {
        let mut max_ord = 0usize;
        for req_id in self.requirements.keys() {
            if let Some(ord) = parse_requirement_ordinal(
                req_id,
                category,
                &self.config.identification.prefix,
                &self.config.identification.id_separator,
            ) {
                max_ord = max_ord.max(ord);
            }
        }
        let next_ord = max_ord + 1;
        let id = format!(
            "{}{}{}{}",
            self.config.identification.prefix,
            self.config.identification.id_separator,
            category,
            self.config.identification.id_separator
        );
        RequirementId(format!(
            "{id}{:0width$}",
            next_ord,
            width = self.config.identification.zero_padding
        ))
    }

    pub fn scaffold_requirement(&self, input: ScaffoldInput<'_>) -> RequirementFile {
        let id = self.next_requirement_id(input.category);
        let now = Utc::now();
        RequirementFile {
            requirement: RequirementBody {
                id,
                title: input.title.to_owned(),
                category: input.category.to_owned(),
                req_type: input.req_type.to_owned(),
                version: Version::new(0, 1, 0),
                created: now,
                updated: now,
                statement: Statement {
                    text: input.statement.to_owned(),
                    rationale: input.rationale.map(ToOwned::to_owned),
                    assumptions: Vec::new(),
                    notes: None,
                },
                status: Status {
                    state: self.config.lifecycle.default_state.clone(),
                    priority: self
                        .config
                        .priority
                        .levels
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "Mandatory".to_owned()),
                    criticality: None,
                    maturity: None,
                    tbd: false,
                    tbr: false,
                },
                approval: None,
                parameters: Vec::new(),
                traceability: Traceability::default(),
                verification: Verification {
                    method: self
                        .config
                        .verification
                        .methods
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "Test".to_owned()),
                    level: self
                        .config
                        .verification
                        .levels
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "System".to_owned()),
                    phase: self
                        .config
                        .verification
                        .phases
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "Development".to_owned()),
                    owner: None,
                    success_criteria: None,
                    activities: Vec::new(),
                },
                validation: None,
                risk: None,
                allocation: None,
                history: Vec::new(),
                tags: Tags::default(),
                custom: BTreeMap::new(),
            },
        }
    }

    fn has_trace_cycles(&self) -> bool {
        let graph = self.build_trace_graph();
        is_cyclic_directed(&graph)
    }

    fn build_trace_graph(&self) -> DiGraph<RequirementId, TraceEdge> {
        let mut graph = DiGraph::<RequirementId, TraceEdge>::new();
        let mut nodes: HashMap<RequirementId, NodeIndex> = HashMap::new();

        for id in self.requirements.keys() {
            let node = graph.add_node(id.clone());
            nodes.insert(id.clone(), node);
        }

        for (id, req) in &self.requirements {
            if let Some(from) = nodes.get(id) {
                for parent in &req.requirement.traceability.parents {
                    if let Some(to) = nodes.get(parent) {
                        graph.add_edge(*from, *to, TraceEdge::Parent);
                    }
                }
                for dep in &req.requirement.traceability.depends_on {
                    if let Some(to) = nodes.get(dep) {
                        graph.add_edge(*from, *to, TraceEdge::Dependency);
                    }
                }
            }
        }
        graph
    }

    fn detect_orphans(&self) -> Vec<LintIssue> {
        let mut issues = Vec::new();
        let stake_ids: HashSet<_> = self
            .requirements
            .iter()
            .filter_map(|(id, req)| {
                if req.requirement.category == "STAKE" {
                    Some(id.clone())
                } else {
                    None
                }
            })
            .collect();

        let mut memo: HashMap<RequirementId, bool> = HashMap::new();
        for (id, req) in &self.requirements {
            if req.requirement.category == "STAKE" {
                continue;
            }
            if !has_path_to_stake(id, &self.requirements, &stake_ids, &mut memo, &mut HashSet::new()) {
                issues.push(issue_warning(
                    "RQ018",
                    format!("orphan requirement `{id}` has no path to STAKE"),
                    Some(id.clone()),
                    self.files_by_id.get(id).cloned(),
                ));
            }
        }
        issues
    }
}

fn read_file(path: &Path) -> Result<String, RqtkError> {
    fs::read_to_string(path).map_err(|source| RqtkError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn is_requirement_file(path: &Path) -> bool {
    path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("toml")
}

fn parse_requirement_ordinal(req_id: &RequirementId, category: &str, prefix: &str, sep: &str) -> Option<usize> {
    let expected_prefix = format!("{prefix}{sep}{category}{sep}");
    if !req_id.0.starts_with(&expected_prefix) {
        return None;
    }
    req_id.0[expected_prefix.len()..].parse::<usize>().ok()
}
