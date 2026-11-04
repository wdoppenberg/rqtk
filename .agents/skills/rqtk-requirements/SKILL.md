---
name: rqtk-requirements
description: Read and write the requirements, needs and stakeholders tracked in `.rqtk/` with the rqtk CLI. Use when a task changes behaviour a requirement governs, when the user mentions a requirement, need, stakeholder, a "shall" statement or an ID such as SYS-0001, or when editing files under `.rqtk/`.
license: MIT OR Apache-2.0
compatibility: Requires the rqtk CLI and a repository set up with rqtk init.
---

# Requirements with rqtk

This repo keeps its requirements as TOML under `.rqtk/`. **Stakeholders** raise **needs**; **requirements** satisfy needs and decompose into child requirements; each requirement is proven by **verification activities**. Every item has a stable ID and every link is by ID. The `rqtk` CLI reads and checks all of it; add `--json` to any command for machine-readable output.

## Before changing behaviour

Find the requirement that governs the behaviour and read its **briefing**:

1. `rqtk search <words> -i` to find candidates.
2. `rqtk context <ID> --json`: statement, rationale, ancestors and children, the needs it satisfies, the tests linked to each activity, verification status, and open lint findings.

The statement is the contract. When the task contradicts it, stop and say so: changing a requirement is the user's decision, and a reworded requirement makes its passing tests **Suspect** until they run again.

## Writing requirements

- Create items with `rqtk add`, `rqtk add-need`, `rqtk add-stakeholder` and `rqtk add-activity`. Run with `--dry-run` first to show the user what will be written. rqtk assigns IDs; use the ID it prints (`--json` has it as `id`).
- A statement is one sentence with the project's shall keyword and a measurable criterion. Split a compound requirement into several.
- Every requirement carries a `rationale` (why it exists), its parents and the needs it satisfies, success criteria, and one activity per independently testable behaviour. `rqtk add` sets all of it:

  ```bash
  rqtk add --category SW --type Functional --title "PID output clamp" \
    --statement "The PID controller shall clamp its output to 0.0–1.0." \
    --rationale "The heater duty cycle is a fraction." \
    --parent REQ-SYS-0001 --satisfies NEED-0001 \
    --criteria "Output stays within [0, 1] for errors of ±100 °C." \
    --activity "Clamp at both ends" --json
  ```

  Activity IDs are generated as `VA-<CATEGORY>-<NUMBER>-<NN>` (here `VA-SW-0001-01`). Tests cite them. Add one to an existing requirement with `rqtk add-activity <ID> --name "…"`.
- Edit the TOML directly for everything else; rqtk preserves comments and layout. Allowed fields: `rqtk schema requirement` (or `need`, `stakeholder`, `config`). Allowed values for category, type, state, priority and verification method/level/phase: `.rqtk/config.toml`.
- Leave `status` unset on activities that tests verify; their status comes from recorded evidence (see the `rqtk-verification` skill).

## Done

`rqtk lint` exits 0. For each finding, `rqtk explain <code>` states what the rule checks and how to fix it.
