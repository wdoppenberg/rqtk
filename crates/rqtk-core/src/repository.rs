use crate::error::RqtkError;
use crate::git::GitContext;
use crate::io::write_requirement_file;
use crate::model::{Approval, ProjectConfig, RepositoryLayout, RqtkConfig};
use crate::model::{
    NeedFile, NeedId, RequirementBody, RequirementFile, RequirementId, ScaffoldInput, Statement,
    Status, StakeholderFile, Traceability, Verification,
};
use crate::validation::{
    LintIssue, has_path_to_stake, is_single_shall_sentence_violation, issue_error, issue_warning,
};

use petgraph::algo::is_cyclic_directed;
use petgraph::graph::{DiGraph, NodeIndex};
use regex::Regex;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

pub struct Loaded;
pub struct Validated;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClosureStatus {
    /// All activities have a terminal status (Passed or Waived).
    Verified,
    /// Activities defined and at least one has been started, but not all are terminal.
    InProgress,
    /// Activities and success criteria defined, but none have been executed yet.
    Planned,
    /// No activities defined, or success criteria missing.
    Gap,
}

fn closure_status_for(ver: &crate::model::Verification) -> ClosureStatus {
    if ver.activities.is_empty()
        || ver
            .success_criteria
            .as_deref()
            .unwrap_or("")
            .trim()
            .is_empty()
    {
        return ClosureStatus::Gap;
    }

    const TERMINAL: &[&str] = &["Passed", "Waived"];

    let any_started = ver
        .activities
        .iter()
        .any(|a| a.executed_at.is_some() || a.status.as_deref().is_some_and(|s| !s.is_empty()));
    let all_terminal = ver
        .activities
        .iter()
        .all(|a| a.status.as_deref().is_some_and(|s| TERMINAL.contains(&s)));

    if all_terminal {
        ClosureStatus::Verified
    } else if any_started {
        ClosureStatus::InProgress
    } else {
        ClosureStatus::Planned
    }
}

#[derive(Debug, Clone)]
pub struct TraceView {
    pub upward: Vec<RequirementId>,
    pub downward: Vec<RequirementId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SatisfactionStatus {
    /// At least one requirement's `satisfies` list names this need.
    Satisfied,
    /// No requirement points to this need yet.
    Unsatisfied,
}

#[derive(Debug, Clone, Copy)]
enum TraceEdge {
    Parent,
    Dependency,
    DerivedFrom,
    Refines,
    ConflictsWith,
}

/// gix::Repository is not Clone, so RequirementSet is not Clone either.
#[derive(Debug)]
pub struct RequirementSet<S = Loaded> {
    pub config: ProjectConfig,
    pub requirements: BTreeMap<RequirementId, RequirementFile>,
    pub needs: BTreeMap<NeedId, NeedFile>,
    pub stakeholders: BTreeMap<String, StakeholderFile>,
    pub files_by_id: BTreeMap<RequirementId, PathBuf>,
    pub needs_by_id: BTreeMap<NeedId, PathBuf>,
    pub stakeholders_by_id: BTreeMap<String, PathBuf>,
    pub root: PathBuf,
    pub needs_root: PathBuf,
    pub stakeholders_root: PathBuf,
    pub repo_root: PathBuf,
    pub config_path: PathBuf,
    pub git: GitContext,
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
        let git = GitContext::open(&repo_root)?;
        let config_path = repo_root.join(".rqtk/config.toml");
        let config_str = read_file(&config_path)?;
        let config: RqtkConfig =
            toml::from_str(&config_str).map_err(|source| RqtkError::TomlParse {
                path: config_path.clone(),
                source,
            })?;

        let requirements_root = repo_root.join(&config.repository.requirements_dir);
        let needs_root = repo_root.join(&config.repository.needs_dir);
        let stakeholders_root = repo_root.join(&config.repository.stakeholders_dir);
        Self::load_from_parts(
            repo_root,
            requirements_root,
            needs_root,
            stakeholders_root,
            config_path,
            config.project_config,
            config.repository,
            git,
        )
    }

