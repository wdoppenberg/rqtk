# Changelog

All notable changes to the `rqtk` command line. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); the project follows
[semantic versioning](https://semver.org) as described under *Stability* in the README.

## [1.2.0] — 2026-09-24

Field trials on six stacks (Python, Rust, TypeScript on Bun, Go, Java on Maven, C++ with
GoogleTest) found cases where rqtk reported a requirement verified when it wasn't. This release
fixes them; some checks are stricter as a result.

### Upgrade notes
- **`coverage --strict` requires every requirement to be Verified.** It used to pass Planned
  and In Progress requirements. Add `--allow planned` (and `--allow in-progress`) to accept
  requirements written ahead of their implementation.
- **RQ030 is an error:** a `verifies` tag with no test declaration below it can never be
  verified. Only comments and attributes may sit between a tag and its test now.
- **`verify` exits 1** when the results match no linked test, or a linked test matches several
  distinct tests.
- **Suspect lasts longer.** A requirement stays Suspect after it changes, even when the same
  unchanged tests pass again, and after a parent requirement or need changes; see *Review debt*
  below. Run your tests and `rqtk verify` once after upgrading and commit
  `.rqtk/evidence.toml`, so the evidence records test source hashes and upstream hashes.
- **Evidence written by 1.2 can't be read by 1.1** (it has new fields). Upgrade everyone who
  runs `rqtk verify` together. From 1.2 on, unknown fields in the evidence file are ignored.
- **Rust:** `#[requirements_docs]` moved to the `requirements-docs` feature;
  `features = ["macros"]` now brings only `#[verifies]`.

### Fixed: verdicts
- Test declarations in Java, Kotlin, C#, C and C++ (including GoogleTest `TEST`/`TEST_F`/
  `TEST_P` and Catch2 `TEST_CASE`) are recognised; before, nothing in those languages could be
  verified.
- A tag binds only to the test directly below it. It used to bind to the next test it
  recognised within 12 lines, so `it.each(…)` gave its activity to an unrelated test.
- JS/TS `.only`, `.skip`, `.concurrent`, `.each(…)` and `describe` groups; Go methods; Python
  `async def`; Kotlin backtick names.
- Results are matched by the file and line the runner reports where available, then by what
  the link's path says about the classname. A name that still matches several distinct tests
  is reported as ambiguous instead of all of them deciding the outcome.
- CTest and GoogleTest disabled tests (`status="disabled"`, `status="notrun"`) count as
  skipped, not passed.
- `verify` no longer says "Evidence is up to date" when nothing matched.

### Added
- **Review debt.** Evidence records a hash of each activity's test source and of everything
  upstream of the requirement. A requirement re-verified by unchanged tests after it changed,
  or whose parent or need changed, is Suspect until its tests change or someone records
  `rqtk review <ID> [--note …]`. `coverage`, `context` and `report` say why a requirement is
  Suspect (`suspect_reasons` in JSON).
- **Table-driven and parameterised tests:** `// rqtk: verifies VA-… case "name"` on a table
  row, or `verifies("VA-…", case = "…")`, links one case.
- **`rqtk add` writes complete requirements:** `--parent`, `--satisfies`, `--criteria`,
  `--activity` (IDs `VA-<CATEGORY>-<NUMBER>-<NN>`), `--priority`, `--method`, `--level`,
  `--phase`. It refuses a category that requires a parent when none is given.
- `rqtk add-activity <ID> --name …`, and `add-need --rationale`.
- `coverage --strict --allow planned|in-progress`.
- `rqtk report` opens with what needs attention, shows the tests and commit (or date and files)
  behind every activity, and adds a stakeholders table and a need → requirement → activity →
  evidence matrix.
- `impact` names the tests to run for each activity and separates activities whose evidence
  already covers the change (`tests`, `done`).
- Lint rule RQ031: an activity's evidence file doesn't exist.
- `activity_states` in verification JSON: activity states as objects.
- `init` names the project after `Cargo.toml`, `pyproject.toml`, `package.json` or `go.mod`, and
  warns outside a git repository.

### Changed
- `init` creates the example stakeholder and need only with `--example`.
- New requirements get priority Medium (or the middle configured level), not the first level.
- New requirement and need files carry no `content_hash`, so editing them leaves no stale-hash
  warning. `rehash` refreshes stored hashes only; `rehash --all` stamps every file.
- `validation.require_parent_for_categories` is the documented name of the parent rule;
  `require_parent_for_levels` still works.
