---
name: rqtk-process
description: The systems engineering process for requirements in `.rqtk/`, from stakeholder needs and user and system requirements through architecture, decomposition, interfaces and allocation to verification, with a review gate closing each step. Use when starting a system, feature or subsystem, when decomposing or allocating requirements, when deciding what comes next, before implementing a requirement that is still Draft, or when the user mentions a design review, a gate (requirements review, PDR, CDR, readiness review), a baseline or the lifecycle.
license: MIT OR Apache-2.0
compatibility: Requires the rqtk CLI and a repository set up with rqtk init.
---

# The systems engineering process

Work goes down the left side of the **V** and back up the right: each level is specified before the level below it, and verified after it. A **gate**, a design review run with `/rqtk-design-review`, closes each step for a **scope**: the whole set, or one requirement and everything below it. Gates are passed scope by scope, so one subsystem can be at CDR while another is still at PDR.

| Step | What it produces | Skill | Closed by |
|---|---|---|---|
| 1. Stakeholders and needs | who needs what, and why | `/rqtk-init`, or `/rqtk-to-requirements` from a spec | Requirements review |
| 2. User requirements | what users must be able to do with the system, each satisfying a need | as step 1 | Requirements review |
| 3. System requirements | what the system must do or be, seen from its boundary, each serving a user requirement | as step 1 | Requirements review |
| 4. Architecture | the design: components and how they interact | any design method or skill | (read at PDR) |
| 5. Decomposition | derived requirements per component, interface requirements, allocation | `/rqtk-decompose` | PDR |
| 6. Detailed design | buildable **leaves**: no TBD or TBR, allocated to modules, activities planned | `/rqtk-decompose` | CDR |
| 7. Implementation and verification | code, and tests linked with `verifies` whose results `rqtk verify` records | `rqtk-verification`; `/rqtk-diff-review` per change | Readiness review |
| 8. Validation | every need satisfied by Verified requirements (`rqtk coverage`) | | Readiness review |

`/rqtk-design-review <gate>` holds each gate, and `/rqtk` tells the user where each scope stands and which skill comes next.

Steps 1 to 3 treat the system as a **black box**: they say what, never how, and come before any design. The **top requirements** are the user and system requirements: the categories with `layer = "user"` and `layer = "system"` in `.rqtk/config.toml`. A project without layers has one top layer, the root category, whose requirements satisfy needs directly.

The `rqtk-requirements` skill has the authoring rules for each item; the `rqtk-verification` skill has the proof loop for step 7.

## Working with the user

Every skill the user runs works the same way:

- **Rounds.** Questions come in rounds: every question whose prerequisites are settled, numbered, each with your recommended answer, worded so that "yes" accepts it. Ask a later question only after the answers it depends on. Use your question tool where you have one.
- **Facts are yours, decisions are theirs.** Look up what rqtk, the repository and the code can tell you. Put every choice to the user.
- **Draft, confirm, write.** Show what you will write, with `--dry-run` where the user wants to see the file, and write only what they confirm.
- **Hand over** in the same shape: what changed, with IDs; the open questions (`tbd`, `tbr`, anything unanswered); and the next skill or gate, from the table above.

These stay with people. Propose them, with the command, and act only when the user agrees:

- changing a requirement that has passed its gate (see "After a gate");
- a review's decision, and the state changes that follow it;
- `approval.approved_by`, a person's signature, which you never fill;
- `rqtk review <ID>`, which confirms that a Suspect requirement still holds;
- `rqtk baseline <version>`, which stamps every requirement and tags the commit;
- the design decisions themselves.

## Architecture

The design starts once the requirements review has passed for the system requirements it answers. Use whatever method the project uses; where a design skill such as `codebase-design` is available, it can do this step. The design reads the system requirements and the design ideas kept in their `notes`. Its decisions reach `.rqtk/` as the categories below the system layer (one per subsystem or component, a `level` below `SYS`) and as the rationales of the derived requirements in step 5.

## Decomposition

- A **leaf** is a requirement without children: code implements it and tests verify it. Every other requirement is covered by its children: together they demand everything it demands, and each child's rationale says which part it takes.
- Derive one level at a time: a child's category sits one `level` below its parent's in `.rqtk/config.toml`. System requirements derive from user requirements; the categories below follow the architecture.
- Behaviour that crosses a boundary, between components or to another repository, becomes an interface requirement with an `[interface]` table, or a `consumes` link when another repository provides it.
- Allocate as the design firms up: `[allocation]` `components` or `subsystems` by PDR, `software_modules` or `source_files` for each leaf by CDR.
- A design decision goes into the rationale of the requirement it shapes, never into a separate decision record. Design documents and code are what a review reads; the requirements are the design's contract.
- A value not known yet is `tbd = true`, one not settled is `tbr = true`, with the open question in `notes`. Each is closed before CDR.

## Where a requirement stands

A requirement has passed a gate when a record in `.rqtk/reviews/` says `decision: passed` or `passed-with-actions` for a scope that holds it: `all`, the requirement itself, or one of its ancestors (`rqtk trace <ID> --json` lists them). Which gate is a requirement's own depends on where it sits: top requirements pass at the requirements review, other non-leaf requirements at PDR, other leaves at CDR. When it passes its own gate, the requirement moves to the approved state (`Approved` unless the project's process says otherwise) once the user agrees.

## Before implementing

Read the briefing, `rqtk context <ID> --json`. When the requirement is not in the approved state, or no passed CDR record covers it, tell the user in one line (the ID, its state, the gate it is missing) and carry on unless they stop you. Building ahead of a review is the user's call; doing it silently is not. A project without `.rqtk/reviews/` holds no gates yet: there, warn only when the requirement is still in the default state, such as Draft.

## After a gate

A requirement that has passed its gate is under **change control**:

- Changing it is the user's decision. Propose the change with its reason, and name the gate it reopens: a changed leaf goes back through CDR, a changed user or system requirement through the requirements review and everything below it.
- `rqtk impact <baseline>` lists what a change reaches; the next review judges each item.

## Tailoring

When `.rqtk/reviews/PROCESS.md` exists, its gates, states and criteria replace the defaults here and in `/rqtk-design-review`. Read it before anything in this skill that names a gate or a state.

## Done

A step is done when a passed record of its gate covers the scope. The process is done at a passed readiness review: `rqtk lint` exits 0, every requirement in scope is Verified, and every need they satisfy is satisfied.
