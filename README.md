# rqtk

Requirements engineering that lives in your repository.

Requirements are TOML files, checked in alongside code. Baselines are git tags. History is `git log`. There is no separate tool, database, or export step standing between your requirements and your version control.

```
.rqtk/
  config.toml            ← project config and validation rules
  requirements/
    SYS/FOBC-SYS-0001.toml
    SW/FOBC-SW-0001.toml
  needs/NEED-0001.toml
  stakeholders/STK-001.toml
```

Each requirement is a single file:

```toml
id = "FOBC-SYS-0001"
title = "Telemetry Data Acquisition"
category = "SYS"
type = "Functional"
state = "Approved"
priority = "Critical"
criticality = "Mission-Critical"
keywords = ["telemetry", "health-monitoring"]
statement = "The OBC software shall acquire telemetry data from all spacecraft subsystems at a minimum rate of 1 Hz."
rationale = "Continuous 1 Hz telemetry ensures ground operators have timely visibility into spacecraft health."
assumptions = ["All subsystems expose a standardised telemetry interface as defined in the ICD."]
notes = "Applies to nominal mode only."

[trace]
satisfies = ["NEED-0001"]
# parents, derived_from, refines, conflicts_with, depends_on, related — all just IDs
# external = [{type = "JIRA", ref = "OBC-42"}]

[verification]
method = "Test"
level = "System"
phase = "Pre-launch"
owner = "Systems Verification Lead"
success_criteria = "All channels deliver frames at ≥1 Hz with no frame loss over 10 minutes."

[[verification.activities]]
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
rqtk init                          # scaffold .rqtk/ with config, requirements, needs, stakeholders
rqtk add --category SYS \
          --type Functional \
          --title "..." \
          --statement "..."        # create next requirement in sequence
rqtk add-need --title "..." --statement "..." --stakeholders STK-001
rqtk add-stakeholder --name "..." [--role "..."]
rqtk lint                          # validate all requirements against config
rqtk context FOBC-SW-0001         # everything about one item: links, tests, status, findings
rqtk impact main                   # what changed since a revision and what to re-verify
rqtk trace FOBC-SYS-0001          # show full traceability chain up and down
rqtk scan                          # list `verifies` links between tests and activities
rqtk verify --results junit.xml    # record test results as evidence in .rqtk/evidence.toml
rqtk coverage                      # need satisfaction and verification status (incl. Suspect)
rqtk coverage --strict             # same, but exit 1 on Gap, Failed or Suspect
rqtk baseline 1.0.0               # tag HEAD as rqtk/1.0.0
rqtk diff 0.9.0 1.0.0             # semantic diff between two baselines
rqtk log FOBC-SYS-0001            # git history for a single requirement
rqtk install-hook                  # install git pre-commit hook (rehash + lint)
rqtk open FOBC-SYS-0001           # open requirement in $EDITOR
rqtk search "telemetry"           # full-text search across requirement fields
rqtk search "telemetry" -i \
          --field title,statement  # case-insensitive search in specific fields
rqtk graph                         # traceability graph as Graphviz DOT
rqtk export --format json|csv|markdown
rqtk rehash                        # recompute and write content hashes (keeps comments)
rqtk report [-o report.md]         # Markdown requirements report (stdout by default)
rqtk schema requirement            # JSON Schema of a file kind (config, need, evidence, …)
rqtk explain RQ010                 # what a lint rule checks and how to fix it
```

Every command accepts `--json`. Every command that writes files (`init`, `add*`, `rehash`, `baseline`, `verify`) accepts `--dry-run`.

## Scripting and agents

rqtk is built to be driven by scripts and coding agents as well as people.

- **`--json`** prints one JSON document on stdout. Paths in it are relative to the repository root, and diagnostics carry `code`, `severity`, `subject` and `location.line`. With `--json`, errors are written to stderr as `{"error": {"kind": "usage" | "error", "message": …}}`.
- **Exit codes are fixed:**

  | Code | Meaning |
  |---|---|
  | 0 | success, nothing to report |
  | 1 | findings: lint errors, Failed/Suspect/Gap under `coverage --strict`, failed tests in `verify`, stale evidence under `verify --check` |
  | 2 | usage error: bad arguments, unknown category/type/rule/kind, or `--json` on a command that prints a document (`report`, `graph`, `open`) |
  | 3 | error: missing or invalid configuration, I/O or git failure |

