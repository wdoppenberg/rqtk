use console::style;
use regex::{Regex, RegexBuilder};
use rqtk_core::{EntityRef, RequirementSet};
use serde::Serialize;
use std::{error::Error, path::Path, path::PathBuf};

use crate::output::{self, Ctx, Exit};

pub struct SearchArgs {
    pub pattern: String,
    pub ignore_case: bool,
    pub field: Option<Vec<String>>,
}

#[derive(Serialize)]
struct FieldMatch<'a> {
    field: &'static str,
    value: &'a str,
    /// Byte ranges of each match within `value`.
    ranges: Vec<(usize, usize)>,
}

#[derive(Serialize)]
struct Hit<'a> {
    subject: EntityRef,
    path: PathBuf,
    matches: Vec<FieldMatch<'a>>,
}

fn highlight(text: &str, ranges: &[(usize, usize)]) -> String {
    let mut out = String::new();
    let mut cursor = 0;
    for &(start, end) in ranges {
        out.push_str(&format!("{}", style(&text[cursor..start]).dim()));
        out.push_str(&format!("{}", style(&text[start..end]).red().bold()));
        cursor = end;
    }
    out.push_str(&format!("{}", style(&text[cursor..]).dim()));
    out
}

/// Match `re` against each wanted field; offsets always index the original text.
fn search_fields<'a>(
    re: &Regex,
    fields: Option<&[String]>,
    candidates: impl IntoIterator<Item = (&'static str, &'a str)>,
) -> Vec<FieldMatch<'a>> {
    let want = |name: &str| fields.is_none_or(|fs| fs.iter().any(|f| f.eq_ignore_ascii_case(name)));
    candidates
        .into_iter()
        .filter(|(field, _)| want(field))
        .filter_map(|(field, value)| {
            let ranges: Vec<_> = re.find_iter(value).map(|m| (m.start(), m.end())).collect();
            (!ranges.is_empty()).then_some(FieldMatch {
                field,
                value,
                ranges,
            })
        })
        .collect()
}

fn push_hit<'a>(
    hits: &mut Vec<Hit<'a>>,
    root: &Path,
    subject: EntityRef,
    path: &Path,
    matches: Vec<FieldMatch<'a>>,
) {
    if !matches.is_empty() {
        hits.push(Hit {
            subject,
            path: output::relative(path, root),
            matches,
        });
    }
}

/// Every loaded item has a file; `path_of` only fails for IDs not in the set.
fn file_of<'a, S>(set: &'a RequirementSet<S>, subject: &EntityRef) -> &'a Path {
    set.path_of(subject)
        .expect("item listed by the set has a file")
}

pub fn run(ctx: &Ctx, args: SearchArgs) -> Result<Exit, Box<dyn Error>> {
    let set = RequirementSet::load_from_repo_root(&ctx.root)?;
    let re = RegexBuilder::new(&regex::escape(&args.pattern))
        .case_insensitive(args.ignore_case)
        .build()?;
    let fields = args.field.as_deref();
    let mut hits = Vec::new();
    let mut push = |subject, path, matches| push_hit(&mut hits, &ctx.root, subject, path, matches);

    for (id, req) in set.requirements() {
        let mut candidates = vec![
            ("id", id.0.as_str()),
            ("title", req.title.as_str()),
            ("statement", req.statement.as_str()),
        ];
        candidates.extend(req.rationale.as_deref().map(|r| ("rationale", r)));
        candidates.extend(req.notes.as_deref().map(|n| ("notes", n)));
        candidates.extend(req.keywords.iter().map(|k| ("keywords", k.as_str())));
        let subject = EntityRef::Requirement(id.clone());
        let path = file_of(&set, &subject);
        push(subject, path, search_fields(&re, fields, candidates));
    }
    for (id, need) in set.needs() {
        let mut candidates = vec![
            ("id", id.0.as_str()),
            ("title", need.title.as_str()),
            ("statement", need.statement.as_str()),
        ];
        candidates.extend(need.rationale.as_deref().map(|r| ("rationale", r)));
        candidates.extend(need.keywords.iter().map(|k| ("keywords", k.as_str())));
        let subject = EntityRef::Need(id.clone());
        let path = file_of(&set, &subject);
        push(subject, path, search_fields(&re, fields, candidates));
    }
    for (id, stk) in set.stakeholders() {
        let mut candidates = vec![("id", id.0.as_str()), ("name", stk.name.as_str())];
        candidates.extend(stk.role.as_deref().map(|r| ("role", r)));
        candidates.extend(stk.organization.as_deref().map(|o| ("organization", o)));
        let subject = EntityRef::Stakeholder(id.clone());
        let path = file_of(&set, &subject);
        push(subject, path, search_fields(&re, fields, candidates));
    }

    if ctx.json() {
        output::json(&serde_json::json!({ "hits": hits }))?;
        return Ok(Exit::Ok);
    }
    for hit in &hits {
        println!("\n{}", style(hit.path.display()).bold().underlined());
        for m in &hit.matches {
            println!(
                "  {:<12} {}",
                style(m.field).cyan(),
                highlight(m.value, &m.ranges)
            );
        }
    }
    let total: usize = hits.iter().map(|h| h.matches.len()).sum();
    if total == 0 {
        eprintln!("  {} no matches for {:?}", style("·").dim(), args.pattern);
    } else {
        println!(
            "\n  {} {} match{} for {:?}",
            style("·").dim(),
            style(total).bold(),
            if total == 1 { "" } else { "es" },
            args.pattern,
        );
    }
    Ok(Exit::Ok)
}
