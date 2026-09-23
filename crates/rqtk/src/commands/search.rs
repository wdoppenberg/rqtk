use console::style;
use rqtk_core::{RequirementBody, RequirementSet};
use std::{error::Error, path::Path};

pub struct SearchArgs {
    pub pattern: String,
    pub ignore_case: bool,
    pub field: Option<Vec<String>>,
}

struct FieldMatch {
    label: &'static str,
    value: String,
    match_ranges: Vec<(usize, usize)>,
}

fn find_matches(haystack: &str, needle: &str, ignore_case: bool) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let (h, n) = if ignore_case {
        (haystack.to_lowercase(), needle.to_lowercase())
    } else {
        (haystack.to_owned(), needle.to_owned())
    };
    let mut start = 0;
    while let Some(pos) = h[start..].find(&n) {
        let abs = start + pos;
        ranges.push((abs, abs + needle.len()));
        start = abs + needle.len();
    }
    ranges
}

fn highlight(text: &str, ranges: &[(usize, usize)]) -> String {
    if ranges.is_empty() {
        return text.to_owned();
    }
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

fn search_req(
    req: &RequirementBody,
    pattern: &str,
    ignore_case: bool,
    fields: Option<&[String]>,
) -> Vec<FieldMatch> {
    let want = |name: &str| match fields {
        None => true,
        Some(fs) => fs.iter().any(|f| f.eq_ignore_ascii_case(name)),
    };

    let mut matches = Vec::new();

    if want("id") {
        let ranges = find_matches(&req.id.0, pattern, ignore_case);
        if !ranges.is_empty() {
            matches.push(FieldMatch {
                label: "id",
                value: req.id.0.clone(),
                match_ranges: ranges,
            });
        }
    }

    if want("title") {
        let ranges = find_matches(&req.title, pattern, ignore_case);
        if !ranges.is_empty() {
            matches.push(FieldMatch {
                label: "title",
                value: req.title.clone(),
                match_ranges: ranges,
            });
        }
    }

    if want("statement") {
        let ranges = find_matches(&req.statement.text, pattern, ignore_case);
        if !ranges.is_empty() {
            matches.push(FieldMatch {
                label: "statement",
                value: req.statement.text.clone(),
                match_ranges: ranges,
            });
        }
    }

    if want("rationale")
        && let Some(rat) = &req.statement.rationale
    {
        let ranges = find_matches(rat, pattern, ignore_case);
        if !ranges.is_empty() {
            matches.push(FieldMatch {
                label: "rationale",
                value: rat.clone(),
                match_ranges: ranges,
            });
        }
    }

    if want("notes")
        && let Some(notes) = &req.statement.notes
    {
        let ranges = find_matches(notes, pattern, ignore_case);
        if !ranges.is_empty() {
            matches.push(FieldMatch {
                label: "notes",
                value: notes.clone(),
                match_ranges: ranges,
            });
        }
    }

    if want("keywords") {
        for kw in &req.keywords {
            let ranges = find_matches(kw, pattern, ignore_case);
            if !ranges.is_empty() {
                matches.push(FieldMatch {
                    label: "keywords",
                    value: kw.clone(),
                    match_ranges: ranges,
                });
            }
        }
    }

    matches
}

fn print_matches(path_str: &str, field_matches: &[FieldMatch]) {
    println!("\n{}", style(path_str).bold().underlined());
    for fm in field_matches {
        let highlighted = highlight(&fm.value, &fm.match_ranges);
        println!("  {:<12} {}", style(fm.label).cyan(), highlighted);
    }
}

pub fn run(repo_root: &Path, args: SearchArgs) -> Result<(), Box<dyn Error>> {
    let set = RequirementSet::load_from_repo_root(repo_root)?;
    let fields = args.field.as_deref();
    let mut total_matches = 0usize;

    for (id, req_file) in &set.requirements {
        let req = &req_file.requirement;
        let field_matches = search_req(req, &args.pattern, args.ignore_case, fields);
        if field_matches.is_empty() {
            continue;
        }
        total_matches += field_matches.len();
        let path_str = set
            .files_by_id
            .get(id)
            .map(|p| p.strip_prefix(&set.repo_root).unwrap_or(p).display().to_string())
            .unwrap_or_else(|| id.to_string());
        print_matches(&path_str, &field_matches);
    }

    for (id, need_file) in &set.needs {
        let need = &need_file.need;
        let want = |name: &str| match fields {
            None => true,
            Some(fs) => fs.iter().any(|f| f.eq_ignore_ascii_case(name)),
        };
        let mut field_matches = Vec::new();
        for (label, value) in [("id", id.0.as_str()), ("title", need.title.as_str()), ("statement", need.statement.text.as_str())] {
            if want(label) {
                let ranges = find_matches(value, &args.pattern, args.ignore_case);
                if !ranges.is_empty() {
                    field_matches.push(FieldMatch { label, value: value.to_owned(), match_ranges: ranges });
                }
            }
        }
        if field_matches.is_empty() {
            continue;
        }
        total_matches += field_matches.len();
        let path_str = set
            .needs_by_id
            .get(id)
            .map(|p| p.strip_prefix(&set.repo_root).unwrap_or(p).display().to_string())
            .unwrap_or_else(|| id.to_string());
        print_matches(&path_str, &field_matches);
    }

    for (id, stk_file) in &set.stakeholders {
        let stk = &stk_file.stakeholder;
        let want = |name: &str| match fields {
            None => true,
            Some(fs) => fs.iter().any(|f| f.eq_ignore_ascii_case(name)),
        };
        let mut field_matches = Vec::new();
        for (label, value) in [("id", id.as_str()), ("name", stk.name.as_str())] {
            if want(label) {
                let ranges = find_matches(value, &args.pattern, args.ignore_case);
                if !ranges.is_empty() {
                    field_matches.push(FieldMatch { label, value: value.to_owned(), match_ranges: ranges });
                }
            }
        }
        if let Some(role) = &stk.role {
            if want("role") {
                let ranges = find_matches(role, &args.pattern, args.ignore_case);
                if !ranges.is_empty() {
                    field_matches.push(FieldMatch { label: "role", value: role.clone(), match_ranges: ranges });
                }
            }
        }
        if field_matches.is_empty() {
            continue;
        }
        total_matches += field_matches.len();
        let path_str = set
            .stakeholders_by_id
            .get(id)
            .map(|p| p.strip_prefix(&set.repo_root).unwrap_or(p).display().to_string())
            .unwrap_or_else(|| id.to_string());
        print_matches(&path_str, &field_matches);
    }

    if total_matches == 0 {
        eprintln!("  {} no matches for {:?}", style("·").dim(), args.pattern);
    } else {
        println!(
            "\n  {} {} match{} for {:?}",
            style("·").dim(),
            style(total_matches).bold(),
            if total_matches == 1 { "" } else { "es" },
            args.pattern,
        );
    }

    Ok(())
}
