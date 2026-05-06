use crate::error::RqtkError;
use crate::model::{ProjectConfig, RepositoryLayout, RqtkConfig};
use crate::model::{
    RequirementBody, RequirementFile, RequirementId, ScaffoldInput, Statement, Status, Tags,
    Traceability, Verification,
};
use crate::validation::{
    LintIssue, has_path_to_stake, is_single_shall_sentence_violation, issue_error, issue_warning,
};
use chrono::Utc;
use petgraph::algo::is_cyclic_directed;
use petgraph::dot::{Config, Dot};
use petgraph::graph::{DiGraph, NodeIndex};
use regex::Regex;
use semver::Version;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

pub struct Loaded;
pub struct Validated;

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

#[derive(Debug, Clone)]
pub struct RequirementSet<S = Loaded> {
    pub config: ProjectConfig,
    pub requirements: BTreeMap<RequirementId, RequirementFile>,
    pub files_by_id: BTreeMap<RequirementId, PathBuf>,
    pub root: PathBuf,
    pub repo_root: PathBuf,
    pub config_path: PathBuf,
    repository_layout: RepositoryLayout,
    id_regex: Regex,
    allowed_states: HashSet<String>,
    allowed_priorities: HashSet<String>,
    allowed_criticalities: HashSet<String>,
    allowed_types: HashSet<String>,
    allowed_methods: HashSet<String>,
    parent_required: HashSet<String>,
    _state: PhantomData<S>,
}

impl RequirementSet<Loaded> {
    pub fn load_from_repo_root(repo_root: impl AsRef<Path>) -> Result<Self, RqtkError> {
        let repo_root = repo_root.as_ref().to_path_buf();
        let config_path = repo_root.join("rqtk.toml");
        let config_str = read_file(&config_path)?;
        let config: RqtkConfig =
            toml::from_str(&config_str).map_err(|source| RqtkError::TomlParse {
                path: config_path.clone(),
                source,
            })?;

        let requirements_root = repo_root.join(&config.repository.requirements_dir);
        Self::load_from_parts(
            repo_root,
            requirements_root,
            config_path,
            config.project_config,
            config.repository,
        )
    }

    pub fn load_from_requirements_dir(
        requirements_dir: impl AsRef<Path>,
    ) -> Result<Self, RqtkError> {
        let requirements_root = requirements_dir.as_ref().to_path_buf();
        let config_path = requirements_root.join("requirements.toml");
        let config_str = read_file(&config_path)?;
        let config: ProjectConfig =
            toml::from_str(&config_str).map_err(|source| RqtkError::TomlParse {
                path: config_path.clone(),
                source,
            })?;
        let repo_root = requirements_root
            .parent()
            .map_or_else(|| requirements_root.clone(), Path::to_path_buf);
        let repository_layout = RepositoryLayout {
            requirements_dir: requirements_root
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("requirements")
                .to_owned(),
            required_files: Vec::new(),
            required_dirs: Vec::new(),
        };
        Self::load_from_parts(
            repo_root,
            requirements_root,
            config_path,
            config,
            repository_layout,
        )
    }

