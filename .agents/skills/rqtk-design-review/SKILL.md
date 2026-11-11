---
name: rqtk-design-review
description: "Run a design review gate (requirements review, PDR, CDR or readiness review) over the requirements in `.rqtk/`, for the whole set or one requirement and everything below it, and record the outcome in `.rqtk/reviews/`. Run only when the user asks for it by name."
disable-model-invocation: true
license: MIT OR Apache-2.0
compatibility: Requires the rqtk CLI and a repository set up with rqtk init.
---

`/rqtk-design-review <gate> [ID]` checks a **scope** against one **gate**'s criteria and writes a **record**. The facts come from rqtk, the judgement is yours, and the decision is the user's.

- **Reads:** the scope's items, what rqtk says about them, earlier records, and the design documents and code the requirements point to.
- **Writes:** the record in `.rqtk/reviews/`, then the state changes and edits the user agrees to.
- **Leaves to you:** the decision, the reviewers and action owners, every state change and edit, and the baseline.

Call the Skill tool with "rqtk-process" first: it describes the process the gates belong to, and the way of working with the user.

The gates are `requirements`, `pdr`, `cdr` and `readiness`, with their criteria in [gates.md](gates.md). When `.rqtk/reviews/PROCESS.md` exists, its gates, states and criteria replace or extend those: follow it. If the user asks for one, write it from [process-template.md](process-template.md).

## Steps

### 1. Pin the gate and the scope

Take the gate the user named; for an unknown one, list the gates and ask. The scope is `all`, or the ID the user gave together with its descendants from `rqtk trace <ID> --json`. Either way it includes the needs the scope's requirements satisfy and those needs' stakeholders. Done when you can list every item in scope.

### 2. Check entry

- The gate before this one has a passed record for this scope, an ancestor of it, or `all`. If not, tell the user which is missing and ask whether to review anyway. A review held out of order says so in its record.
- List the actions of earlier records that cover this scope, closed or still open.
- Find the last baseline with `rqtk baseline --list --json`.

Done when the entry is met or the user has agreed to review out of order.

### 3. Collect the facts

These are deterministic; copy them, don't second-guess them. Keep only what falls in scope.

- `rqtk lint --json`: findings.
- `rqtk coverage --json`: status per requirement and need, and why each Suspect one is Suspect.
- `rqtk export --format json --output <file>` into a temporary file: every field, among them `state`, `tbd`, `tbr`, `allocation`, `risk`, `approval` and `interface`.
- `rqtk impact <last baseline> --json`, when there is a baseline: what changed since.
- `rqtk context <ID> --json` for each item you judge.

Done when each has run.

### 4. Judge every criterion

Take the gate's criteria in order and give each a verdict, **met**, **not met** or **n/a**, citing IDs. A criterion is met only when it holds for every item in scope; every exception is a finding. Where a criterion asks, read the design documents and code the requirements point to: allocated modules and files, interface specs. Done when every criterion has a verdict and every one not met has at least one finding.

### 5. Write the findings

Each **finding** has a number (`F-1`, `F-2`, …), a severity, the item's ID, the criterion, what is wrong (quote the text), and a proposed action. **Major** findings block the gate; **minor** ones can be closed after it. A finding that would change a requirement proposes the edit without making it. Done when every exception from step 4 is a finding.

### 6. Write the record

Write `.rqtk/reviews/<YYYY-MM-DD>-<gate>[-<ID>].md` in the format below, with `decision: open`. Present the criteria table and the findings, and recommend a decision: `passed` with no findings, `passed-with-actions` with minor ones only, `failed` with any major one. Done when the record is written and the user has your recommendation.

### 7. Close

The decision is the user's. When they give it:

1. Set `decision` and `reviewers` in the record, and list the actions with the owners the user names.
2. On `passed` or `passed-with-actions`, list the state changes the gate allows: the scope's requirements whose own gate this is (see `rqtk-process`), into the approved state. Apply the ones the user agrees to. `approval.approved_by` is left to people.
3. Apply the requirement edits from findings that the user agrees to, following the `rqtk-requirements` skill. A reworded requirement turns Suspect.
4. Give the baseline command for the user to run on a clean, committed tree: `rqtk baseline <version>`. It stamps every requirement and creates a git tag, so the version and the run are the user's. Once it has run, set `baseline` in the record.

Done when the record has a decision and every proposed state change and edit is applied or declined, as the record's Changes section says.

## Hand over

- **Changed:** the record's path, the decision, and the state changes and edits applied.
- **Open:** the actions with their owners, and the baseline command if it hasn't run.
- **Next:** on a pass, the next step of the scope from the `rqtk-process` table: after the requirements review, the design and then `/rqtk-decompose`; after PDR, `/rqtk-decompose` down to the leaves; after CDR, implementation; after readiness, nothing. On a failure, the major findings to close before holding the gate again.

## Record format

```markdown
---
gate: pdr                  # requirements | pdr | cdr | readiness, or a gate from PROCESS.md
scope: SYS-0003            # an ID, or all
date: 2026-11-10
commit: 1a2b3c4            # git rev-parse --short HEAD when the facts were collected
decision: open             # open | passed | passed-with-actions | failed
reviewers: []              # the people the user names
baseline:                  # the version, once `rqtk baseline` has run
---

# PDR: <scope's title>

## Scope
The IDs reviewed: requirements, needs, stakeholders.

## Entry
The previous gate's record, and the actions carried in from earlier records.

## Criteria
| # | Criterion | Verdict | Items |
|---|---|---|---|

## Findings
### F-1 (major) <ID>: <one line>
The criterion, the quoted text, the proposed action.

## Decision
The user's decision and its conditions; the actions, each with an owner.

## Changes
State changes and edits applied, and those declined.
```