- `rqtk = { default-features = false, features = ["macros"] }` builds 16 crates instead of about
  270: the macros look activities up with a TOML parser, and the `rqtk` crate's dependencies
  hang off its `lib`, `cli` and `macros` features.
- `scan` shows what each link is attached to and counts links not attached to a test.

### Fixed: other
- A closed stdout (`rqtk … | head`) ends the command quietly instead of panicking.
- The workspace builds on macOS with a plain `cargo build` (the Python extension links).
- `skills install` says "Added" when it adds its block to an existing AGENTS.md.

## [1.1.0] — 2026-09-23

### Added
- `pip install rqtk` (or `uv tool install rqtk`): the Python package on PyPI now contains the
  full `rqtk` command line as well as the `@rqtk.verifies` decorator. One abi3 wheel per
  platform (Linux glibc and musl, macOS, Windows) covers Python 3.9 and later.

### Fixed
- Building the whole workspace on Windows no longer fails: the Python extension's library is
  now `_rqtk` (imported as `rqtk._rqtk`) instead of `rqtk`, which collided with the `rqtk`
  binary's `rqtk.exe`/`rqtk.pdb`.

## [1.0.0] — 2026-09-23

First stable release: from here on the command line, file format, `--json` output, exit codes
and lint rule codes follow the *Stability* promise in the README. The code is the same as
0.1.0; the list below is everything that changed on the way from the first internal drafts,
where breaking changes were still allowed.

### Verification from test evidence
- `rqtk scan` finds `verifies` links in source: Rust `#[verifies("…")]`, Python
  `@verifies("…")`, and a `// rqtk: verifies …` comment tag for any language.
- `rqtk verify --results junit.xml` records test outcomes per activity in
  `.rqtk/evidence.toml`, against the requirement's content hash. `--check` fails when the
  committed evidence is out of date.
- Coverage adds **Suspect** (tests passed for an earlier version of the requirement) and
  **Failed**. The hand-written `status` of a test-linked activity is ignored (lint RQ029).
- `#[verifies]` rebuilds the annotated test when its requirement file changes.
- Rust projects need just one crate: `rqtk` with its `macros` feature provides
  `#[rqtk::verifies("…")]` (also as `rqtk::macros`), without pulling in the CLI's
  dependencies.

### For coding agents
- Global `--json` on every command; fixed exit codes (0 ok, 1 findings, 2 usage, 3 error);
  `--dry-run` on every command that writes; no interactive prompts.
- `rqtk context <ID>` (briefing for one item), `rqtk impact <rev>` (what a change touches and
  what to re-verify), `rqtk schema <kind>`, `rqtk explain <code>`.
- Agent skills (`rqtk-requirements`, `rqtk-verification`, `/to-requirements`,
  `/requirements-review`), installed with `rqtk skills install` or `rqtk init --agents` into
  `.agents/skills` with links for Claude Code; also available as a Claude Code plugin.

### Distribution
- Prebuilt binaries for macOS, Linux and Windows with shell and PowerShell installers
  (`curl -LsSf https://rqtk.dev/install.sh | sh`), built by cargo-dist on each release tag.
- Documentation at [rqtk.dev](https://rqtk.dev), with `llms.txt` and a Markdown copy of every
  page for agents.

### File format (schema_version 1)
- Flat files: `statement = "…"` and status fields at the top level, links in `[trace]`.
- Unknown fields are errors; every broken file is reported in one run with its line
  (RQ100–RQ102); duplicate IDs and file-name mismatches are caught.
- Native TOML dates are accepted.
- `rehash` and `baseline` preserve comments and layout.
- Content hashes are versioned (`v1:`) and no longer collide across field boundaries.

### Lint
- New rules RQ024–RQ030; `RQ017` names every requirement in a cycle and ignores symmetric
  links; `RQ011` checks all statements with whole-word matching; STK001 became RQ023.

### Removed
- C++ bindings (`rqtk-cpp`, `codegen-cpp-verifies`), GraphML output, the Typst/PDF report
  (`rqtk report` now writes Markdown), interactive prompts, and the unused
  `[change_control]` and `[export]` configuration sections.

## [0.1.0] — 2026-09-23

Pre-release of the same code as 1.0.0, published to exercise the release pipeline: GitHub
Release with installers, crates.io, and rqtk.dev. The library crates (`rqtk-core`,
`rqtk-export`, `rqtk-report`, `rqtk-macros`) remain at 0.1.0.