    fn load_from_parts(
        repo_root: PathBuf,
        requirements_root: PathBuf,
        config_path: PathBuf,
        config: ProjectConfig,
        repository_layout: RepositoryLayout,
    ) -> Result<Self, RqtkError> {
        let id_regex = Regex::new(&config.identification.id_pattern).map_err(|source| {
            RqtkError::InvalidIdPattern {
                pattern: config.identification.id_pattern.clone(),
                source,
            }
        })?;

        let allowed_states: HashSet<String> = config.lifecycle.states.iter().cloned().collect();
        let allowed_priorities: HashSet<String> = config.priority.levels.iter().cloned().collect();
        let allowed_criticalities: HashSet<String> =
            config.criticality.levels.iter().cloned().collect();
        let allowed_types: HashSet<String> = config.req_types.allowed.iter().cloned().collect();
        let allowed_methods: HashSet<String> =
            config.verification.methods.iter().cloned().collect();
        let parent_required: HashSet<String> = config
            .validation
            .require_parent_for_levels
            .iter()
            .cloned()
            .collect();

        let mut requirements = BTreeMap::new();
        let mut files_by_id = BTreeMap::new();

        let toml_files = collect_toml_files(&requirements_root)?;
        for path in toml_files {
            if path.file_name().and_then(|n| n.to_str()) == Some("requirements.toml") {
                continue;
            }
            let text = read_file(&path)?;
            let req_file: RequirementFile =
                toml::from_str(&text).map_err(|source| RqtkError::TomlParse {
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
            root: requirements_root,
            repo_root,
            config_path,
            repository_layout,
            id_regex,
            allowed_states,
            allowed_priorities,
            allowed_criticalities,
            allowed_types,
            allowed_methods,
            parent_required,
            _state: PhantomData,
        })
    }

    pub fn validate(self) -> (RequirementSet<Validated>, Vec<LintIssue>) {
        let mut issues = Vec::new();

        let known_ids: HashSet<_> = self.requirements.keys().cloned().collect();
        let known_categories: HashSet<_> = self.config.categories.keys().cloned().collect();

        for (req_id, req_file) in &self.requirements {
            let req = &req_file.requirement;
            let path = self.files_by_id.get(req_id).cloned();

            if !self.id_regex.is_match(&req_id.0) {
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
            if !self.allowed_types.contains(req.req_type.as_str()) {
                issues.push(issue_error(
                    "RQ003",
                    format!("unknown requirement type `{}`", req.req_type),
                    Some(req_id.clone()),
                    path.clone(),
                ));
            }
            if !self.allowed_states.contains(req.status.state.as_str()) {
                issues.push(issue_error(
                    "RQ004",
                    format!("invalid lifecycle state `{}`", req.status.state),
                    Some(req_id.clone()),
                    path.clone(),
                ));
            }
            if !self
                .allowed_priorities
                .contains(req.status.priority.as_str())
            {
                issues.push(issue_error(
                    "RQ005",
                    format!("invalid priority `{}`", req.status.priority),
                    Some(req_id.clone()),
                    path.clone(),
                ));
            }
            if let Some(criticality) = req.status.criticality.as_deref() {
                if !self.allowed_criticalities.contains(criticality) {
                    issues.push(issue_error(
                        "RQ006",
                        format!("invalid criticality `{criticality}`"),
                        Some(req_id.clone()),
                        path.clone(),
                    ));
                }
            }
            if self.config.validation.require_rationale
                && req
                    .statement
                    .rationale
                    .as_deref()
                    .unwrap_or("")
                    .trim()
                    .is_empty()
            {
                issues.push(issue_error(
                    "RQ007",
                    "rationale is required but missing".to_owned(),
                    Some(req_id.clone()),
                    path.clone(),
                ));
            }
            if self.config.validation.require_verification_method
                && req.verification.method.trim().is_empty()
            {
                issues.push(issue_error(
                    "RQ008",
                    "verification.method is required but missing".to_owned(),
                    Some(req_id.clone()),
                    path.clone(),
                ));
            }
            if !self
                .allowed_methods
                .contains(req.verification.method.as_str())
            {
                issues.push(issue_error(
                    "RQ009",
                    format!("invalid verification method `{}`", req.verification.method),
                    Some(req_id.clone()),
                    path.clone(),
                ));
            }
            if is_single_shall_sentence_violation(
                &req.statement.text,
                &self.config.validation.shall_keywords,
            ) {
                issues.push(issue_error(
                    "RQ010",
                    "statement must contain exactly one normative sentence with a shall keyword"
                        .to_owned(),
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
            if self.parent_required.contains(req.category.as_str())
                && req.traceability.parents.is_empty()
            {
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
        issues.extend(self.check_repository_structure());

        let validated = RequirementSet {
            config: self.config,
            requirements: self.requirements,
            files_by_id: self.files_by_id,
            root: self.root,
            repo_root: self.repo_root,
            config_path: self.config_path,
            repository_layout: self.repository_layout,
            id_regex: self.id_regex,
            allowed_states: self.allowed_states,
            allowed_priorities: self.allowed_priorities,
            allowed_criticalities: self.allowed_criticalities,
            allowed_types: self.allowed_types,
            allowed_methods: self.allowed_methods,
            parent_required: self.parent_required,
            _state: PhantomData,
        };
        (validated, issues)
    }
}

impl RequirementSet<Validated> {
    pub fn coverage_gaps(&self) -> Vec<RequirementId> {
        let mut gaps = Vec::new();
        for (id, req) in &self.requirements {
            let ver = &req.requirement.verification;
            if ver.activities.is_empty()
                || ver
                    .success_criteria
                    .as_deref()
                    .unwrap_or("")
                    .trim()
                    .is_empty()
            {
                gaps.push(id.clone());
            }
        }
        gaps
    }

    pub fn to_dot(&self) -> String {
        let graph = self.build_trace_graph();
        format!("{:?}", Dot::with_config(&graph, &[Config::EdgeNoLabel]))
    }
}

impl<S> RequirementSet<S> {
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

    pub fn category_dir(&self, category: &str) -> PathBuf {
        self.root.join(category)
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
        is_cyclic_directed(&self.build_trace_graph())
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
        let root_ids: HashSet<_> = self
            .requirements
            .iter()
            .filter_map(|(id, req)| {
                let cat = self.config.categories.get(&req.requirement.category)?;
                if cat.is_root { Some(id.clone()) } else { None }
            })
            .collect();

        let mut memo: HashMap<RequirementId, bool> = HashMap::new();
        for (id, req) in &self.requirements {
            let is_root = self
                .config
                .categories
                .get(&req.requirement.category)
                .map_or(false, |c| c.is_root);
            if is_root {
                continue;
            }
            if !has_path_to_stake(
                id,
                &self.requirements,
                &root_ids,
                &mut memo,
                &mut HashSet::new(),
            ) {
                issues.push(issue_warning(
                    "RQ018",
                    format!("orphan requirement `{id}` has no path to a root category"),
                    Some(id.clone()),
                    self.files_by_id.get(id).cloned(),
                ));
            }
        }
        issues
    }

    fn check_repository_structure(&self) -> Vec<LintIssue> {
        let mut issues = Vec::new();
        for rel_dir in &self.repository_layout.required_dirs {
            let path = self.repo_root.join(rel_dir);
            if !path.is_dir() {
                issues.push(issue_error(
                    "RQ019",
                    format!("repository is missing required directory `{rel_dir}`"),
                    None,
                    Some(path),
                ));
            }
        }
        for rel_file in &self.repository_layout.required_files {
            let path = self.repo_root.join(rel_file);
            if !path.is_file() {
                issues.push(issue_error(
                    "RQ020",
                    format!("repository is missing required file `{rel_file}`"),
                    None,
                    Some(path),
                ));
            }
        }
        issues
    }
}

fn collect_toml_files(dir: &Path) -> Result<Vec<PathBuf>, RqtkError> {
    let mut result = Vec::new();
    let entries = fs::read_dir(dir).map_err(|source| RqtkError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| RqtkError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if path.is_dir() {
            result.extend(collect_toml_files(&path)?);
        } else if is_requirement_file(&path) {
            result.push(path);
        }
    }
    Ok(result)
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

fn parse_requirement_ordinal(
    req_id: &RequirementId,
    category: &str,
    prefix: &str,
    sep: &str,
) -> Option<usize> {
    let expected_prefix = format!("{prefix}{sep}{category}{sep}");
    if !req_id.0.starts_with(&expected_prefix) {
        return None;
    }
    req_id.0[expected_prefix.len()..].parse::<usize>().ok()
}
