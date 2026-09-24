//! Agent skills shipped inside the binary, so installed skills always match the CLI that
//! installed them.

use std::{collections::BTreeMap, error::Error, path::Path, path::PathBuf};

use console::style;
use serde::Serialize;

use crate::cli::output::{self, Ctx, Exit, Usage};

struct Embedded {
    /// Path relative to the skills directory, e.g. `rqtk-requirements/SKILL.md`.
    path: &'static str,
    content: &'static str,
}

macro_rules! embed {
    ($($path:literal),* $(,)?) => {
        &[$(Embedded { path: $path, content: include_str!(concat!("../../../skills/", $path)) }),*]
    };
}

const FILES: &[Embedded] = embed![
    "rqtk-requirements/SKILL.md",
    "rqtk-requirements/agents/openai.yaml",
    "rqtk-verification/SKILL.md",
    "rqtk-verification/agents/openai.yaml",
    "to-requirements/SKILL.md",
    "to-requirements/agents/openai.yaml",
    "requirements-review/SKILL.md",
    "requirements-review/agents/openai.yaml",
];

/// Records, per installed file, the hash of the content rqtk wrote. A file whose content still
/// matches is untouched and may be upgraded; anything else is a local edit and is kept.
const MANIFEST: &str = ".rqtk-skills.json";

fn sha256(text: &str) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

fn read_manifest(dir: &Path) -> BTreeMap<String, String> {
    std::fs::read_to_string(dir.join(MANIFEST))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

const BLOCK_BEGIN: &str = "<!-- BEGIN rqtk (managed by `rqtk skills install`) -->";
const BLOCK_END: &str = "<!-- END rqtk -->";
const BLOCK_BODY: &str = "\
## Requirements (rqtk)

Requirements, stakeholder needs and verification evidence live in `.rqtk/`, managed with the \
`rqtk` CLI. Before changing behaviour, read the governing requirement with `rqtk context <ID>`. \
Work is done when `rqtk lint` and `rqtk coverage --strict` both exit 0. Skills: \
`rqtk-requirements`, `rqtk-verification`, `/to-requirements`, `/requirements-review`.";

/// A skill's name, description and who can invoke it, read from its frontmatter.
#[derive(Debug, Serialize)]
pub struct SkillInfo {
    pub name: String,
    pub description: String,
    /// `true` when only the user can invoke it (`disable-model-invocation: true`).
    pub user_invoked: bool,
}

pub fn skills() -> Vec<SkillInfo> {
    FILES
        .iter()
        .filter(|f| f.path.ends_with("/SKILL.md"))
        .map(|f| parse_frontmatter(f.content))
        .collect()
}

fn parse_frontmatter(content: &str) -> SkillInfo {
    let front = content
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---").map(|(f, _)| f))
        .unwrap_or_default();
    let field = |key: &str| {
        front
            .lines()
            .find_map(|l| l.strip_prefix(key)?.strip_prefix(':'))
            .map(|v| v.trim().trim_matches('"').to_owned())
            .unwrap_or_default()
    };
    SkillInfo {
        name: field("name"),
        description: field("description"),
        user_invoked: field("disable-model-invocation") == "true",
    }
}

/// Where a family of agents reads project skills from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum AgentTarget {
    /// `.agents/skills`: the shared location read by Codex, Cursor, GitHub Copilot, Gemini
    /// CLI, OpenCode, Amp, Cline, Zed, Warp and others.
    Universal,
    /// `.claude/skills`: Claude Code.
    Claude,
}

