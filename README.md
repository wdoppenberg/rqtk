# rqtk

Requirements engineering that lives in your repository.

Requirements are TOML files, checked in alongside code. Baselines are git tags. History is `git log`. There is no separate tool, database, or export step standing between your requirements and your version control.

```
rqtk.toml          ← project config and validation rules
requirements/
  FOBC-SYS-0001.toml
  FOBC-SW-0001.toml
  ...
```

Each requirement is a single file:

```toml
[requirement]
id = "FOBC-SYS-0001"
title = "Telemetry Data Acquisition"
category = "SYS"
type = "Functional"
keywords = ["telemetry", "health-monitoring"]

[requirement.statement]
text = "The OBC software shall acquire telemetry data from all spacecraft subsystems at a minimum rate of 1 Hz."
rationale = "Continuous 1 Hz telemetry ensures ground operators have timely visibility into spacecraft health."
assumptions = ["All subsystems expose a standardised telemetry interface as defined in the ICD."]
notes = "Applies to nominal mode only."

[requirement.status]
state = "Approved"
priority = "Critical"
criticality = "Mission-Critical"

[requirement.traceability]
# parents, derived_from, satisfies, refines, conflicts_with, depends_on, related — all just IDs
# external = [{type = "JIRA", ref = "OBC-42"}]

[requirement.verification]
method = "Test"
level = "System"
phase = "Pre-launch"
owner = "Systems Verification Lead"
success_criteria = "All channels deliver frames at ≥1 Hz with no frame loss over 10 minutes."

[[requirement.verification.activities]]
id = "VA-SYS-001-01"
name = "End-to-end telemetry acquisition test"
procedure = "Activate all subsystem simulators and record frame timestamps for 10 minutes."
expected_result = "Frame arrival rate ≥1 Hz on every channel; zero dropped frames."
status = "Passed"
```

## Why

Most requirements tools are separate systems — documents, spreadsheets, or SaaS platforms disconnected from source control. Engineers end up maintaining two ledgers: one in the tool, one in the code. They diverge.

`rqtk` treats requirements as source. A requirement change is a commit. A baseline is a tag. A delta between baselines is a diff. Code review covers both the implementation and the requirement that drove it.

This fits the workflow developers already use. It also makes requirements auditable, branchable, and mergeable without learning anything new.

## Install

```bash
cargo install rqtk
```

## Commands

```bash
rqtk init                          # scaffold rqtk.toml and requirements/
rqtk add --category SYS \
          --type Functional \
          --title "..." \
          --statement "..."        # create next requirement in sequence
rqtk lint                          # validate all requirements against config
rqtk trace FOBC-SYS-0001          # show full traceability chain up and down
rqtk coverage                      # report verification status (gap/planned/in-progress/verified)
rqtk coverage --strict             # same, but fail unless every requirement is verified
rqtk baseline 1.0.0               # tag HEAD as rqtk/1.0.0
rqtk diff 0.9.0 1.0.0             # semantic diff between two baselines
rqtk log FOBC-SYS-0001            # git history for a single requirement
rqtk install-hook                  # install git pre-commit hook (rehash + lint)
rqtk open FOBC-SYS-0001           # open requirement in $EDITOR
rqtk search "telemetry"           # full-text search across requirement fields
rqtk search "telemetry" -i \
          --field title,statement  # case-insensitive search in specific fields
rqtk graph --format dot           # export traceability graph
rqtk export --format json|csv|markdown
rqtk rehash                        # recompute and write content hashes
rqtk report                        # generate PDF report via Typst
```

## Data model

A requirement file has these top-level sections:

| Section | Purpose |
|---|---|
| `[requirement]` | Identity: `id`, `title`, `category`, `type`, `keywords`, `content_hash` |
| `[requirement.statement]` | `text` (shall statement), `rationale`, `assumptions`, `notes` |
| `[requirement.status]` | `state`, `priority`, `criticality`, `maturity`, `tbd`, `tbr` |
| `[requirement.approval]` | `baselined_at`, `baselined_by`, `approved_by`, `ecr_ids` |
| `[requirement.parameters]` | Quantitative constraints: `name`, `operator`, `value`, `unit`, `tolerance` |
| `[requirement.traceability]` | Link fields below |
| `[requirement.verification]` | `method`, `level`, `phase`, `owner`, `success_criteria`, activities |
| `[requirement.validation]` | Stakeholder acceptance: `method`, `stakeholder`, `acceptance_criteria`, `status` |
| `[requirement.risk]` | `hazards`, `mitigations`, `fmea_ref`, `safety_critical`, `security_sensitive` |
| `[requirement.allocation]` | `subsystems`, `components`, `software_modules`, `source_files` |
| `[requirement.custom]` | Arbitrary project-specific key/value pairs |

### Traceability links

```toml
[requirement.traceability]
parents        = ["FOBC-SYS-0001"]          # decomposed from
derived_from   = ["FOBC-SYS-0001"]          # derived from another requirement
satisfies      = ["STAKE-001"]              # satisfies a stakeholder need (string)
refines        = ["FOBC-SYS-0001"]          # refines a higher-level requirement
conflicts_with = ["FOBC-SW-0003"]           # known conflict
depends_on     = ["FOBC-HW-0001"]           # runtime dependency
related        = ["FOBC-SW-0002"]           # informational link
external       = [{type = "JIRA", ref = "OBC-42"}]
```

### Verification activities

```toml
[[requirement.verification.activities]]
id             = "VA-SYS-001-01"
name           = "End-to-end telemetry acquisition test"
procedure      = "..."
expected_result = "..."
status         = "Passed"
executed_at    = 2024-11-15
evidence       = ["test-report-v1.pdf"]
```

## Baselines and diffs

