# Coding agents

rqtk is built to be driven by coding agents as well as people: nothing prompts, every command prints JSON with `--json`, and exit codes are fixed. Six agent skills bring it into an agent's normal workflow.

## Skills

```bash
rqtk skills install      # or, in a new repository: rqtk init --agents
```

| Skill | Invoked by | What it does |
|---|---|---|
| `rqtk-requirements` | agent or you | Reading and writing `.rqtk/`: find the governing requirement, read its briefing, author items that lint clean. |
| `rqtk-verification` | agent or you | The proof loop: link a test, run it, `rqtk verify`; done when `rqtk lint` and `rqtk coverage --strict` exit 0. |
| `rqtk-process` | agent or you | The systems engineering process: needs, system requirements, decomposition, interfaces, allocation, verification, and the review gate that closes each step. Warns before an agent implements a requirement that hasn't passed its gate. |
| `/to-requirements` | you | Turn a spec or discussion into needs and requirements with verification activities; you approve the draft before anything is written. |
| `/requirements-review` | you | Review a diff against the requirements it touches: facts from `rqtk impact`, then a verdict per requirement, and the `rqtk review` commands that would settle what is still Suspect. |
| `/design-review` | you | Hold a review gate (requirements review, PDR, CDR, readiness) over the whole set or one requirement's subtree, and write the record to `.rqtk/reviews/`; the decision, state changes and baseline stay yours. |

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

## Design reviews

Around that flow sit the gates of a systems engineering process, one per step down the V and one to close it:

| Gate | Reviews | Approves |
|---|---|---|
| `requirements` | needs and system requirements | root-category requirements |
| `pdr` (preliminary design) | the decomposition: derived and interface requirements, allocation to components | other non-leaf requirements |
| `cdr` (critical design) | the buildable leaves: no TBD or TBR, allocated to modules, verification planned | leaves |
| `readiness` | the proof: everything Verified, every change since the last baseline judged | nothing; it closes the scope |

`/design-review pdr SYS-0003` reviews one requirement and everything below it, so parts of a system can pass gates at different times. The agent collects the facts from rqtk, judges each criterion, and writes `.rqtk/reviews/<date>-<gate>-<ID>.md` with the findings. You give the decision; the agent then proposes the state changes (Draft → Approved) and the `rqtk baseline` command, and applies only what you agree to. It never fills `approval.approved_by`.

A project tailors the gates, its states and their criteria in `.rqtk/reviews/PROCESS.md`; ask `/design-review` to write a starter. The `rqtk-process` skill reads the records to tell where a requirement stands, and warns before implementing one whose gate hasn't passed.

## Commands agents use most

```bash
rqtk context REQ-SYS-0001 --json # the briefing: statement, links, tests, status, findings
rqtk impact main --json          # what a change touches, what to re-verify, and with which tests
rqtk lint --json                 # findings with code, severity and file:line
rqtk explain RQ010               # what a rule checks and how to fix it
rqtk schema requirement          # the JSON Schema of a file kind
rqtk add … --parent … --criteria … --activity …   # a complete requirement in one command
```

Exit codes and the JSON contract are described under [JSON output and exit codes](../reference/json-and-exit-codes.md).

## Machine-readable docs

These docs are also available in plain Markdown for agents: [`/llms.txt`](/llms.txt) indexes every page, and [`/llms-full.txt`](/llms-full.txt) contains them all in one file.
