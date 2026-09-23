# Changelog

All notable changes to the `rqtk` command line. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); the project follows
[semantic versioning](https://semver.org) as described under *Stability* in the README.

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
