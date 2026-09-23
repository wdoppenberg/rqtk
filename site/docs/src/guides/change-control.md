# Baselines and change control

Requirement changes that slip through without review are harder to catch than code changes: there is no compiler to notice a modified shall statement. rqtk leans on git for the record and adds what git can't tell you.

## Baselines

A baseline is an annotated git tag, `rqtk/<version>`:

```bash
rqtk baseline 1.0.0 --dry-run   # show what would be stamped and tagged
rqtk baseline 1.0.0
```

It writes `approval.baselined_at` and `approval.baselined_by` into every requirement (leaving the rest of each file untouched), commits that, and tags the commit.

## What changed

```bash
rqtk diff 0.9.0 1.0.0      # between baselines, branches, tags or SHAs
rqtk impact main           # between a revision and your working tree
rqtk log SYS-0001          # git history of one requirement
```

`diff` classifies each modified requirement as **semantic** (its content hash changed: statement, links, parameters or verification) or **cosmetic** (anything else).

`impact` goes further. For a branch or pull request it lists:

- requirements and needs added, removed or changed;
- requirements **downstream** of a semantic change, which may need their own review;
- every verification activity to **re-run**, because its requirement changed or a file containing one of its tests changed.

## Review and sign-off

Two platform features make the git log an auditable change record:

**CODEOWNERS** routes changes to the right reviewer:

```
.rqtk/requirements/SYS/    @systems-lead
.rqtk/requirements/SW/     @software-lead
```

**Signed commits**, enforced by branch protection, tie every change to a verified identity.

The `[approval]` table in each requirement is a readable summary for anyone looking at the TOML. The authoritative record is the git log: who signed the commit, and who approved the pull request.

```toml
[approval]
baselined_at = 2026-11-01
baselined_by = "systems-lead"
approved_by  = ["systems-lead", "chief-engineer"]
ecr_ids      = ["ECR-0042"]
```

## Pre-commit hook

```bash
rqtk install-hook
```

This adds a managed block to `.git/hooks/pre-commit` that runs `rqtk rehash` (refresh content hashes) and `rqtk lint` before each commit. Existing hook content is kept.
