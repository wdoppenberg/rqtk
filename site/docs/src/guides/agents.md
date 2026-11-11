# Coding agents

rqtk is built to be driven by coding agents as well as people: nothing prompts, every command prints JSON with `--json`, and exit codes are fixed. Nine agent skills bring it into an agent's normal workflow.

## Skills

```bash
rqtk skills install      # or, in a new repository: rqtk init --agents
```

| Skill | Invoked by | What it does |
|---|---|---|
| `rqtk-requirements` | agent or you | Reading and writing `.rqtk/`: find the governing requirement, read its briefing, author items that lint clean. |
| `rqtk-verification` | agent or you | The proof loop: link a test, run it, `rqtk verify`; done when `rqtk lint` and `rqtk coverage --strict` exit 0. |
| `rqtk-process` | agent or you | The systems engineering process: needs, user and system requirements, architecture, decomposition, interfaces, allocation, verification, and the review gate that closes each step. Warns before an agent implements a requirement that hasn't passed its gate. |
| `/rqtk` | you | Where things stand: places each scope on a step of the process from lint, coverage and the review records, and recommends the next skill. Writes nothing. |
| `/rqtk-init` | you | Set up `.rqtk/` by interviewing you about the project's stakeholders, boundary, needs, user requirements and system requirements, and on request their verification. It stops before the design, which is yours to make. In an existing project the agent reads the code first and puts its findings to you as questions; nothing is written until you confirm it. |
| `/rqtk-to-requirements` | you | Turn a spec or discussion into needs, user and system requirements with verification activities; you approve the draft before anything is written. |
| `/rqtk-decompose` | you | After you've made the design: turn it into the requirements one level below a given requirement, a category per component, with interfaces and allocation, ending at the PDR. |
| `/rqtk-diff-review` | you | Review a diff against the requirements it touches: facts from `rqtk impact`, then a verdict per requirement, and the `rqtk review` commands that would settle what is still Suspect. |
| `/rqtk-design-review` | you | Hold a review gate (requirements review, PDR, CDR, readiness) over the whole set or one requirement's subtree, and write the record to `.rqtk/reviews/`; the decision, state changes and baseline stay yours. |

The skills follow the open [Agent Skills](https://agentskills.io) format, so they work with any agent that supports it:

| Agents | Read skills from | What rqtk installs |
|---|---|---|
| Codex, Cursor, GitHub Copilot, Gemini CLI, OpenCode, Amp, Cline, Zed, Warp, … | `.agents/skills/` | the skill files |
| Claude Code | `.claude/skills/` | a link per skill into `.agents/skills/` |

`rqtk skills install --for claude` or `--for universal` installs for one family only; `--copy` writes copies instead of links; `--dir` installs anywhere else. The skills are compiled into the binary, so they always describe the rqtk you have. Reinstall after upgrading: untouched skills are updated, and skills you edited are kept.

rqtk also adds a short block to `AGENTS.md` (and to `CLAUDE.md`, unless it already imports `@AGENTS.md`) telling agents where requirements live and when work is done. It never creates those files unless you name one with `--instructions`.

## Starting out

In a repository without `.rqtk/`, install the skills (`rqtk skills install` needs no `.rqtk/`) and run `/rqtk-init`. The agent grills you in rounds, one stage at a time, each question with a recommended answer. It runs `rqtk init`, then writes each stage with rqtk once you confirm it. Questions you can't answer yet are recorded as TBD or TBR rather than guessed. It ends before any design: next come `/rqtk-design-review requirements`, then the architecture with whatever design method you use, then the decomposition into subsystem requirements that follows it.

## Where the skills fit

The skills slot into an existing flow rather than imposing one, and were designed alongside [mattpocock/skills](https://github.com/mattpocock/skills). rqtk keeps the contract (what must hold, where it is proven, which gate approved it); the other skills shape the work:

```
                 /rqtk  ── where each scope stands, and what's next ──
  what (black box)                                         proof
  ────────────────                                         ─────
  /rqtk-init              grill → /to-spec                 /rqtk-design-review readiness
  (a whole project)       → /rqtk-to-requirements                     ▲
     needs, user and system requirements                  /to-tickets → /implement (TDD)
              ▼                                           tests cite activity IDs; rqtk verify
  /rqtk-design-review requirements                        /code-review + /rqtk-diff-review
              ▼                                                       ▲
  how: the architecture, with any design method                       │
  (e.g. codebase-design)                                              │
              ▼                                                       │
  /rqtk-decompose <ID>  → /rqtk-design-review pdr                     │
  /rqtk-decompose ...   → /rqtk-design-review cdr  ───────────────────┘
```

- `/rqtk-to-requirements` turns the spec into requirements with activity IDs.
- The design is yours: `/rqtk-decompose` turns it into the requirements of each part.
- Tickets cite the requirement and activity IDs in their acceptance criteria; the implementer's tests cite the activity IDs, and `rqtk verify` records them.
- `/rqtk-diff-review` checks each change on the requirements axis, next to the usual code review.

Every skill you run works the same way: it says what it reads, writes and leaves to you, asks in rounds with a recommended answer, and hands over what changed, what is open and which skill comes next.

## Design reviews

Around that flow sit the gates of a systems engineering process, one per step down the V and one to close it:

| Gate | Reviews | Approves |
|---|---|---|
| `requirements` | needs, user requirements and system requirements: the black box | user and system requirements |
| `pdr` (preliminary design) | the decomposition: derived and interface requirements, allocation to components | other non-leaf requirements |
| `cdr` (critical design) | the buildable leaves: no TBD or TBR, allocated to modules, verification planned | leaves |
| `readiness` | the proof: everything Verified, every change since the last baseline judged | nothing; it closes the scope |

`/rqtk-design-review pdr SYS-0003` reviews one requirement and everything below it, so parts of a system can pass gates at different times. The agent collects the facts from rqtk, judges each criterion, and writes `.rqtk/reviews/<date>-<gate>-<ID>.md` with the findings. You give the decision; the agent then proposes the state changes (Draft → Approved) and the `rqtk baseline` command, and applies only what you agree to. It never fills `approval.approved_by`.

A project tailors the gates, its states and their criteria in `.rqtk/reviews/PROCESS.md`; ask `/rqtk-design-review` to write a starter. The `rqtk-process` skill reads the records to tell where a requirement stands, and warns before implementing one whose gate hasn't passed.

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
