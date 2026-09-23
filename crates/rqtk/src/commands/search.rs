use console::style;
use regex::{Regex, RegexBuilder};
use rqtk_core::RequirementSet;
use std::{error::Error, path::Path};

pub struct SearchArgs {
    pub pattern: String,
    pub ignore_case: bool,
    pub field: Option<Vec<String>>,
}

struct FieldMatch<'a> {
    label: &'static str,
    value: &'a str,
    ranges: Vec<(usize, usize)>,
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
        .filter(|(label, _)| want(label))
        .filter_map(|(label, value)| {
            let ranges: Vec<_> = re.find_iter(value).map(|m| (m.start(), m.end())).collect();
            (!ranges.is_empty()).then_some(FieldMatch {
                label,
                value,
                ranges,
            })
        })
        .collect()
}

fn print_matches(path: &Path, repo_root: &Path, matches: &[FieldMatch<'_>]) {
    let shown = path.strip_prefix(repo_root).unwrap_or(path);
    println!("\n{}", style(shown.display()).bold().underlined());
    for m in matches {
        println!(
            "  {:<12} {}",
            style(m.label).cyan(),
            highlight(m.value, &m.ranges)
        );
    }
}

pub fn run(repo_root: &Path, args: SearchArgs) -> Result<(), Box<dyn Error>> {
    let set = RequirementSet::load_from_repo_root(repo_root)?;
    let re = RegexBuilder::new(&regex::escape(&args.pattern))
        .case_insensitive(args.ignore_case)
        .build()?;
    let fields = args.field.as_deref();
    let mut total = 0usize;
    let mut report = |path: &Path, matches: Vec<FieldMatch<'_>>| {
        if !matches.is_empty() {
            total += matches.len();
            print_matches(path, &set.repo_root, &matches);
        }
    };

    for (id, req) in &set.requirements {
        let mut candidates = vec![
            ("id", id.0.as_str()),
            ("title", req.title.as_str()),
            ("statement", req.statement.as_str()),
        ];
        candidates.extend(req.rationale.as_deref().map(|r| ("rationale", r)));
        candidates.extend(req.notes.as_deref().map(|n| ("notes", n)));
        candidates.extend(req.keywords.iter().map(|k| ("keywords", k.as_str())));
        report(&set.files_by_id[id], search_fields(&re, fields, candidates));
    }

    for (id, need) in &set.needs {
        let mut candidates = vec![
            ("id", id.0.as_str()),
            ("title", need.title.as_str()),
            ("statement", need.statement.as_str()),
        ];
        candidates.extend(need.rationale.as_deref().map(|r| ("rationale", r)));
        candidates.extend(need.keywords.iter().map(|k| ("keywords", k.as_str())));
        report(&set.needs_by_id[id], search_fields(&re, fields, candidates));
    }

    for (id, stk) in &set.stakeholders {
        let mut candidates = vec![("id", id.0.as_str()), ("name", stk.name.as_str())];
        candidates.extend(stk.role.as_deref().map(|r| ("role", r)));
        candidates.extend(stk.organization.as_deref().map(|o| ("organization", o)));
        report(
            &set.stakeholders_by_id[id],
            search_fields(&re, fields, candidates),
        );
    }

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

    Ok(())
}