Baselines are annotated git tags under `refs/tags/rqtk/<version>`. They require no files, no databases, and no out-of-band state.

`rqtk baseline` automatically writes `baselined_at` and `baselined_by` into every requirement file, commits those changes, then creates the tag. No manual editing required.

```bash
rqtk baseline 1.0.0
#   ✓  Baseline created
#         tag                  rqtk/1.0.0
#         stamped              14
#         by                   Wouter Doppenberg

# later
rqtk diff 0.9.0 1.0.0
#  Diff  0.9.0 → 1.0.0  (1 added, 0 removed, 2 modified)
#
#  +  Added
#     ·  FOBC-SW-0001
#
#  ~  Modified
#     ·  FOBC-SYS-0003  [semantic]
#     ·  FOBC-HW-0001   [admin]
```

Modifications are classified as **semantic** (statement, traceability, or verification method changed) or **admin** (title, tags, priority, etc.). A semantic change signals that downstream implementations may need re-verification.

## Verification flow

Two commands address verification:

| Command | Question |
|---|---|
| `rqtk lint` | Is every requirement file structurally valid? |
| `rqtk coverage` | Have the verification activities actually been planned and completed? |

`lint` catches form errors and sits in the pre-commit hook. `coverage` is for CI and milestone reviews.

`rqtk coverage` classifies each requirement into one of four states:

- **Verified** — all activities have `status = "Passed"` or `"Waived"` and an `executed_at` date
- **In Progress** — at least one activity started, but not all terminal
- **Planned** — activities and success criteria defined, none executed yet
- **Gap** — no activities defined, or success criteria missing

The full status breakdown is always printed. Pass `--strict` to exit with code 1 if any requirement has no activities or success criteria defined (a **Gap**), making it suitable as a CI gate:

```bash
rqtk coverage
#
#   Verified 8  ·  In Progress 2  ·  Planned 3  ·  Gap 1  (14 total)
#
#   Gap  (no activities or success criteria defined)
#      ·  FOBC-HW-0003
#
#   Planned  (activities defined, none executed)
#      ·  FOBC-SW-0004
#      ·  FOBC-SW-0005
#      ·  FOBC-HW-0002
#
#   In Progress  (partially executed, not all terminal)
#      ·  FOBC-SYS-0002
#      ·  FOBC-SW-0001

rqtk coverage --strict   # exits 1 if any Gap is present
```

## Change control

Requirement changes that slip through without review are harder to detect than code changes, because there is no compiler to catch a modified `shall` statement. Two platform features close this gap without any additional tooling.

**CODEOWNERS** maps requirement directories to the engineers responsible for approving changes to them. A PR touching `requirements/SYS/` cannot merge until the cognizant systems engineer has reviewed it.

```
# CODEOWNERS
requirements/SYS/    @systems-lead
requirements/SW/     @software-lead
requirements/HW/     @hardware-lead
```

**Signed commits** (enforced via branch protection) bind a committer's cryptographic identity to every change. Combined with CODEOWNERS, every semantic change to a requirement is both reviewed by the right person and signed by a verified identity — the git log becomes an auditable change record.

On the rqtk side, the `content_hash` field (maintained by `rqtk rehash` and checked by lint rule RQ021) detects whether any semantic field was modified outside the normal commit flow. The `[requirement.approval]` section records the formal outcome in the file itself:

```toml
[requirement.approval]
baselined_at  = 2024-11-01
baselined_by  = "systems-lead"
approved_by   = ["systems-lead", "chief-engineer"]
ecr_ids       = ["ECR-0042"]
```

The authoritative change record is the git log: who signed the commit, who approved the PR. The `[requirement.approval]` fields are a human-readable summary inside the file for anyone reading the TOML directly — useful, but secondary to the platform record.

## Validation

Rules are defined in `rqtk.toml` — allowed categories, lifecycle states, verification methods, traceability constraints, forbidden keywords, and more. `rqtk lint` enforces them on every run and is designed to sit in a pre-commit hook.

```bash
rqtk lint
#   ID            Severity  Code   Message
#   FOBC-SW-0002  error     RQ007  missing rationale
#   FOBC-HW-0001  warning   RQ021  content hash is stale — run `rqtk rehash`
#
#   1 error  ·  1 warning  (12 checked)
```

## Linking tests to verification activities

Every requirement can declare verification activities with IDs like `VA-SYS-001-01`. `rqtk` can enforce at build or test time that a function actually exists to cover each activity.

### Rust

Add `rqtk-macros` as a dev-dependency and annotate test functions:

```rust
use rqtk_macros::verifies;

#[verifies("VA-SYS-001-01")]
#[test]
fn telemetry_acquisition_rate() {
    // ...
}
```

The macro resolves the activity ID at compile time by walking up from `CARGO_MANIFEST_DIR` to find `rqtk.toml`. If the activity does not exist in any requirement file, the build fails with an error pointing to the annotation. When it does exist, the macro injects the requirement context as rustdoc on the function — visible in IDE hover and `cargo doc`.

### Python

Install the Python extension (`pip install rqtk`) and use the `verifies` decorator:

```python
from rqtk import verifies

@verifies("VA-SYS-001-01")
def test_telemetry_acquisition_rate():
    ...
```

The decorator resolves the activity ID at import time from the `rqtk.toml` found by walking up from the current working directory. An unknown ID raises `ValueError` immediately, failing the test collection step before any test runs. Known IDs attach the requirement context to `__doc__` on the function.

## Pre-commit hook

```bash
rqtk install-hook
```

Writes a `pre-commit` hook to `.git/hooks/` that runs `rqtk rehash` (updates content hashes) and `rqtk lint` before every commit. Pass `--force` to overwrite an existing hook.
