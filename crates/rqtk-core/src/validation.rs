use crate::model::{RequirementFile, RequirementId};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LintSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone)]
pub struct LintIssue {
    pub severity: LintSeverity,
    pub code: &'static str,
    pub message: String,
    pub requirement_id: Option<RequirementId>,
    pub path: Option<PathBuf>,
}

pub fn issue_error(
    code: &'static str,
    message: String,
    requirement_id: Option<RequirementId>,
    path: Option<PathBuf>,
) -> LintIssue {
    LintIssue {
        severity: LintSeverity::Error,
        code,
        message,
        requirement_id,
        path,
    }
}

pub fn issue_warning(
    code: &'static str,
    message: String,
    requirement_id: Option<RequirementId>,
    path: Option<PathBuf>,
) -> LintIssue {
    LintIssue {
        severity: LintSeverity::Warning,
        code,
        message,
        requirement_id,
        path,
    }
}

pub fn is_single_shall_sentence_violation(text: &str, shall_keywords: &[String]) -> bool {
    // Count only terminal punctuation: '.' followed by whitespace or end-of-string
    // (excludes dots inside version numbers like "2.0.0" or filenames like "rqtk.toml"),
    // plus '!' and '?' anywhere.
    let dot_sentences = text
        .char_indices()
        .filter(|&(i, c)| {
            c == '.'
                && text[i + 1..]
                    .chars()
                    .next()
                    .is_none_or(|next| next.is_whitespace())
        })
        .count();
    let sentence_count = dot_sentences + text.matches('!').count() + text.matches('?').count();
    if sentence_count != 1 {
        return true;
    }
    let lower = text.to_lowercase();
    let has_shall = shall_keywords
        .iter()
        .any(|kw| lower.contains(&kw.to_lowercase()));
    !has_shall
}

pub fn has_path_to_stake(
    id: &RequirementId,
    reqs: &BTreeMap<RequirementId, RequirementFile>,
    stake_ids: &HashSet<RequirementId>,
    memo: &mut HashMap<RequirementId, bool>,
    visiting: &mut HashSet<RequirementId>,
) -> bool {
    if stake_ids.contains(id) {
        memo.insert(id.clone(), true);
        return true;
    }
    if let Some(hit) = memo.get(id) {
        return *hit;
    }
    if !visiting.insert(id.clone()) {
        return false;
    }
    let mut result = false;
    if let Some(req) = reqs.get(id) {
        for parent in &req.requirement.traceability.parents {
            if has_path_to_stake(parent, reqs, stake_ids, memo, visiting) {
                result = true;
                break;
            }
        }
    }
    visiting.remove(id);
    memo.insert(id.clone(), result);
    result
}