    fn load_from_parts(
        repo_root: PathBuf,
        requirements_root: PathBuf,
        needs_root: PathBuf,
        stakeholders_root: PathBuf,
        config_path: PathBuf,
        config: ProjectConfig,
        repository_layout: RepositoryLayout,
        git: GitContext,
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

        let mut needs = BTreeMap::new();
        let mut needs_by_id = BTreeMap::new();
        if needs_root.is_dir() {
            for path in collect_toml_files(&needs_root)? {
                let text = read_file(&path)?;
                let need_file: NeedFile =
                    toml::from_str(&text).map_err(|source| RqtkError::TomlParse {
                        path: path.clone(),
                        source,
                    })?;
                let need_id = need_file.need.id.clone();
                needs_by_id.insert(need_id.clone(), path);
                needs.insert(need_id, need_file);
            }
        }

        let mut stakeholders = BTreeMap::new();
        let mut stakeholders_by_id = BTreeMap::new();
        if stakeholders_root.is_dir() {
            for path in collect_toml_files(&stakeholders_root)? {
                let text = read_file(&path)?;
                let stk_file: StakeholderFile =
                    toml::from_str(&text).map_err(|source| RqtkError::TomlParse {
                        path: path.clone(),
                        source,
                    })?;
                let stk_id = stk_file.stakeholder.id.clone();
                stakeholders_by_id.insert(stk_id.clone(), path);
                stakeholders.insert(stk_id, stk_file);
            }
        }

        Ok(Self {
            config,
            requirements,
            needs,
            stakeholders,
            files_by_id,
            needs_by_id,
            stakeholders_by_id,
            root: requirements_root,
            needs_root,
            stakeholders_root,
            repo_root,
            config_path,
            git,
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

    /// Write `baselined_at` and `baselined_by` into every requirement file.
    /// Returns the paths of all files that were written.
    pub fn stamp_baseline(
        &mut self,
        by: &str,
        date: chrono::NaiveDate,
    ) -> Result<Vec<std::path::PathBuf>, RqtkError> {
        let mut changed = Vec::new();
        for (id, req_file) in &mut self.requirements {
            let approval = req_file
                .requirement
                .approval
                .get_or_insert_with(|| Approval {
                    baselined_at: None,
                    baselined_by: None,
                    approved_by: Vec::new(),
                    ecr_ids: Vec::new(),
                });
            approval.baselined_at = Some(date);
            approval.baselined_by = Some(by.to_owned());
            let path = self.files_by_id[id].clone();
            write_requirement_file(&path, req_file)?;
            changed.push(path);
        }
        Ok(changed)
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
            if let Some(criticality) = req.status.criticality.as_deref()
                && !self.allowed_criticalities.contains(criticality)
            {
                issues.push(issue_error(
                    "RQ006",
                    format!("invalid criticality `{criticality}`"),
                    Some(req_id.clone()),
                    path.clone(),
                ));
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
            if let Some(stored_hash) = &req.content_hash {
                let computed = req.compute_content_hash();
                if stored_hash != &computed {
                    issues.push(issue_warning(
                        "RQ021",
                        format!(
                            "content hash is stale (stored {}, computed {}) — run `rqtk rehash`",
                            &stored_hash[..8.min(stored_hash.len())],
                            &computed[..8.min(computed.len())]
                        ),
                        Some(req_id.clone()),
                        path.clone(),
                    ));
                }
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
            let known_need_ids: HashSet<_> = self.needs.keys().cloned().collect();
            for need_id in &req.traceability.satisfies {
                if !known_need_ids.contains(need_id) {
                    issues.push(issue_error(
                        "RQ022",
                        format!("satisfies references unknown need `{need_id}`"),
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

    pub fn verification_closure(&self) -> BTreeMap<RequirementId, ClosureStatus> {
        self.requirements
            .iter()
            .map(|(id, req)| {
                let ver = &req.requirement.verification;
                let status = closure_status_for(ver);
                (id.clone(), status)
            })
            .collect()
    }

    pub fn satisfaction_closure(&self) -> BTreeMap<NeedId, SatisfactionStatus> {
        let mut result: BTreeMap<NeedId, SatisfactionStatus> = self
            .needs
            .keys()
            .map(|id| (id.clone(), SatisfactionStatus::Unsatisfied))
            .collect();
        for req_file in self.requirements.values() {
            for need_id in &req_file.requirement.traceability.satisfies {
                result.insert(need_id.clone(), SatisfactionStatus::Satisfied);
            }
        }
        result
    }

    pub fn to_dot(&self) -> String {
        const PALETTE: &[&str] = &[
            "#AED6F1", "#A9DFBF", "#FAD7A0", "#F1948A",
            "#D7BDE2", "#A3E4D7", "#F9E79F", "#FADBD8",
        ];

        let mut cats: Vec<&str> = self
            .requirements
            .values()
            .map(|r| r.requirement.category.as_str())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        cats.sort_unstable();

        let color_map: HashMap<&str, &str> = cats
            .iter()
            .enumerate()
            .map(|(i, &c)| (c, PALETTE[i % PALETTE.len()]))
            .collect();

        let mut out = String::from("digraph {\n");
        out.push_str("    node [style=filled fontname=\"Helvetica\"];\n");
        out.push_str("    edge [fontname=\"Helvetica\" fontsize=10];\n");

        for (id, req_file) in &self.requirements {
            let req = &req_file.requirement;
            let color = color_map.get(req.category.as_str()).copied().unwrap_or("#FFFFFF");
            let is_root = self
                .config
                .categories
                .get(&req.category)
                .is_some_and(|c| c.is_root);
            let shape = if is_root { "box" } else { "ellipse" };
            let tooltip = dot_escape(&format!(
                "{}\\n{}  |  {}  |  {}",
                req.title, req.category, req.status.state, req.status.priority
            ));
            out.push_str(&format!(
                "    \"{}\" [ label=\"{}\" shape={} fillcolor=\"{}\" tooltip=\"{}\" ];\n",
                id.0, id.0, shape, color, tooltip
            ));
        }

        for (id, req_file) in &self.requirements {
            let t = &req_file.requirement.traceability;
            for parent in &t.parents {
                out.push_str(&format!(
                    "    \"{}\" -> \"{}\";\n",
                    id.0, parent.0
                ));
            }
            for dep in &t.depends_on {
                out.push_str(&format!(
                    "    \"{}\" -> \"{}\" [ style=dashed color=blue label=depends ];\n",
                    id.0, dep.0
                ));
            }
            for src in &t.derived_from {
                out.push_str(&format!(
                    "    \"{}\" -> \"{}\" [ style=dotted color=gray label=derived ];\n",
                    id.0, src.0
                ));
            }
            for tgt in &t.refines {
                out.push_str(&format!(
                    "    \"{}\" -> \"{}\" [ style=dashed color=purple label=refines ];\n",
                    id.0, tgt.0
                ));
            }
            for tgt in &t.conflicts_with {
                out.push_str(&format!(
                    "    \"{}\" -> \"{}\" [ style=dashed color=red label=conflicts ];\n",
                    id.0, tgt.0
                ));
            }
        }

        out.push_str("}\n");
        out
    }

    pub fn to_graphml(&self) -> String {
        let mut out = String::new();
        out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        out.push_str("<graphml xmlns=\"http://graphml.graphdrawing.org/graphml\"\n");
        out.push_str("         xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"\n");
        out.push_str("         xsi:schemaLocation=\"http://graphml.graphdrawing.org/graphml http://graphml.graphdrawing.org/graphml/graphml-attributes.xsd\">\n");

        let node_keys = [
            ("d_title",    "title",                "string"),
            ("d_cat",      "category",             "string"),
            ("d_type",     "req_type",             "string"),
            ("d_state",    "state",                "string"),
            ("d_priority", "priority",             "string"),
            ("d_crit",     "criticality",          "string"),
            ("d_vmethod",  "verification_method",  "string"),
            ("d_vlevel",   "verification_level",   "string"),
            ("d_tbd",      "tbd",                  "boolean"),
            ("d_tbr",      "tbr",                  "boolean"),
            ("d_stmt",     "statement",            "string"),
        ];
        for (key_id, name, typ) in &node_keys {
            out.push_str(&format!(
                "  <key id=\"{}\" for=\"node\" attr.name=\"{}\" attr.type=\"{}\"/>\n",
                key_id, name, typ
            ));
        }
        out.push_str("  <key id=\"d_etype\" for=\"edge\" attr.name=\"type\" attr.type=\"string\"/>\n");

        out.push_str("  <graph id=\"G\" edgedefault=\"directed\">\n");

        for (id, req_file) in &self.requirements {
            let req = &req_file.requirement;
            out.push_str(&format!("    <node id=\"{}\">\n", xml_escape(&id.0)));
            out.push_str(&gml_data("d_title",    &xml_escape(&req.title)));
            out.push_str(&gml_data("d_cat",      &xml_escape(&req.category)));
            out.push_str(&gml_data("d_type",     &xml_escape(&req.req_type)));
            out.push_str(&gml_data("d_state",    &xml_escape(&req.status.state)));
            out.push_str(&gml_data("d_priority", &xml_escape(&req.status.priority)));
            out.push_str(&gml_data("d_crit",     &xml_escape(req.status.criticality.as_deref().unwrap_or(""))));
            out.push_str(&gml_data("d_vmethod",  &xml_escape(&req.verification.method)));
            out.push_str(&gml_data("d_vlevel",   &xml_escape(&req.verification.level)));
            out.push_str(&gml_data("d_tbd",      if req.status.tbd { "true" } else { "false" }));
            out.push_str(&gml_data("d_tbr",      if req.status.tbr { "true" } else { "false" }));
            out.push_str(&gml_data("d_stmt",     &xml_escape(&req.statement.text)));
            out.push_str("    </node>\n");
        }

        let mut edge_id = 0usize;
        for (id, req_file) in &self.requirements {
            let t = &req_file.requirement.traceability;
            let emit = |out: &mut String, eid: &mut usize, src: &str, tgt: &str, etype: &str| {
                out.push_str(&format!(
                    "    <edge id=\"e{}\" source=\"{}\" target=\"{}\">\n",
                    eid, xml_escape(src), xml_escape(tgt)
                ));
                out.push_str(&gml_data("d_etype", etype));
                out.push_str("    </edge>\n");
                *eid += 1;
            };
            for p in &t.parents         { emit(&mut out, &mut edge_id, &id.0, &p.0, "parent"); }
            for d in &t.depends_on      { emit(&mut out, &mut edge_id, &id.0, &d.0, "depends_on"); }
            for d in &t.derived_from    { emit(&mut out, &mut edge_id, &id.0, &d.0, "derived_from"); }
            for r in &t.refines         { emit(&mut out, &mut edge_id, &id.0, &r.0, "refines"); }
            for c in &t.conflicts_with  { emit(&mut out, &mut edge_id, &id.0, &c.0, "conflicts_with"); }
        }

        out.push_str("  </graph>\n");
        out.push_str("</graphml>\n");
        out
    }
}

fn dot_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn gml_data(key: &str, value: &str) -> String {
    format!("      <data key=\"{}\">{}</data>\n", key, value)
}

impl<S> RequirementSet<S> {
    pub fn trace_view(&self, root_id: &RequirementId) -> Result<TraceView, RqtkError> {
        if !self.requirements.contains_key(root_id) {
            return Err(RqtkError::RequirementNotFound(root_id.clone()));
        }

        let mut upward = Vec::new();
        let mut stack = vec![root_id.clone()];
        let mut seen = HashSet::new();
        seen.insert(root_id.clone());
        while let Some(id) = stack.pop() {
            if let Some(req) = self.requirements.get(&id) {
                for p in &req.requirement.traceability.parents {
                    if seen.insert(p.clone()) {
                        upward.push(p.clone());
                        stack.push(p.clone());
                    }
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
        down_seen.insert(root_id.clone());
        while let Some(id) = down_stack.pop() {
            if let Some(children) = children_map.get(&id) {
                for child in children {
                    if down_seen.insert(child.clone()) {
                        downward.push(child.clone());
                        down_stack.push(child.clone());
                    }
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
        let mut body = RequirementBody {
            id,
            title: input.title.to_owned(),
            category: input.category.to_owned(),
            req_type: input.req_type.to_owned(),

            content_hash: None,
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
            keywords: Vec::new(),
            custom: BTreeMap::new(),
        };
        body.content_hash = Some(body.compute_content_hash());
        RequirementFile { requirement: body }
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
            if let Some(&from) = nodes.get(id) {
                let t = &req.requirement.traceability;
                for p in &t.parents {
                    if let Some(&to) = nodes.get(p) { graph.add_edge(from, to, TraceEdge::Parent); }
                }
                for d in &t.depends_on {
                    if let Some(&to) = nodes.get(d) { graph.add_edge(from, to, TraceEdge::Dependency); }
                }
                for d in &t.derived_from {
                    if let Some(&to) = nodes.get(d) { graph.add_edge(from, to, TraceEdge::DerivedFrom); }
                }
                for r in &t.refines {
                    if let Some(&to) = nodes.get(r) { graph.add_edge(from, to, TraceEdge::Refines); }
                }
                for c in &t.conflicts_with {
                    if let Some(&to) = nodes.get(c) { graph.add_edge(from, to, TraceEdge::ConflictsWith); }
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
                .is_some_and(|c| c.is_root);
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
