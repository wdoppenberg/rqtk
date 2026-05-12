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

[requirement.statement]
text = "The OBC software shall acquire telemetry data from all spacecraft subsystems at a minimum rate of 1 Hz."
rationale = "Continuous 1 Hz telemetry ensures ground operators have timely visibility into spacecraft health."

[requirement.status]
state = "Approved"
priority = "Critical"

[requirement.traceability]
# parents, depends_on, derived_from, supersedes — all just IDs

[requirement.verification]
method = "Test"
level = "System"
phase = "Pre-launch"
success_criteria = "All channels deliver frames at ≥1 Hz with no frame loss over 10 minutes."
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
rqtk coverage                      # identify requirements missing verification
rqtk baseline 1.0.0               # tag HEAD as rqtk/1.0.0
rqtk diff 0.9.0 1.0.0             # semantic diff between two baselines
rqtk log FOBC-SYS-0001            # git history for a single requirement
rqtk graph --format dot           # export traceability graph
rqtk export --format json|csv|markdown
rqtk rehash                        # recompute and write content hashes
rqtk report                        # generate PDF report via Typst
```

## Baselines and diffs

Baselines are annotated git tags under `refs/tags/rqtk/<version>`. They require no files, no databases, and no out-of-band state.

```bash
git add requirements/FOBC-SW-0001.toml
git commit -m "add telemetry requirement"
rqtk baseline 1.0.0

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
./scripts/install-hooks.sh
```

The hook runs `rqtk rehash` (updates content hashes) and `rqtk lint` before every commit. Schema changes to `schema/` are also checked.