impl AgentTarget {
    fn dir(self) -> &'static str {
        match self {
            AgentTarget::Universal => ".agents/skills",
            AgentTarget::Claude => ".claude/skills",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Action {
    Created,
    Updated,
    Unchanged,
    /// A symlink to the shared copy in `.agents/skills` was created.
    Linked,
    /// The file differs from the shipped version and `--force` was not given.
    SkippedModified,
}

#[derive(Serialize)]
struct Written {
    path: PathBuf,
    action: Action,
    /// For symlinks: where the link points, relative to the link's directory.
    #[serde(skip_serializing_if = "Option::is_none")]
    link_target: Option<PathBuf>,
    /// For instruction files: the rqtk block was new to an existing file.
    #[serde(skip)]
    block_added: bool,
}

#[derive(Serialize)]
pub struct Report {
    /// Skill directories installed into, relative to the repo root.
    dirs: Vec<PathBuf>,
    files: Vec<Written>,
    /// Instruction files that received the rqtk block. Empty when none exists or was named.
    instructions: Vec<Written>,
    dry_run: bool,
}

pub struct InstallArgs {
    /// Agent families to install for; ignored when `dir` is set.
    pub targets: Vec<AgentTarget>,
    /// Install one copy into this directory instead.
    pub dir: Option<PathBuf>,
    /// Copy into `.claude/skills` instead of symlinking to `.agents/skills`.
    pub copy: bool,
    pub instructions: Vec<PathBuf>,
    pub force: bool,
    pub dry_run: bool,
}

impl Default for InstallArgs {
    fn default() -> Self {
        Self {
            targets: vec![AgentTarget::Universal, AgentTarget::Claude],
            dir: None,
            copy: false,
            instructions: Vec::new(),
            force: false,
            dry_run: false,
        }
    }
}

pub fn list(ctx: &Ctx) -> Result<Exit, Box<dyn Error>> {
    let skills = skills();
    if ctx.json() {
        output::json(&skills)?;
        return Ok(Exit::Ok);
    }
    for s in &skills {
        let who = if s.user_invoked { "user " } else { "model" };
        let name = if s.user_invoked {
            format!("/{}", s.name)
        } else {
            s.name.clone()
        };
        println!(
            "  {}  {:<22} {}",
            style(who).dim(),
            style(name).bold(),
            s.description
        );
    }
    Ok(Exit::Ok)
}

pub fn install(ctx: &Ctx, args: &InstallArgs) -> Result<Exit, Box<dyn Error>> {
    let report = execute(ctx, args)?;
    if ctx.json() {
        output::json(&report)?;
    } else {
        print(&report);
    }
    Ok(Exit::Ok)
}

pub fn execute(ctx: &Ctx, args: &InstallArgs) -> Result<Report, Box<dyn Error>> {
    let root = &ctx.root;
    // Resolve (and validate) the instruction files before writing anything.
    let instruction_files = instruction_targets(root, &args.instructions)?;

    let mut dirs = Vec::new();
    let mut files = Vec::new();
    match &args.dir {
        Some(dir) => {
            copy_skills(root, dir, None, args, &mut files)?;
            dirs.push(dir.clone());
        }
        None => {
            let universal = args.targets.contains(&AgentTarget::Universal);
            for target in [AgentTarget::Universal, AgentTarget::Claude] {
                if !args.targets.contains(&target) {
                    continue;
                }
                let dir = PathBuf::from(target.dir());
                // Claude Code links to the shared copy, so every agent reads the same files.
                let link_to = (target == AgentTarget::Claude && universal && !args.copy)
                    .then(|| PathBuf::from(AgentTarget::Universal.dir()));
                copy_skills(root, &dir, link_to.as_deref(), args, &mut files)?;
                dirs.push(dir);
            }
        }
    }

    let instructions = instruction_files
        .into_iter()
        .map(|path| install_instructions(root, path, args.dry_run))
        .collect::<Result<_, _>>()?;
    Ok(Report {
        dirs,
        files,
        instructions,
        dry_run: args.dry_run,
    })
}

/// Install every bundled skill into `dir`. With `link_to`, each skill directory becomes a
/// symlink to the same skill under `link_to`, unless something other than that link is
/// already there (an earlier copy, or local edits), which is then treated as a copy.
fn copy_skills(
    root: &Path,
    dir: &Path,
    link_to: Option<&Path>,
    args: &InstallArgs,
    out: &mut Vec<Written>,
) -> Result<(), Box<dyn Error>> {
    let skill_names: Vec<&str> = FILES
        .iter()
        .filter_map(|f| f.path.strip_suffix("/SKILL.md"))
        .collect();
    let mut manifest = read_manifest(&root.join(dir));
    for name in skill_names {
        let skill_dir = root.join(dir).join(name);
        if let Some(shared) = link_to
            && cfg!(unix)
        {
            // A relative link resolves from the directory containing it (`dir`): climb out
            // of `dir` to the root, then down into the shared copy.
            let ups = dir.components().count();
            let target: PathBuf = std::iter::repeat_n(Path::new(".."), ups)
                .collect::<PathBuf>()
                .join(shared)
                .join(name);
            let existing = std::fs::read_link(&skill_dir).ok();
            let action = match existing {
                Some(current) if current == target => Some(Action::Unchanged),
                Some(_) => None, // a different link: leave it to the copy path below
                None if skill_dir.exists() => None,
                None => Some(Action::Linked),
            };
            if let Some(action) = action {
                if action == Action::Linked && !args.dry_run {
                    std::fs::create_dir_all(root.join(dir))?;
                    symlink_dir(&target, &skill_dir)?;
                }
                out.push(Written {
                    path: output::relative(&skill_dir, root),
                    action,
                    link_target: Some(target),
                    block_added: false,
                });
                continue;
            }
        }
        for file in FILES
            .iter()
            .filter(|f| f.path.starts_with(&format!("{name}/")))
        {
            let path = root.join(dir).join(file.path);
            let action = match std::fs::read_to_string(&path) {
                Err(_) => Action::Created,
                Ok(existing) if existing == file.content => Action::Unchanged,
                // Still exactly what an earlier rqtk wrote: safe to upgrade.
                Ok(existing) if manifest.get(file.path) == Some(&sha256(&existing)) => {
                    Action::Updated
                }
                Ok(_) if args.force => Action::Updated,
                Ok(_) => Action::SkippedModified,
            };
            if action != Action::SkippedModified {
                manifest.insert(file.path.to_owned(), sha256(file.content));
            }
            if !args.dry_run && matches!(action, Action::Created | Action::Updated) {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&path, file.content)?;
            }
            out.push(Written {
                path: output::relative(&path, root),
                action,
                link_target: None,
                block_added: false,
            });
        }
    }
    if !args.dry_run && manifest != read_manifest(&root.join(dir)) {
        std::fs::write(
            root.join(dir).join(MANIFEST),
            serde_json::to_string_pretty(&manifest)? + "\n",
        )?;
    }
    Ok(())
}

#[cfg(unix)]
fn symlink_dir(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(not(unix))]
fn symlink_dir(_target: &Path, _link: &Path) -> std::io::Result<()> {
    unreachable!("links are only created on unix")
}

pub fn print(report: &Report) {
    let verb = if report.dry_run { "(dry run)" } else { "" };
    for f in &report.files {
        let label = match f.action {
            Action::Created => style(format!("create {verb}")).green(),
            Action::Updated => style(format!("update {verb}")).yellow(),
            Action::Linked => style(format!("link {verb}")).green(),
            Action::Unchanged => style("unchanged".to_owned()).dim(),
            Action::SkippedModified => style("kept (edited locally)".to_owned()).red(),
        };
        match &f.link_target {
            Some(target) => println!(
                "  {:<24} {} -> {}",
                label,
                f.path.display(),
                target.display()
            ),
            None => println!("  {:<24} {}", label, f.path.display()),
        }
    }
    if report.instructions.is_empty() {
        println!(
            "\n  No AGENTS.md or CLAUDE.md found. Point your agents at the skills with \
             `rqtk skills install --instructions AGENTS.md`."
        );
    }
    for i in &report.instructions {
        let action = match i.action {
            Action::Created => "Created",
            Action::Updated if i.block_added => "Added",
            Action::Updated => "Updated",
            _ => "Unchanged",
        };
        println!("\n  {action} rqtk block in {} {verb}", i.path.display());
    }
    if report
        .files
        .iter()
        .any(|f| f.action == Action::SkippedModified)
    {
        println!("  Skills edited locally were kept; pass --force to replace them.");
    }
}

/// Instruction files to put the rqtk block in. Named files win. Otherwise: `AGENTS.md` if it
/// exists (read by most agents), plus `CLAUDE.md` if it exists and does not already import
/// `@AGENTS.md`. Nothing is created unless named.
fn instruction_targets(root: &Path, named: &[PathBuf]) -> Result<Vec<PathBuf>, Usage> {
    if !named.is_empty() {
        return named
            .iter()
            .map(|path| {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default();
                if matches!(name, "CLAUDE.md" | "AGENTS.md") {
                    Ok(root.join(path))
                } else {
                    Err(Usage(format!(
                        "--instructions must name a CLAUDE.md or AGENTS.md file, not `{}`",
                        path.display()
                    )))
                }
            })
            .collect();
    }
    let agents = root.join("AGENTS.md");
    let claude = root.join("CLAUDE.md");
    let mut targets = Vec::new();
    let has_agents = agents.is_file();
    if has_agents {
        targets.push(agents);
    }
    if claude.is_file() {
        let imports_agents = std::fs::read_to_string(&claude)
            .is_ok_and(|text| text.lines().any(|l| l.trim() == "@AGENTS.md"));
        if !(has_agents && imports_agents) {
            targets.push(claude);
        }
    }
    Ok(targets)
}

/// Add or refresh the rqtk block in `target`, creating the file if needed.
fn install_instructions(
    root: &Path,
    target: PathBuf,
    dry_run: bool,
) -> Result<Written, Box<dyn Error>> {
    let existing = std::fs::read_to_string(&target).ok();
    let updated = super::splice_managed(
        existing.as_deref().unwrap_or(""),
        BLOCK_BEGIN,
        BLOCK_END,
        BLOCK_BODY,
    );
    let action = match &existing {
        None => Action::Created,
        Some(text) if *text == updated => Action::Unchanged,
        Some(_) => Action::Updated,
    };
    if !dry_run && action != Action::Unchanged {
        std::fs::write(&target, updated)?;
    }
    let block_added = existing
        .as_deref()
        .is_some_and(|text| !text.contains(BLOCK_BEGIN));
    Ok(Written {
        path: output::relative(&target, root),
        action,
        link_target: None,
        block_added,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_skill_has_matching_name_and_description() {
        for file in FILES.iter().filter(|f| f.path.ends_with("/SKILL.md")) {
            let dir = file.path.split('/').next().unwrap();
            let info = parse_frontmatter(file.content);
            // https://agentskills.io/specification
            assert_eq!(info.name, dir, "{}", file.path);
            let valid_name = !info.name.is_empty()
                && info.name.len() <= 64
                && info.name.split('-').all(|part| {
                    !part.is_empty()
                        && part
                            .chars()
                            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
                });
            assert!(valid_name, "invalid skill name `{}`", info.name);
            assert!(!info.description.is_empty(), "{}", file.path);
            assert!(info.description.chars().count() <= 1024, "{}", file.path);
            let front = file.content.split("\n---").next().unwrap();
            let compatibility = front
                .lines()
                .find_map(|l| l.strip_prefix("compatibility:"))
                .unwrap_or_else(|| panic!("{} lacks `compatibility`", file.path));
            assert!(compatibility.trim().chars().count() <= 500);
            if info.user_invoked {
                // Agents other than Claude Code ignore `disable-model-invocation`.
                assert!(
                    info.description.contains("only when the user asks"),
                    "{} needs the invocation guard in its description",
                    file.path
                );
            }
        }
    }

    #[test]
    fn invocation_is_consistent_across_harnesses() {
        for skill in skills() {
            let yaml = FILES
                .iter()
                .find(|f| f.path == format!("{}/agents/openai.yaml", skill.name))
                .unwrap_or_else(|| panic!("{} has no agents/openai.yaml", skill.name));
            let implicit_off = yaml.content.contains("allow_implicit_invocation: false");
            assert_eq!(skill.user_invoked, implicit_off, "{}", skill.name);
        }
    }
}
