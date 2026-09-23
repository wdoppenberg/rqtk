---
name: requirements-review
description: "Review the changes since a fixed point against the requirements they touch: what changed, what needs re-verifying, and whether the code does what each statement demands. Run only when the user asks for it by name."
disable-model-invocation: true
license: MIT OR Apache-2.0
compatibility: Requires the rqtk CLI and a repository set up with rqtk init.
---

Review the diff between `HEAD` (plus uncommitted work) and a fixed point on the **requirements axis**: does the change do what the requirements demand, and is it proven? This complements a standards and spec review such as `/code-review`; run them separately and report them separately.

## Process

### 1. Pin the fixed point

Use the commit, branch, tag or baseline the user gave; ask if they gave none. Confirm it resolves with `git rev-parse <fixed-point>`.

### 2. Collect the facts

These are deterministic; don't second-guess them:

- `rqtk impact <fixed-point> --json`: requirements and needs added, removed or changed; downstream requirements; activities to re-verify and why.
- `rqtk lint --json`: findings.
- `rqtk coverage --json`: status per requirement and activity.

### 3. Judge each touched requirement

For every requirement that `impact` lists as changed, downstream, or owning an activity to re-verify, read `rqtk context <ID> --json` and the relevant hunks of `git diff <fixed-point>`. Decide:

- **Implemented?** Does the code do what the statement says, all of it and nothing contrary? Quote the statement.
- **Proven?** Do the linked tests check the statement's criterion through the public interface, or do they pass by construction? Is the evidence current, or Suspect, Failed, or not run?
- **Downstream:** does a changed parent or need invalidate a child's statement?

### 4. Find untraced changes

List behaviour changes in the diff that no requirement covers: new or altered behaviour whose code isn't exercised by any linked test. Each one is either missing a requirement or out of scope for the change.

## Report

Two sections, in this order, under 400 words:

- **Facts**: lint errors, Failed, Suspect and not-run activities, activities to re-verify. Copy them from the rqtk output.
- **Judgement**: per requirement, one of implemented / partial / wrong / untested, each with the quoted statement and the hunk; then the untraced changes.

End with one line: the number of findings and the worst one.
