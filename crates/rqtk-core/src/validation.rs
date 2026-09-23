use crate::model::{Requirement, RequirementId};
use std::collections::{BTreeMap, HashMap, HashSet};

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

pub fn has_path_to_root(
    id: &RequirementId,
    reqs: &BTreeMap<RequirementId, Requirement>,
    root_ids: &HashSet<RequirementId>,
    memo: &mut HashMap<RequirementId, bool>,
    visiting: &mut HashSet<RequirementId>,
) -> bool {
    if root_ids.contains(id) {
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
        for parent in &req.trace.parents {
            if has_path_to_root(parent, reqs, root_ids, memo, visiting) {
                result = true;
                break;
            }
        }
    }
    visiting.remove(id);
    memo.insert(id.clone(), result);
    result
}