- **Nothing prompts.** Every input is a flag, and `--dry-run` shows exactly what would be written. For `add*` that is the full TOML, or the item under `--json`.
- **`rqtk schema <kind>`** prints the JSON Schema for each file kind, and **`rqtk explain <code>`** gives a rule's meaning and fix. An agent can look both up instead of guessing.
- **`rqtk context <ID>`** is the briefing for working on one item:
  - for a requirement: its ancestors, children and other links, the needs it satisfies, the test functions linked to each activity, their verification status and evidence, and its lint findings;
  - for a need: its stakeholders and the requirements that satisfy it;
  - for a stakeholder: their needs.
- **`rqtk impact <rev>`** compares the working tree with a branch, tag, SHA or baseline. It lists:
  - requirements and needs that were added, removed or changed (semantic or cosmetic);
  - requirements downstream of a semantic change, via parents, depends_on, derived_from, refines or satisfies;
  - every activity to re-verify, because its requirement changed or a file containing its linked test changed.

  Use it in code review to check that a change matches the requirements it touches.

A typical loop for an agent implementing a requirement:

```bash
rqtk context FOBC-SW-0003 --json           # what is asked, what verifies it, what it's linked to
# write a failing test annotated with #[verifies("VA-SW-003-01")], implement, make it pass
cargo nextest run --profile ci
rqtk verify --results target/nextest/ci/junit.xml
rqtk lint && rqtk coverage --strict        # exit 0 means done
```

## Data model

The file format is versioned: `.rqtk/config.toml` declares `schema_version = 1`, and rqtk refuses to load any other version. JSON Schemas for every file kind live in [`schema/`](schema/) and are regenerated with `cargo run -p rqtk-core --bin generate_schemas`.

Unknown keys are errors, so a typo such as `ratoinale` is reported rather than silently dropped. Each file must be named after the ID it contains, and IDs are unique across requirements, needs and stakeholders.

A requirement file has scalar fields at the top level and these tables:

| Key / table | Purpose |
|---|---|
| top level | `id`, `title`, `category`, `type`, `state`, `priority`, `criticality`, `maturity`, `tbd`, `tbr`, `keywords`, `statement` (shall statement), `rationale`, `assumptions`, `notes`, `content_hash` |
| `[trace]` | Link fields below |
| `[verification]` | `method`, `level`, `phase`, `owner`, `success_criteria`, `[[verification.activities]]` |
| `[approval]` | `baselined_at`, `baselined_by`, `approved_by`, `ecr_ids` |
| `[[parameters]]` | Quantitative constraints: `name`, `operator`, `value`, `unit`, `tolerance` |
| `[validation]` | Stakeholder acceptance: `method`, `stakeholder`, `acceptance_criteria`, `status` |
| `[risk]` | `hazards`, `mitigations`, `fmea_ref`, `safety_critical`, `security_sensitive` |
| `[allocation]` | `subsystems`, `components`, `software_modules`, `source_files` |
| `[custom]` | Arbitrary project-specific key/value pairs |

Needs (`.rqtk/needs/`) have `id`, `title`, `state`, `priority`, `stakeholders`, `keywords`, `statement`, `rationale`, `content_hash` and an optional `[acceptance]` table. Stakeholders (`.rqtk/stakeholders/`) have `id`, `name`, `role`, `organization` and optional `[concerns]` and `[authority]` tables.

Dates may be written as native TOML dates (`executed_at = 2024-11-15`) or as strings.

### Traceability links

```toml
[trace]
parents        = ["FOBC-SYS-0001"]          # decomposed from
derived_from   = ["FOBC-SYS-0001"]          # derived from another requirement
satisfies      = ["NEED-0001"]              # satisfies a stakeholder need
refines        = ["FOBC-SYS-0001"]          # refines a higher-level requirement
conflicts_with = ["FOBC-SW-0003"]           # known conflict
depends_on     = ["FOBC-HW-0001"]           # runtime dependency
related        = ["FOBC-SW-0002"]           # informational link
external       = [{type = "JIRA", ref = "OBC-42"}]
```

### Verification activities

