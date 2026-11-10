---
name: rqtk-process
description: The systems engineering process for requirements in `.rqtk/`, from stakeholder needs through decomposition, interfaces and allocation to verification, with a review gate closing each step. Use when starting a system, feature or subsystem, when decomposing or allocating requirements, when deciding what comes next, before implementing a requirement that is still Draft, or when the user mentions a design review, a gate (requirements review, PDR, CDR, readiness review), a baseline or the lifecycle.
license: MIT OR Apache-2.0
compatibility: Requires the rqtk CLI and a repository set up with rqtk init.
---

# The systems engineering process

Work goes down the left side of the **V** and back up the right: each level is specified before the level below it, and verified after it. A **gate**, a design review run with `/design-review`, closes each step for a **scope**: the whole set, or one requirement and everything below it. Gates are passed scope by scope, so one subsystem can be at CDR while another is still at PDR.

| Step | What it produces | Closed by |
|---|---|---|
| 1. Stakeholders and needs | who needs what, and why: `rqtk add-stakeholder`, `rqtk add-need` | Requirements review |
| 2. System requirements | root-category requirements, each satisfying a need | Requirements review |
| 3. Decomposition | derived requirements, interface requirements, allocation to components | PDR |
| 4. Detailed design | buildable **leaves**: no TBD or TBR, allocated to modules, activities planned | CDR |
| 5. Implementation and verification | code, and tests linked with `verifies` whose results `rqtk verify` records | Readiness review |
| 6. Validation | every need satisfied by Verified requirements (`rqtk coverage`) | Readiness review |

The `rqtk-requirements` skill has the authoring rules for each item; the `rqtk-verification` skill has the proof loop for step 5.

## Decomposition

- A **leaf** is a requirement without children: code implements it and tests verify it. Every other requirement is covered by its children: together they demand everything it demands, and each child's rationale says which part it takes.
- Derive one level at a time: a child's category sits one `level` below its parent's in `.rqtk/config.toml`.
- Behaviour that crosses a boundary, between components or to another repository, becomes an interface requirement with an `[interface]` table, or a `consumes` link when another repository provides it.
- Allocate as the design firms up: `[allocation]` `components` or `subsystems` by PDR, `software_modules` or `source_files` for each leaf by CDR.
- A design decision goes into the rationale of the requirement it shapes, never into a separate decision record. Design documents and code are what a review reads; the requirements are the design's contract.
- A value not known yet is `tbd = true`, one not settled is `tbr = true`, with the open question in `notes`. Each is closed before CDR.

## Where a requirement stands

A requirement has passed a gate when a record in `.rqtk/reviews/` says `decision: passed` or `passed-with-actions` for a scope that holds it: `all`, the requirement itself, or one of its ancestors (`rqtk trace <ID> --json` lists them). Which gate is a requirement's own depends on where it sits: root-category requirements pass at the requirements review, other non-leaf requirements at PDR, leaves at CDR. When it passes its own gate, the requirement moves to the approved state (`Approved` unless the project's process says otherwise) once the user agrees.

## Before implementing

Read the briefing, `rqtk context <ID> --json`. When the requirement is not in the approved state, or no passed CDR record covers it, tell the user in one line (the ID, its state, the gate it is missing) and carry on unless they stop you. Building ahead of a review is the user's call; doing it silently is not. A project without `.rqtk/reviews/` holds no gates yet: there, warn only when the requirement is still in the default state, such as Draft.

## After a gate

A requirement that has passed its gate is under **change control**:

- Changing it is the user's decision. Propose the change with its reason, and name the gate it reopens: a changed leaf goes back through CDR, a changed system requirement through the requirements review and everything below it.
- `rqtk impact <baseline>` lists what a change reaches; the next review judges each item.
- `approval.approved_by` is a person's signature: leave it to people. State changes after a review are made only when the user agrees (see `/design-review`).

## Tailoring

When `.rqtk/reviews/PROCESS.md` exists, its gates, states and criteria replace the defaults here and in `/design-review`. Read it before anything in this skill that names a gate or a state.

## Done

A step is done when a passed record of its gate covers the scope. The process is done at a passed readiness review: `rqtk lint` exits 0, every requirement in scope is Verified, and every need they satisfy is satisfied.
