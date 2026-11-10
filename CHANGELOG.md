# Changelog

All notable changes to the `rqtk` command line. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); the project follows
[semantic versioning](https://semver.org) as described under *Stability* in the README.

## [Unreleased]

### Added
- **Interfaces across repositories.** A requirement with an `[interface]` table is an
  interface other repositories can consume, specified by `[[interface.specs]]` entries
  (`format`, `path`, `element`). The spec content is part of the content hash.
  - OpenAPI 3.0 and 3.1 are parsed: `$ref`s are followed across files, `element` pointers are
    checked, and only the element and what it references are hashed, in a canonical form.
    For an operation, path-level parameters, servers, security and the security schemes it
    uses are hashed too. YAML alias expansion and nesting depth are bounded.
  - protobuf, AsyncAPI, JSON Schema and `other` are hashed as whole files; `path` may be a
    glob, which skips what the repository's own `.gitignore` excludes. Line endings are
    normalised before hashing.
  - `impact` and `diff` see a requirement whose spec files alone changed.
- **`rqtk interfaces add | sync | remove | list`.** A consumer declares a provider under
  `[interfaces.<alias>]` in `.rqtk/config.toml` and syncs it into `.rqtk/vendor/<alias>/`,
  pinned in `.rqtk/interfaces.lock` together with the provider's verification status. Sync is
  all or nothing, accepts only `https` and SSH URLs (`file://` with `--allow-file-url`),
  refuses symbolic links, paths and references outside the provider's repository, specs
  that don't resolve, file names with control characters, files over 10 MiB and sources
  over 100 MiB, and warns that vendored content must be reviewed. A sync interrupted while
  swapping in is undone by the next one. `ref` may name a tag, an `rqtk baseline`, a branch
  or a commit.
- **`consumes = ["<alias>:<ID>"]` trace links.** A consumer requirement and its
  descendants become Suspect (`upstream_changed`) when a sync changes what it consumes. `context`, `trace`, `graph`,
  `report`, `coverage` and `impact` show consumed interfaces as read-only `alias:ID` nodes;
  the provider's status never counts toward the consumer's coverage. `rqtk context
  <alias>:<ID>` briefs a vendored requirement.
- `consumes` links may form cycles across repositories (an API one way, a webhook back);
  a change makes its consumers Suspect and never echoes back to its origin. `sync`
  warns when a provider's commit is already in the consumer's own history (the repository
  itself or a fork).
- The agent skills cover interfaces: marking and consuming them, reading vendored text as the
  provider's data, running `interfaces add`/`sync` only when asked and handing over the diff,
  settling `upstream_changed` from a consumed interface, and reviewing `impact.interfaces`.
- Lint rules RQ032–RQ043 for spec files, OpenAPI documents, `consumes` links, vendored
  snapshots, config/lockfile drift and branch pins.

- **`rqtk-client`, a Rust library for reading repositories.** Open a working tree, a tag,
  baseline or commit (read from git, without a checkout) or a git URL (fetched into a per-user
  cache). A repository is a snapshot: queries answer from the state at opening until
  `reload()`. It lists requirements, needs, stakeholders and activities with their links in
  both directions, filters requirements by a `Filter` given as data, and returns the same
  views the CLI prints. A repository is `Send + Sync`, so one snapshot can serve queries from
  many threads at once. Errors carry a stable code. The crate is released with the CLI's
  version, and its public API follows semantic versioning.
- **The client in Python.** The `rqtk` package has `rqtk.open(path, ref=…)` and
  `rqtk.open_remote(url, ref)`, returning a `Repository` with the same queries and views as
  the Rust client. Values are frozen dataclasses with the JSON's field names, and
  enumerations are `Literal` strings (`rqtk.types`, generated from the published schemas).
  Errors raise `rqtk.Error` with the same `code`. Calls release the GIL, and one repository
  answers calls from several threads at the same time.
- **The client in TypeScript: `@rqtk/client`** for Node, Bun and Deno. `open`, `openRef`,
  `openRemote` and `reload` return Promises and load off the event loop; queries return the
  client's JSON, typed by interfaces generated from the published schemas. Errors are
  `RqtkError` with the same `code`.
- **`rqtk baseline --list`** lists the baselines, oldest first.
- **Published JSON Schemas for every `--json` document**, under `schema/output/` (and the
  client's types under `schema/client/`). `rqtk schema` lists them next to the file kinds and
  prints any of them.

### Changed (breaking)
- **`--json` output is version 2.** Every document has a `schema` field naming its shape and
  version, such as `"rqtk/coverage/2"`. The views (`context`, `trace`, `coverage`, `lint`,
  `impact`, `diff`, `baseline --list`, `log`) are now the client's documents, so the CLI and
  the library always agree.
- Errors printed with `--json` carry `schema: "rqtk/error/2"` and, when opening or querying
  the repository failed, the client's error `code`.
- An unknown ID in `trace` or `log` is now a usage error (exit code 2), as it already was in
  `context`.
- An unreadable `.rqtk/evidence.toml` is now a lint finding (`RQ100`) instead of an error
  that stops `lint`, `coverage` and `context`.

### Fixed
- `scan` reads `.mjs`, `.cjs`, `.mts` and `.cts` files by default, so `verifies` tags in
  ES-module and CommonJS test files are found without listing the extensions in `[scan]`.

### Removed
- Intel macOS (`x86_64-apple-darwin`) release binaries and Python wheels. Apple Silicon,
  Linux and Windows builds continue; Intel Macs can still install with `cargo install rqtk`
  or from the source distribution.

### Upgrade notes
- **Scripts that parse `--json`:** ignore the new `schema` field, or check it to know which
  shape you have. Then, per command:
  - `coverage`, `context`: each requirement's `activities` is a list of objects
    (`{"id": …, "state": …}`). The `[id, state]` pairs and the duplicate `activity_states`
    field are gone; read `activities`.
  - `explain`: always `{"rules": […]}`, also for a single rule (`rqtk explain RQ001`).
  - `skills list`: `{"skills": […]}` instead of a bare list.
  - `interfaces list`: `{"sources": […]}` instead of a bare list.
  - `schema` (the listing): `{"kinds": […]}`, each with `kind`, `category` (`file`,
    `document` or `client`) and `describes`, instead of a list of `{kind, file}`.
  - `diff`: the revisions are `from_ref` and `to_ref` (were `from` and `to`; `from` is
    reserved in Python).
  - `impact`: a test file now counts as changed when it differs from the base revision as it
    was when rqtk read the repository, rather than when git was asked afterwards.
- Requirement files with an `[interface]` table, configs with `[interfaces]`, and `consumes`
  links can't be read by rqtk 1.2. Requirements without them hash exactly as before.

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
  row, or `verifies("VA-…", case = "…")`, links one case: Go subtests, pytest parameters,
  rows of `it.each([...])`, and GoogleTest parameter values.
- JUnit `@DisplayName` / `@ParameterizedTest(name = …)` and xUnit `DisplayName` are read from
  the annotations, so tests match under Gradle, which reports display names.
- Evidence names tests as `<source file>::<name>`, the same whichever runner reported them
  (Bun and vitest, CTest and GoogleTest).
- `verify` counts linked tests that got no result, and warns when it records evidence with no
  commit to record it against.
- `init --hook` installs the pre-commit hook.
- The report header shows uncommitted requirement changes and the latest baseline.
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
- `identification.id_pattern` is optional: without it, IDs are checked against
  `<prefix>-<CATEGORY>-<NUMBER>` for the configured categories, so adding a category is one
  table. `init` no longer writes it.
- Everything except history (`impact`, `diff`, `log`, `baseline`) works outside a git
  repository; `lint` used to fail there.
- New stakeholder and need IDs follow `zero_padding` (`STK-0001`), or the width a project's
  existing IDs already use.
- Requirements in top-level categories default to verification level System.
- `trace` says when a requirement is top-level or a leaf instead of printing empty lists, and
  shows titles.
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
