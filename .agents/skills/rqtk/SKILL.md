---
name: rqtk
description: "Show where the requirements in `.rqtk/` stand in the systems engineering process, scope by scope, and recommend the next skill to run. Run only when the user asks for it by name."
disable-model-invocation: true
license: MIT OR Apache-2.0
compatibility: Requires the rqtk CLI.
---

`/rqtk` answers "where are we, and what's next?" for the requirements in this repository.

- **Reads:** `.rqtk/`, lint, coverage, the review records and the baselines.
- **Writes:** nothing.
- **Leaves to you:** which next step to take.

Call the Skill tool with "rqtk-process" first: its step table is the map this skill places each scope on.

## Steps

### 1. Collect the facts

When there is no `.rqtk/config.toml`, the answer is `/rqtk-init`; skip to the hand-over. Otherwise collect, and copy without second-guessing:

- `rqtk lint --json` and `rqtk coverage --json`;
- the frontmatter of every record in `.rqtk/reviews/` (gate, scope, decision, open actions), and `.rqtk/reviews/PROCESS.md` if it exists;
- `rqtk baseline --list --json`;
- the categories and their `layer` and `level` in `.rqtk/config.toml`.

Done when each of these is read or known to be absent.

### 2. Place each scope

The scopes are the whole set, and each system requirement (each root-category requirement without layers) with everything below it. Place each on the first step of the `rqtk-process` table it has not finished:

| Not finished when | Step |
|---|---|
| a stakeholder raises no need, or a need has no stakeholder | 1. Stakeholders and needs |
| a need has no user requirement (RQ044, RQ045, unsatisfied needs) | 2. User requirements |
| a user requirement has no system requirement under it (RQ046) | 3. System requirements |
| no passed `requirements` record covers it | requirements review |
| its system requirement has no children and no category below it | 4. Architecture, then 5 |
| no passed `pdr` record covers it | PDR |
| a leaf has TBD or TBR, no allocation, or no activity | 6. Detailed design |
| no passed `cdr` record covers it | CDR |
| a requirement is not Verified | 7. Implementation and verification |
| no passed `readiness` record covers it, or one predates a change since the last baseline | readiness review |

Note the open actions of records that cover the scope. Done when every scope has a step.

## Hand over

A table: scope, step, what holds it there (IDs and counts), next skill or command. Then one recommendation: the next step for the scope furthest behind, or the one the user has been working on, with the command to run it. Name the project's lint errors first when there are any; they come before everything else.
