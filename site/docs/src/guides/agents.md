# Coding agents

rqtk is built to be driven by coding agents as well as people: nothing prompts, every command prints JSON with `--json`, and exit codes are fixed. Four agent skills bring it into an agent's normal workflow.

## Skills

```bash
rqtk skills install      # or, in a new repository: rqtk init --agents
```

| Skill | Invoked by | What it does |
|---|---|---|
| `rqtk-requirements` | agent or you | Reading and writing `.rqtk/`: find the governing requirement, read its briefing, author items that lint clean. |
| `rqtk-verification` | agent or you | The proof loop: link a test, run it, `rqtk verify`; done when `rqtk lint` and `rqtk coverage --strict` exit 0. |
| `/to-requirements` | you | Turn a spec or discussion into needs and requirements with verification activities; you approve the draft before anything is written. |
| `/requirements-review` | you | Review a diff against the requirements it touches: facts from `rqtk impact`, then a verdict per requirement. |

The skills follow the open [Agent Skills](https://agentskills.io) format, so they work with any agent that supports it:

| Agents | Read skills from | What rqtk installs |
|---|---|---|
| Codex, Cursor, GitHub Copilot, Gemini CLI, OpenCode, Amp, Cline, Zed, Warp, … | `.agents/skills/` | the skill files |
| Claude Code | `.claude/skills/` | a link per skill into `.agents/skills/` |

`rqtk skills install --for claude` or `--for universal` installs for one family only; `--copy` writes copies instead of links; `--dir` installs anywhere else. The skills are compiled into the binary, so they always describe the rqtk you have. Reinstall after upgrading: untouched skills are updated, and skills you edited are kept.

rqtk also adds a short block to `AGENTS.md` (and to `CLAUDE.md`, unless it already imports `@AGENTS.md`) telling agents where requirements live and when work is done. It never creates those files unless you name one with `--instructions`.

## Where the skills fit

The skills are small and slot into an existing flow rather than imposing one. They were designed alongside [mattpocock/skills](https://github.com/mattpocock/skills):

```
grill → /to-spec → /to-requirements → /to-tickets → /implement → /code-review + /requirements-review
```

- `/to-requirements` turns the spec into requirements with activity IDs.
- Tickets cite those IDs in their acceptance criteria.
- The implementer's tests cite the activity IDs, and `rqtk verify` records them.
- `/requirements-review` checks the result on the requirements axis, next to the usual code review.

## Commands agents use most

```bash
rqtk context SYS-0001 --json     # the briefing: statement, links, tests, status, findings
rqtk impact main --json          # what a change touches and what to re-verify
rqtk lint --json                 # findings with code, severity and file:line
rqtk explain RQ010               # what a rule checks and how to fix it
rqtk schema requirement          # the JSON Schema of a file kind
```

Exit codes and the JSON contract are described under [JSON output and exit codes](../reference/json-and-exit-codes.md).

## Machine-readable docs

These docs are also available in plain Markdown for agents: [`/llms.txt`](/llms.txt) indexes every page, and [`/llms-full.txt`](/llms-full.txt) contains them all in one file.
