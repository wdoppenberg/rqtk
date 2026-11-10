//! Which requirements to list, as data: the same object in every client language, and what a
//! query language would parse into later.

use rqtk_core::ClosureStatus;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Conditions a requirement must meet. Every field that is set must match; a list matches
/// when any of its values does. The default filter matches everything.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Filter {
    /// Verification status, e.g. `["suspect", "failed"]`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub status: Vec<ClosureStatus>,
    /// Category key, e.g. `["SYS"]`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub category: Vec<String>,
    /// Requirement type, e.g. `["Interface"]`.
    #[serde(rename = "type", skip_serializing_if = "Vec::is_empty")]
    pub req_type: Vec<String>,
    /// Lifecycle state, e.g. `["Approved"]`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub state: Vec<String>,
    /// Priority, e.g. `["Critical", "High"]`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub priority: Vec<String>,
    /// Only requirements below this one through `trace.parents`, at any depth.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ancestor: Option<String>,
    /// Only requirements that consume this interface: `<alias>:<ID>`, or `<alias>` for any
    /// interface of that source.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consumes: Option<String>,
    /// Case-insensitive text in the ID, title, statement or rationale.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

impl Filter {
    pub(crate) fn matches_lists(
        &self,
        status: ClosureStatus,
        req: &rqtk_core::Requirement,
    ) -> bool {
        let any =
            |wanted: &[String], value: &str| wanted.is_empty() || wanted.iter().any(|w| w == value);
        (self.status.is_empty() || self.status.contains(&status))
            && any(&self.category, &req.category)
            && any(&self.req_type, &req.req_type)
            && any(&self.state, &req.state)
            && any(&self.priority, &req.priority)
            && self.consumes.as_deref().is_none_or(|wanted| {
                req.trace.consumes.iter().any(|c| {
                    c.0 == wanted
                        || c.0
                            .split_once(':')
                            .is_some_and(|(alias, _)| alias == wanted)
                })
            })
            && self.text.as_deref().is_none_or(|text| {
                let text = text.to_lowercase();
                [
                    Some(req.id.0.as_str()),
                    Some(req.title.as_str()),
                    Some(req.statement.as_str()),
                    req.rationale.as_deref(),
                ]
                .into_iter()
                .flatten()
                .any(|field| field.to_lowercase().contains(&text))
            })
    }
}