```toml
[[verification.activities]]
id             = "VA-SYS-001-01"
name           = "End-to-end telemetry acquisition test"
procedure      = "..."
expected_result = "..."
status         = "Passed"
executed_at    = 2024-11-15
evidence       = ["test-report-v1.pdf"]
```

`status`, `executed_at` and `evidence` are for activities performed by hand (inspection, analysis, demonstration). An activity linked to a test gets its status from `rqtk verify` instead; see [Verification flow](#verification-flow).

## Baselines and diffs

Baselines are annotated git tags under `refs/tags/rqtk/<version>`. They require no files, no databases, and no out-of-band state.

`rqtk baseline` automatically writes `baselined_at` and `baselined_by` into every requirement file (leaving the rest of each file, including comments, untouched), commits those changes, then creates the tag. No manual editing required.

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

Modifications are classified as **semantic** (statement, structural links, parameters, or verification method/level/phase changed) or **admin** (title, keywords, priority, etc.). A semantic change signals that downstream implementations may need re-verification.

## Verification flow

Three commands address verification:

| Command | Question |
|---|---|
| `rqtk lint` | Is every requirement file valid, and does every `verifies` annotation name a real activity? |
| `rqtk verify` | Which activities did this test run prove, and against which version of each requirement? |
| `rqtk coverage` | Is every requirement verified against its *current* content? |

`lint` catches form errors and sits in the pre-commit hook. `verify` and `coverage` belong in CI.

### Status comes from test results, not from the file

An activity that is linked to a test (see [Linking tests](#linking-tests-to-verification-activities)) gets its status from recorded test results. Its hand-written `status` field is ignored, and `lint` warns about it (RQ029). No one, human or agent, can mark a tested requirement verified by editing a file.

```bash
cargo nextest run --profile ci              # or: pytest --junitxml=junit.xml, go-junit-report, …
rqtk verify --results target/nextest/ci/junit.xml
git add .rqtk/evidence.toml
```

`rqtk verify` reads JUnit XML and matches each test case to the functions annotated with `verifies`. It records one entry per activity in `.rqtk/evidence.toml`: the outcome, the tests that produced it, the commit, and the **content hash of the requirement at that moment**. An activity passes only when every test linked to it ran and passed. If a run covers only some of an activity's tests, for example Python tests without the Rust ones, that activity's existing evidence is left unchanged. `verify` exits 1 if a linked test failed. `verify --check` writes nothing and exits 1 if the committed evidence is out of date.

Without nextest, stable Rust can emit JUnit directly:

```bash
RUSTC_BOOTSTRAP=1 cargo test --workspace --tests -- -Z unstable-options --format junit > junit.xml
```

Activities that are not linked to any test, such as inspections, analyses and demonstrations, keep using their hand-written `status` (`"Passed"`, `"Waived"`, `"Failed"`, …).

### Coverage states

`rqtk coverage` puts each requirement into one of these states:

- **Verified**: every activity passed, meaning linked tests passed against the requirement's current content, or a manual status is `Passed`/`Waived`.
- **Suspect**: tests passed, but the requirement's statement, links, parameters or verification method changed afterwards. Re-run the tests and `rqtk verify`.
- **Failed**: a linked test failed, or a manual status is `Failed`.
- **In Progress**: some activities passed or started, but not all.
- **Planned**: activities and success criteria are defined, but nothing has been executed.
- **Gap**: no activities are defined, or success criteria are missing.

```bash
rqtk coverage
#
#   Verified 8  ·  Suspect 1  ·  Failed 0  ·  In Progress 2  ·  Planned 2  ·  Gap 1  (14 total)
#
#   Suspect  (changed since its tests passed; run the tests and `rqtk verify`)
#      ·  FOBC-SW-0002  (VA-SW-002-01 suspect)
#
#   Gap  (no activities or success criteria defined)
#      ·  FOBC-HW-0003

rqtk coverage --strict   # exits 1 on any Gap, Failed or Suspect requirement, or unsatisfied need
```

## Change control

Requirement changes that slip through without review are harder to detect than code changes, because there is no compiler to catch a modified `shall` statement. Two platform features close this gap without any additional tooling.

**CODEOWNERS** maps requirement directories to the engineers responsible for approving changes to them. A PR touching `.rqtk/requirements/SYS/` cannot merge until the cognizant systems engineer has reviewed it.

```
# CODEOWNERS
.rqtk/requirements/SYS/    @systems-lead
.rqtk/requirements/SW/     @software-lead
.rqtk/requirements/HW/     @hardware-lead
```

**Signed commits** (enforced via branch protection) bind a committer's cryptographic identity to every change. Combined with CODEOWNERS, every semantic change to a requirement is both reviewed by the right person and signed by a verified identity — the git log becomes an auditable change record.

On the rqtk side, the `content_hash` field (maintained by `rqtk rehash` and checked by lint rule RQ021) detects whether any semantic field was modified outside the normal commit flow. The `[approval]` section records the formal outcome in the file itself:

```toml
[approval]
baselined_at  = 2024-11-01
baselined_by  = "systems-lead"
approved_by   = ["systems-lead", "chief-engineer"]
ecr_ids       = ["ECR-0042"]
```

The authoritative change record is the git log: who signed the commit, who approved the PR. The `[approval]` fields are a human-readable summary inside the file for anyone reading the TOML directly — useful, but secondary to the platform record.

## Validation

Rules are defined in `.rqtk/config.toml` — allowed categories, lifecycle states, verification methods, traceability constraints, forbidden keywords, and more. `rqtk lint` enforces them on every run and is designed to sit in a pre-commit hook.

```bash
rqtk lint
#   Location                                    ID            Severity  Code   Message
#   .rqtk/requirements/SW/FOBC-SW-0001.toml:8   -             error     RQ100  unknown field `ratoinale`, expected one of …
#   .rqtk/requirements/SW/FOBC-SW-0003.toml:11  FOBC-SW-0003  error     RQ015  unknown parents reference `FOBC-SYS-9999`
#   .rqtk/requirements/HW/FOBC-HW-0001.toml:9   FOBC-HW-0001  warning   RQ021  content hash is stale …
#
#   2 errors  ·  1 warning  (12 checked)
```

Every broken file is reported in one run, with its line number. The full list of rule codes is in [`crates/rqtk-core/src/rules.rs`](crates/rqtk-core/src/rules.rs); codes are stable and never reused.

## Linking tests to verification activities

Every requirement can declare verification activities with IDs like `VA-SYS-001-01`. Tests declare which activity they verify, and `rqtk scan` lists every link it finds:

```bash
rqtk scan
#   VA-SYS-001-01
#      · crates/obc/tests/telemetry.rs:12  telemetry_acquisition_rate
#
#   14 links to 9 activities  ·  3 activities not linked to tests
```

The scanner walks the repository (respecting `.gitignore`) and recognises Rust attributes (`#[verifies("…")]`), Python decorators (`@verifies("…")`), and, in any language, a comment tag placed directly above the test:

```go
// rqtk: verifies VA-SYS-001-01
func TestTelemetryRate(t *testing.T) { … }
```

Configure what is scanned in `.rqtk/config.toml`:

```toml
[scan]
paths = ["."]                         # default
exclude = ["tests/fixtures/**"]       # gitignore-style globs
```

`rqtk lint` reports annotations that name an unknown activity (RQ028) and annotations that aren't followed by a function (RQ030).

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

The macro resolves the activity ID at compile time by walking up from `CARGO_MANIFEST_DIR` to find `.rqtk/config.toml`. If the activity does not exist in any requirement file, the build fails with an error pointing to the annotation. When it does exist, the macro injects the requirement context as rustdoc on the function — visible in IDE hover and `cargo doc` — and registers the requirement file as a build input, so editing it triggers a rebuild.

### Python

Install the Python extension (`pip install rqtk`) and use the `verifies` decorator:

```python
from rqtk import verifies

@verifies("VA-SYS-001-01")
def test_telemetry_acquisition_rate():
    ...
```

The decorator resolves the activity ID at import time from the `.rqtk/config.toml` found by walking up from the current working directory. An unknown ID raises `ValueError` immediately, failing the test collection step before any test runs. Known IDs attach the requirement context to `__doc__` on the function.

## Pre-commit hook

```bash
rqtk install-hook
```

Writes a `pre-commit` hook to `.git/hooks/` that runs `rqtk rehash` (updates content hashes) and `rqtk lint` before every commit. Pass `--force` to overwrite an existing hook.
